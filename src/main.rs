
extern crate mio;
extern crate http_muncher;
extern crate sha1;
extern crate rustc_serialize;
extern crate byteorder;

mod frame;
mod client;
mod http;
mod server;
mod interface;

use std::net::SocketAddr;
use std::thread;

use mio::*;
use mio::tcp::*;
use std::sync::mpsc;

use server::{WebSocketServer, WebSocketServerState};
use interface::*;

struct ChatHandler {
    server: WebSocketServerState,
    receiver: mpsc::Receiver<WebSocketEvent>
}

impl ChatHandler {
    fn run(&mut self) {
        for event in self.receiver.iter() {
            match event {
                WebSocketEvent::Connect(tok) => {
                    println!("connected peer: {:?}", tok);
                    self.server.send_message(WebSocketEvent::TextMessage(tok, "Hello!".to_string()));
                },

                WebSocketEvent::TextMessage(tok, msg) => {
                    println!("msg from {:?}", tok);

                    for peer in self.server.get_peers().unwrap().into_iter() {
                        println!("-> relaying to peer {:?}", peer);

                        let response = WebSocketEvent::TextMessage(peer, format!("{:?} says \"{}\"", tok, msg));
                        self.server.send_message(response);
                    }
                },

                _ => {}
            }
        }
    }
}

fn main() {
    let address = "0.0.0.0:10000".parse::<SocketAddr>().unwrap();
    let server_socket = TcpListener::bind(&address).unwrap();

    let (tx, rx) = mpsc::channel();

    let server_state = WebSocketServerState::new();

    let socket = server_socket.try_clone().unwrap();
    let server_state_cl = server_state.clone();
    let tx = tx.clone();

    thread::spawn(move || {
        let mut server = WebSocketServer::new(socket, server_state_cl, tx);
        let mut event_loop = EventLoop::new().unwrap();

        event_loop.register(&server.socket,
                            server::SERVER_TOKEN,
                            EventSet::readable(),
                            PollOpt::edge()).unwrap();

        event_loop.run(&mut server).unwrap();
    });

    let mut handler = ChatHandler {
        server: server_state,
        receiver: rx
    };
    handler.run();
}
