
use std::vec::Vec;
use std::io;

use smoltcp::Error;
use smoltcp::phy::Device;

extern {
    // These are the callbacks into C++, called from poll_smoltcp_stack.
    // The available memory behind the pointer parameters is owned by the
    // Rust part, and is at least "don't worry about it" long. (lol safety)
    pub fn smoltcp_send_eth_frame(opp_module_id: i32, data: *const u8, size: u32) -> ();
    pub fn smoltcp_recv_eth_frame(opp_module_id: i32, buffer: *mut u8) -> u32;
}

/// A virtual Ethernet interface.
#[derive(Debug)]
pub struct CInterface {
    opp_module_id: i32,
    mtu:           usize
}

impl CInterface {
    pub fn new(opp_module_id : i32) -> io::Result<CInterface> {
        Ok(CInterface {
            opp_module_id: opp_module_id,
            mtu:           1536
        })
    }
}

impl Device for CInterface {
    type RxBuffer = Vec<u8>;
    type TxBuffer = TxBuffer;

    fn mtu(&self) -> usize { self.mtu }

    fn receive(&mut self) -> Result<Self::RxBuffer, Error> {
        let mut buffer = vec![0; self.mtu];
        unsafe {
            let size = smoltcp_recv_eth_frame(self.opp_module_id, buffer.as_mut_ptr()) as usize;
            trace!("Received an {} bytes long ethernet frame", size);
            buffer.resize(size, 0);
        }
        if buffer.len() == 0 { return Err(Error::Unrecognized); }
        Ok(buffer)
    }

    fn transmit(&mut self, length: usize) -> Result<Self::TxBuffer, Error> {
        Ok(TxBuffer {
            opp_module_id: self.opp_module_id,
            buffer: vec![0; length]
        })
    }
}

#[doc(hidden)]
pub struct TxBuffer {
    opp_module_id : i32,
    buffer: Vec<u8>
}

impl AsRef<[u8]> for TxBuffer {
    fn as_ref(&self) -> &[u8] { self.buffer.as_ref() }
}

impl AsMut<[u8]> for TxBuffer {
    fn as_mut(&mut self) -> &mut [u8] { self.buffer.as_mut() }
}

impl Drop for TxBuffer {
    fn drop(&mut self) {
        unsafe {
            trace!("Sending an {} bytes long ethernet frame", self.buffer.len());
            smoltcp_send_eth_frame(self.opp_module_id, self.buffer.as_ptr(), self.buffer.len() as u32);
        }
    }
}

