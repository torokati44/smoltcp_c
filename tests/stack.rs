extern crate smoltcp_c;

use std::slice;

#[no_mangle]
pub fn smoltcp_send_eth_frame(opp_module_id: i32, data: *const u8, size: u32) -> () {

}


#[no_mangle]
pub unsafe fn smoltcp_recv_eth_frame(opp_module_id: i32, buffer: *mut u8) -> u32 {

    let num_packet_bytes = 66;
    let packet_bytes = &[
    0x80, 0x29, 0x94, 0x8b, 0x90, 0xd7, 0xe0, 0xdb,
    0x55, 0xd6, 0x98, 0x43, 0x08, 0x00, 0x45, 0x00,
    0x00, 0x34, 0xf0, 0x69, 0x40, 0x00, 0x40, 0x06,
    0xad, 0x59, 0x0a, 0x00, 0x00, 0x0c, 0x68, 0xf4,
    0x2a, 0x01, 0xd7, 0xb6, 0x01, 0xbb, 0xfb, 0xac,
    0x9f, 0x74, 0x0f, 0x41, 0xfc, 0x63, 0x80, 0x10,
    0x08, 0x43, 0xb4, 0x34, 0x00, 0x00, 0x01, 0x01,
    0x08, 0x0a, 0x02, 0x23, 0xd6, 0xc2, 0xb6, 0x18,
    0x0e, 0x0e
    ];

    let mut buf = slice::from_raw_parts_mut(buffer, num_packet_bytes);
    buf.clone_from_slice(packet_bytes);
    num_packet_bytes as u32
}

#[test]
fn it_works() {
    unsafe {
        let s = smoltcp_c::make_smoltcp_stack(3);
        smoltcp_c::poll_smoltcp_stack(s, 44);
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
    	let s = smoltcp_c::make_smoltcp_stack(2);
        assert_eq!(4, add_two(2));
    }

}*/