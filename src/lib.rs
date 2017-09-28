#[macro_use]
extern crate log;
extern crate smoltcp;

use std::str::{self, FromStr};

use smoltcp::wire::{EthernetAddress, IpAddress};
use smoltcp::iface::{ArpCache, SliceArpCache, EthernetInterface};
use smoltcp::socket::{SocketHandle, SocketSet, Socket, AsSocket, TcpSocket, TcpSocketBuffer};
use smoltcp::phy::EthernetTracer;


pub mod logger;
pub mod device;
use device::CInterface;
use std::any::Any;

use std::ffi::CStr;
use std::os::raw::c_char;


extern "C" {
    // this is calling BACK TO C++ - implemented there
    pub fn smoltcp_recv_tcp_data(opp_module_id: i32, port: u16, data: *const u8, size: u32) -> ();
}

pub struct Stack<'a, 'b: 'a, 'c: 'a + 'b> {
    iface: EthernetInterface<'a, 'b, 'c, EthernetTracer<CInterface>>,
    sockets: SocketSet<'a, 'b, 'c>,
    opp_module_id: i32,
}


// this is called FROM C++
#[no_mangle]
pub unsafe extern "C" fn smoltcp_send_tcp_data(
    stack: *mut Stack<'static, 'static, 'static>,
    port: u16,
    data: *const u8,
    size: u32,
) -> () {
    let mut stack = Box::from_raw(stack);

    for mut socket in stack.sockets.iter_mut() {
        let mut tcp_socket: &mut TcpSocket = socket.as_socket();

        if tcp_socket.local_endpoint().port == port {
            trace!("got me sock\n");

            tcp_socket.send_slice(std::slice::from_raw_parts(data, size as usize));
        }
    }


    Box::into_raw(stack);
}


// This will be called from C++ to create the stack for the OPP module given by its id.
// The returned pointer should be used only to identify the stack instance by
// passing it to poll_smoltcp_stack, and must not be dereferenced from C++.
#[no_mangle]
pub unsafe extern "C" fn make_smoltcp_stack(
    opp_module_id: i32,
    mac: *const c_char,
    ip: *const c_char,
) -> *mut Stack<'static, 'static, 'static> {

    let device = CInterface::new(opp_module_id).unwrap();
    let device = EthernetTracer::new(device, |_timestamp, printer| trace!("{}", printer));

    let arp_cache = SliceArpCache::new(vec![Default::default(); 8]);

    let m = CStr::from_ptr(mac);
    let i = CStr::from_ptr(ip);

    let hardware_addr = EthernetAddress::from_str(m.to_str().unwrap()).unwrap();
    let protocol_addrs = [IpAddress::from_str(i.to_str().unwrap()).unwrap()];

    let iface = EthernetInterface::new(
        Box::new(device),
        Box::new(arp_cache) as Box<ArpCache>,
        hardware_addr,
        protocol_addrs,
    );

    let sockets = SocketSet::new(vec![]);

    let stack = Stack {
        iface: iface,
        sockets: sockets,
        opp_module_id: opp_module_id,
    };
    let boxed_builder = Box::new(stack);

    trace!("Stack succesfully created");
    Box::into_raw(boxed_builder)
}


#[no_mangle]
pub unsafe extern "C" fn make_smoltcp_tcp_socket() -> *mut Socket<'static, 'static> {
    let tcp_rx_buffer = TcpSocketBuffer::new(vec![0; 64]);
    let tcp_tx_buffer = TcpSocketBuffer::new(vec![0; 128]);
    let tcp_sock = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);

    Box::into_raw(Box::new(tcp_sock))
}


#[no_mangle]
pub unsafe extern "C" fn add_smoltcp_tcp_socket(
    stack: *mut Stack<'static, 'static, 'static>,
    socket: *mut Socket<'static, 'static>,
) -> SocketHandle {
    let x: Result<Box<Socket>, _> = (Box::from_raw(socket) as Box<Any + 'static>).downcast();
    let mut z = *x.unwrap();
    {
        let t: &mut TcpSocket = z.as_socket();
        t.listen(6970).unwrap();
    }

    (*stack).sockets.add(z)
}



#[no_mangle]
pub unsafe extern "C" fn make_add_smoltcp_tcp_socket(
    stack: *mut Stack<'static, 'static, 'static>,
) -> SocketHandle {
    let tcp_rx_buffer = TcpSocketBuffer::new(vec![0; 64]);
    let tcp_tx_buffer = TcpSocketBuffer::new(vec![0; 128]);
    let mut z = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);


    //let mut z = *x.unwrap();
    {
        let t: &mut TcpSocket = z.as_socket();
        t.listen(6970).unwrap();
    }
    (*stack).sockets.add(z)
}

// This will be called from C++, and calls back to there for sending/receiving frames.
#[no_mangle]
pub unsafe extern "C" fn poll_smoltcp_stack(stack: *mut Stack, timestamp_ms: u64) -> () {
    trace!("Polling smoltcp stack at {} ms", timestamp_ms);
    if stack.is_null() {
        return;
    }

    let mut stack = Box::from_raw(stack);
    stack.poll(timestamp_ms);
    trace!("Polling done");
    Box::into_raw(stack);
}

impl<'a, 'b, 'c> Stack<'a, 'b, 'c> {
    fn poll(&mut self, timestamp_ms: u64) -> () {

        trace!("Poll callback at {} ms", timestamp_ms);

        // doing a real poll first to receive any incoming frames
        match self.iface.poll(&mut self.sockets, timestamp_ms) {
            Ok(_) => (),
            Err(e) => debug!("poll error: {}", e),
        }


        // tcp:6970: echo with reverse
        {
            for mut socket in self.sockets.iter_mut() {
                let mut tcp_socket: &mut TcpSocket = socket.as_socket();
                if !tcp_socket.is_open() {
                    trace!("Listening on tcp port 6970");
                    tcp_socket.listen(6970).unwrap()
                }

                if tcp_socket.is_active() {
                    debug!("tcp:{} is connected", tcp_socket.local_endpoint().port);
                } else if !tcp_socket.is_active() {
                    debug!("tcp:{} is not connected", tcp_socket.local_endpoint().port);
                }

                if tcp_socket.may_recv() {
                    let data = {
                        let data = tcp_socket.recv(2000).unwrap().to_owned();
                        if data.len() > 0 {
                            debug!(
                                "tcp:6970 recv data: {:?}",
                                str::from_utf8(data.as_ref()).unwrap_or("(invalid utf8)")
                            );
                            //data.reverse();
                        }
                        data
                    };

                    unsafe {
                        smoltcp_recv_tcp_data(
                            self.opp_module_id,
                            tcp_socket.local_endpoint().port,
                            data.as_ptr(),
                            data.len() as u32,
                        );
                    }
                } else if tcp_socket.may_send() {
                    debug!("tcp:{} close", tcp_socket.local_endpoint().port);
                    tcp_socket.close();
                }
            }
        }

        /*
if tcp_socket.can_send() && data.len() > 0 {
                        debug!(
                            "tcp:6970 send data: {:?}",
                            str::from_utf8(data.as_ref()).unwrap_or("(invalid utf8)")
                        );
                        tcp_socket.send_slice(&data[..]).unwrap();
                    }
*/

        // and doing another real poll after processing to send out frames we just created
        match self.iface.poll(&mut self.sockets, timestamp_ms) {
            Ok(_) => (),
            Err(e) => debug!("poll error: {}", e),
        }
    }
}
