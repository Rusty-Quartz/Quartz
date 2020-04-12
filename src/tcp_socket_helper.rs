use std::net::{TcpStream};
use std::io::prelude::*;


pub fn read_packet(stream: &mut TcpStream, buf: &mut [u8]) -> usize {
	println!("Attempting to read packet");
	let len = stream.read(&mut *buf).unwrap();

	print_byte_array(&buf, buf[0] as usize);

	len
}


fn print_byte_array(arr: &[u8], bytes:usize) {
	for i in 0..bytes+1 {
		print!("{:02x?},", arr[i]);
	}
	print!("\n");
}