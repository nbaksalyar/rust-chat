
extern crate mio_websocket;

use std::net::SocketAddr;

use mio_websocket::interface::*;

fn main() {
    let (events, mut ws) = WebSocket::new("0.0.0.0:10000".parse::<SocketAddr>().unwrap());

    for event in events {
        match event {
            WebSocketEvent::Connect(tok) => {
                println!("connected peer: {:?}", tok);

                ws.send(WebSocketEvent::TextMessage(tok, "Hello!".to_string()));
            },

            WebSocketEvent::TextMessage(tok, msg) => {
                println!("msg from {:?}", tok);

                for peer in ws.get_peers() {
                    println!("-> relaying to peer {:?}", peer);

                    let response = WebSocketEvent::TextMessage(peer, format!("{:?} says \"{}\"", tok, msg));
                    ws.send(response);
                }
            },

            _ => {}
        }
    }
}
