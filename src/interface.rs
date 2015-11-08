/// High-level WebSocket library interface

use std::net::SocketAddr;
use std::thread;
use std::sync::mpsc;

use mio::{Token, EventLoop, EventSet, PollOpt};
use mio::tcp::{TcpListener};

use server::{WebSocketServer, WebSocketServerState, SERVER_TOKEN};

#[derive(Clone)]
pub enum WebSocketEvent {
    Connect(Token),
    Close(Token),
    Ping(Token, Vec<u8>),
    Pong(Token, Vec<u8>),
    TextMessage(Token, String),
    BinaryMessage(Token, Vec<u8>),
    // Internal service messages
    Reregister(Token)
}

pub trait WebSocketHandler {
    fn run(&mut self, receiver: mpsc::Receiver<WebSocketEvent>, server: &mut WebSocketServerState);
}

pub struct WebSocket {
    state: WebSocketServerState
}

impl WebSocket {
    pub fn new(address: SocketAddr) -> (mpsc::Receiver<WebSocketEvent>, WebSocket) {
        let server_state = WebSocketServerState::new();
        let mio_server_state = server_state.clone();

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let server_socket = TcpListener::bind(&address).unwrap();

            let mut server = WebSocketServer::new(server_socket, mio_server_state, tx);
            let mut event_loop = EventLoop::new().unwrap();

            event_loop.register(&server.socket,
                                SERVER_TOKEN,
                                EventSet::readable(),
                                PollOpt::edge()).unwrap();

            event_loop.run(&mut server).unwrap();
        });

        (rx, WebSocket { state: server_state })
    }

    pub fn get_peers(&mut self) -> Vec<Token> {
        self.state.get_peers().unwrap_or_else(Vec::new)
    }

    pub fn send(&mut self, msg: WebSocketEvent) {
        self.state.send_message(msg);
    }
}
