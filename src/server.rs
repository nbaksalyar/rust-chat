use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};

use mio::*;
use mio::tcp::*;

use client::WebSocketClient;
use interface::WebSocketEvent;

pub const SERVER_TOKEN: Token = Token(0);

#[derive(Clone)]
pub struct WebSocketServerState {
    clients: Arc<RwLock<HashMap<Token, WebSocketClient>>>,
    token_counter: Arc<AtomicUsize>
}

impl WebSocketServerState {
    pub fn new() -> WebSocketServerState {
        WebSocketServerState {
            token_counter: Arc::new(AtomicUsize::new(1)),
            clients: Arc::new(RwLock::new(HashMap::new()))
        }
    }

    fn new_client(&mut self, client_socket: TcpStream, tx: mpsc::Sender<WebSocketEvent>,
                  event_loop_tx: Sender<WebSocketEvent>) -> Token {
        let new_token = Token(self.token_counter.fetch_add(1, Ordering::SeqCst));

        if let Ok(ref mut clients) = self.clients.write() {
            clients.insert(new_token, WebSocketClient::new(client_socket, new_token, tx.clone(), event_loop_tx));
        }

        new_token
    }

    pub fn get_peers(&self) -> Option<Vec<Token>> {
        if let Ok(clients) = self.clients.read() {
            Some(clients.keys().cloned().collect::<Vec<_>>())
        } else {
            None
        }
    }

    fn remove_client(&mut self, tkn: &Token) -> Option<WebSocketClient> {
        if let Ok(ref mut clients) = self.clients.write() {
            clients.remove(tkn)
        } else {
            None
        }
    }

    pub fn send_message(&mut self, msg: WebSocketEvent) {
        match msg {
            WebSocketEvent::TextMessage(tkn, _) |
            WebSocketEvent::BinaryMessage(tkn, _) |
            WebSocketEvent::Connect(tkn) |
            WebSocketEvent::Close(tkn) |
            WebSocketEvent::Ping(tkn, _) => {
                if let Ok(ref mut clients) = self.clients.write() {
                    let client = clients.get_mut(&tkn).unwrap();

                    client.send_message(msg);
                }
            },
            _ => {}
        }
    }
}

pub struct WebSocketServer {
    pub socket: TcpListener,
    state: WebSocketServerState,
    tx: mpsc::Sender<WebSocketEvent>
}

impl WebSocketServer {
    pub fn new(socket: TcpListener, shared_state: WebSocketServerState, tx: mpsc::Sender<WebSocketEvent>) -> WebSocketServer {
        WebSocketServer {
            socket: socket,
            state: shared_state,
            tx: tx
        }
    }
}

impl Handler for WebSocketServer {
    type Timeout = usize;
    type Message = WebSocketEvent;

    fn notify(&mut self, event_loop: &mut EventLoop<WebSocketServer>, msg: WebSocketEvent) {
        match msg {
            WebSocketEvent::Reregister(tkn) => {
                if let Ok(ref clients) = self.state.clients.read() {
                    let client = clients.get(&tkn).unwrap();
                    event_loop.reregister(&client.socket, tkn, client.interest,
                                          PollOpt::edge() | PollOpt::oneshot()).unwrap();
                }
            },
            _ => {}
        }
    }

    fn ready(&mut self, event_loop: &mut EventLoop<WebSocketServer>, token: Token, events: EventSet) {
        if events.is_readable() {
            match token {
                SERVER_TOKEN => {
                    let client_socket = match self.socket.accept() {
                        Ok(Some((sock, _))) => sock,
                        Ok(None) => unreachable!(),
                        Err(e) => {
                            println!("Accept error: {}", e);
                            return;
                        }
                    };

                    let new_token = self.state.new_client(client_socket, self.tx.clone(), event_loop.channel());

                    if let Ok(ref clients) = self.state.clients.read() {
                        event_loop.register(&clients[&new_token].socket,
                                            new_token, EventSet::readable(),
                                            PollOpt::edge() | PollOpt::oneshot()).unwrap();
                    }
                },
	        token => {
                    if let Ok(ref mut clients) = self.state.clients.write() {
                        let mut client = clients.get_mut(&token).unwrap();

                        client.read();

                        event_loop.reregister(&client.socket, token, client.interest,
                                              PollOpt::edge() | PollOpt::oneshot()).unwrap();
                    }
                }
            }
        }

        if events.is_writable() {
            if let Ok(ref mut clients) = self.state.clients.write() {
                let mut client = clients.get_mut(&token).unwrap();

                client.write();

                event_loop.reregister(&client.socket, token, client.interest,
                                      PollOpt::edge() | PollOpt::oneshot()).unwrap();
            }
        }

        if events.is_hup() {
            // Close connection
            let client = self.state.remove_client(&token).unwrap();

            client.socket.shutdown(Shutdown::Both);
            event_loop.deregister(&client.socket);

            println!("hang up connection");
        }
    }
}
