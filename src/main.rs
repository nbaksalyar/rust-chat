
extern crate mio;
extern crate http_muncher;
extern crate sha1;
extern crate rustc_serialize;
extern crate byteorder;

mod frame;
mod client;
mod http;
mod server;

use std::net::SocketAddr;

use mio::*;
use mio::tcp::*;

use server::WebSocketServer;

fn main() {
    let address = "0.0.0.0:10000".parse::<SocketAddr>().unwrap();
    let server_socket = TcpListener::bind(&address).unwrap();

    let mut event_loop = EventLoop::new().unwrap();

    let mut server = WebSocketServer::new(server_socket);

    event_loop.register(&server.socket,
                        server::SERVER_TOKEN,
                        EventSet::readable(),
                        PollOpt::edge()).unwrap();
    event_loop.run(&mut server).unwrap();
}
