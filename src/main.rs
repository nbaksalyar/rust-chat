
extern crate mio_websocket;
extern crate env_logger;

use std::net::SocketAddr;

use mio_websocket::interface::*;

fn main() {
    env_logger::init().unwrap();

    let mut ws = WebSocket::new("0.0.0.0:10000".parse::<SocketAddr>().unwrap());

    loop {
        match ws.next() {
            (tok, WebSocketEvent::Connect) => {
                println!("connected peer: {:?}", tok);
            },

            (tok, WebSocketEvent::TextMessage(msg)) => {
                for peer in ws.get_connected().unwrap() {
                    if peer != tok {
                        println!("-> relaying to peer {:?}", peer);

                        let response = WebSocketEvent::TextMessage(msg.clone());
                        ws.send((peer, response));
                    }
                }
            },

            (tok, WebSocketEvent::BinaryMessage(msg)) => {
                println!("msg from {:?}", tok);
                let response = WebSocketEvent::BinaryMessage(msg.clone());
                ws.send((tok, response));
            },

            _ => {}
        }
    }
}
