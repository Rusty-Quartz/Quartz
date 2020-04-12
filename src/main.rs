mod server_ping;
mod server;
mod config;
mod tcp_socket_helper;
mod mc_datatype_helpers;

use std::net::{TcpListener};
use std::io::Result;

use server_ping::send_server_ping;
use config::load_config;
use server::QuartzServer;


fn main() -> Result<()> {

	let config = load_config(String::from("./server.properties"));

	let server = QuartzServer {
		players: Vec::new(),
		config: config,
		debug: true
	};

	let listener = TcpListener::bind(format!("127.0.0.1:{}", server.config.port))?;
	
	for stream in listener.incoming() {
		println!("client connecting!");
		send_server_ping(&mut stream?, &server);
	}
	Ok(())
}