
extern crate mio_websocket;
extern crate env_logger;

use std::net::SocketAddr;

use mio_websocket::interface::*;

fn main() {
    env_logger::init().unwrap();

    let mut ws = WebSocket::new("0.0.0.0:10000".parse::<SocketAddr>().unwrap());

    loop {
        match ws.next() {
            WebSocketEvent::Connect(tok) => {
                println!("connected peer: {:?}", tok);
            },

            WebSocketEvent::TextMessage(tok, msg) => {
                println!("msg from {:?}", tok);

                for peer in ws.get_connected().unwrap() {
                    if peer != tok {
                        println!("-> relaying to peer {:?}", peer);

                        let response = WebSocketEvent::TextMessage(peer, msg.clone());
                        ws.send(response);
                    }
                }
            },

            WebSocketEvent::BinaryMessage(tok, msg) => {
                println!("msg from {:?}", tok);
                let response = WebSocketEvent::BinaryMessage(tok, msg.clone());
                ws.send(response);
            },

            _ => {}
        }
    }
}
