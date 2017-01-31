#[macro_use]
extern crate log;
extern crate env_logger;
extern crate smoltcp;

use std::str::{self};
use std::time::{Instant};
use std::vec::Vec;

use log::{LogLevelFilter, LogRecord};
use env_logger::{LogBuilder};

use smoltcp::wire::{EthernetAddress, IpAddress, EthernetFrame, PrettyPrinter};
use smoltcp::iface::{ArpCache, SliceArpCache, EthernetInterface};
use smoltcp::socket::{SocketHandle, SocketSet, AsSocket, TcpSocket, TcpSocketBuffer};
use smoltcp::phy::Tracer;


pub mod device;
use device::CInterface;


pub struct Stack<'a, 'b : 'a, 'c: 'a + 'b> {
    iface : EthernetInterface<'a, 'b, 'c, Tracer<CInterface, EthernetFrame<&'static[u8]>>>,
    tcp_handle: SocketHandle,
    sockets: SocketSet<'a, 'b, 'c>,
    tcp_active: bool,
}

// This will be called from C++ to create the stack for the OPP module given by its id.
// The returned pointer should be used only to identify the stack instance by
// passing it to poll_smoltcp_stack, and must not be dereferenced from C++.
// TODO: take MAC and IP addresses as parameters
#[no_mangle]
pub unsafe extern fn make_smoltcp_stack(opp_module_id: i32) -> *mut Stack<'static,'static,'static> {
    let startup_time = Instant::now();
    LogBuilder::new()
        .format(move |record: &LogRecord| {
            let elapsed = Instant::now().duration_since(startup_time);
            if record.target().starts_with("smoltcp::") {
                format!("\x1b[0m[{:6}.{:03}s] ({}): {}\x1b[0m",
                        elapsed.as_secs(), elapsed.subsec_nanos() / 1000000,
                        record.target().replace("smoltcp::", ""), record.args())
            } else {
                format!("\x1b[32m[{:6}.{:03}s] ({}): {}\x1b[0m",
                        elapsed.as_secs(), elapsed.subsec_nanos() / 1000000,
                        record.target(), record.args())
            }
        })
        .filter(None, LogLevelFilter::Trace)
        .init()
        .unwrap();

    fn trace_writer(printer: PrettyPrinter<EthernetFrame<&[u8]>>) {
        print!("\x1b[37m{}\x1b[0m", printer)
    }
  
    let device = CInterface::new(opp_module_id).unwrap();
    let device = Tracer::<_, EthernetFrame<&[u8]>>::new(device, trace_writer);

    let arp_cache = SliceArpCache::new(vec![Default::default(); 8]);
    
    let tcp_rx_buffer = TcpSocketBuffer::new(vec![0; 64]);
    let tcp_tx_buffer = TcpSocketBuffer::new(vec![0; 128]);
    let tcp_socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);

    let hardware_addr  = EthernetAddress([0x0A, 0xAA, 0x00, 0x00, 0x00, 0x02]);
    let protocol_addrs = [IpAddress::v4(10, 0, 0, 3)];
    let iface          = EthernetInterface::new(
        Box::new(device), Box::new(arp_cache) as Box<ArpCache>,
        hardware_addr, protocol_addrs);

    let mut sockets = SocketSet::new(vec![]);
    let tcp_handle = sockets.add(tcp_socket);

    let stack = Stack{ iface: iface, tcp_handle : tcp_handle, sockets : sockets, tcp_active: false};
    let boxed_builder = Box::new (stack);

    trace!("Stack succesfully created");
    Box::into_raw(boxed_builder)
}

// This will be called from C++, and calls back to there for sending/receiving frames.
#[no_mangle]
pub unsafe extern fn poll_smoltcp_stack(stack: *mut Stack, timestamp_ms : u64) -> () {
    trace!("Polling smoltcp stack at {} ms", timestamp_ms);
    if stack.is_null() { return }
    
    let mut stack = Box::from_raw(stack);
    stack.poll(timestamp_ms);
    trace!("Polling done");
    Box::into_raw(stack);
}

impl<'a, 'b, 'c> Stack<'a, 'b, 'c> {
    fn poll(&mut self, timestamp_ms : u64) -> () {
       
        trace!("Poll callback at {} ms", timestamp_ms);
        
        // doing a real poll first to receive any incoming frames
        match self.iface.poll(&mut self.sockets, timestamp_ms) {
            Ok(()) => (),
            Err(e) => debug!("poll error: {}", e)
        }
        
        // tcp:6970: echo with reverse
        {
            let mut socket: &mut TcpSocket = self.sockets.get_mut(self.tcp_handle).as_socket();

            if !socket.is_open() {
                trace!("Listening on tcp port 6970");
                socket.listen(6970).unwrap()
            }

            if socket.is_active() && !self.tcp_active {
                debug!("tcp:6970 connected");
            } else if !socket.is_active() && self.tcp_active {
                debug!("tcp:6970 disconnected");
            }
            self.tcp_active = socket.is_active();

            if socket.may_recv() {
                let data = {
                    let mut data = socket.recv(128).unwrap().to_owned();
                    if data.len() > 0 {
                        debug!("tcp:6970 recv data: {:?}",
                               str::from_utf8(data.as_ref()).unwrap_or("(invalid utf8)"));
                        data = data.split(|&b| b == b'\n').collect::<Vec<_>>().concat();
                        data.reverse();
                        data.extend(b"\n");
                    }
                    data
                };
                if socket.can_send() && data.len() > 0 {
                    debug!("tcp:6970 send data: {:?}",
                           str::from_utf8(data.as_ref()).unwrap_or("(invalid utf8)"));
                    socket.send_slice(&data[..]).unwrap();
                }
            } else if socket.may_send() {
                debug!("tcp:6970 close");
                socket.close();
            }
        }
        
        // and doing another real poll after processing to send out frames we just created
        match self.iface.poll(&mut self.sockets, timestamp_ms) {
            Ok(()) => (),
            Err(e) => debug!("poll error: {}", e)
        }
    }
}
