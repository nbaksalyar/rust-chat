use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::fmt;
use std::sync::mpsc;

use mio::*;
use mio::tcp::*;
use http_muncher::Parser;
use rustc_serialize::base64::{ToBase64, STANDARD};
use sha1::Sha1;

use http::HttpParser;
use frame::{WebSocketFrame, OpCode};
use server::WebSocketServer;
use interface::WebSocketEvent;

const WEBSOCKET_KEY: &'static [u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

fn gen_key(key: &str) -> String {
    let mut m = Sha1::new();
    let mut buf = [0u8; 20];

    m.update(key.as_bytes());
    m.update(WEBSOCKET_KEY);

    m.output(&mut buf);

    return buf.to_base64(STANDARD);
}

enum ClientState {
    AwaitingHandshake(Mutex<Parser<HttpParser>>),
    HandshakeResponse,
    Connected
}

pub struct WebSocketClient {
    pub socket: TcpStream,
    headers: Arc<Mutex<HashMap<String, String>>>,
    pub interest: EventSet,
    state: ClientState,
    outgoing: Vec<WebSocketEvent>,
    tx: Mutex<mpsc::Sender<WebSocketEvent>>,
    event_loop_tx: Sender<WebSocketEvent>,
    token: Token
}

impl WebSocketClient {
    pub fn new(socket: TcpStream, token: Token, server_sink: mpsc::Sender<WebSocketEvent>,
               event_loop_sink: Sender<WebSocketEvent>) -> WebSocketClient {
        let headers = Arc::new(Mutex::new(HashMap::new()));

        WebSocketClient {
            socket: socket,
            headers: headers.clone(),
            interest: EventSet::readable(),
            state: ClientState::AwaitingHandshake(Mutex::new(Parser::request(HttpParser {
                current_key: None,
                headers: headers.clone()
            }))),
            outgoing: Vec::new(),
            tx: Mutex::new(server_sink),
            event_loop_tx: event_loop_sink,
            token: token,
        }
    }

    pub fn send_message(&mut self, msg: WebSocketEvent) {
        self.outgoing.push(msg);

        if self.interest.is_readable() {
            self.interest.insert(EventSet::writable());
            self.interest.remove(EventSet::readable());

            self.event_loop_tx.send(WebSocketEvent::Reregister(self.token));
        }
    }

    pub fn write(&mut self) {
        match self.state {
            ClientState::HandshakeResponse => self.write_handshake(),
            ClientState::Connected => self.write_frames(),
            _ => {}
        }
    }

    fn write_handshake(&mut self) {
        let headers = self.headers.lock().unwrap();
        let response_key = gen_key(&*headers.get("Sec-WebSocket-Key").unwrap());
        let response = fmt::format(format_args!("HTTP/1.1 101 Switching Protocols\r\n\
                                                 Connection: Upgrade\r\n\
                                                 Sec-WebSocket-Accept: {}\r\n\
                                                 Upgrade: websocket\r\n\r\n", response_key));
        self.socket.try_write(response.as_bytes()).unwrap();

        // Change the state
        self.state = ClientState::Connected;

        self.interest.remove(EventSet::writable());
        self.interest.insert(EventSet::readable());
    }

    fn write_frames(&mut self) {
        let mut close_connection = false;

        {
            let frames = self.outgoing.iter().filter_map(|event| {
                match *event {
                    WebSocketEvent::TextMessage(_, ref data) => Some(WebSocketFrame::from(&*data.clone())),
                    WebSocketEvent::BinaryMessage(_, ref data) => Some(WebSocketFrame::from(data.clone())),
                    WebSocketEvent::Close(_) => Some(WebSocketFrame::close(0, b"Server-initiated close")),
                    WebSocketEvent::Ping(_) => Some(WebSocketFrame::ping()),
                    _ => None
                }
            });

            for frame in frames {
                println!("outgoing {:?}", frame);

                if let Err(e) = frame.write(&mut self.socket) {
                    println!("error on write: {}", e);
                }

                if frame.is_close() {
                    close_connection = true;
                }
            }
        }

        self.outgoing.clear();

        self.interest.remove(EventSet::writable());
        self.interest.insert(if close_connection {
            EventSet::hup()
        } else {
            EventSet::readable()
        });
    }

    pub fn read(&mut self) {
        match self.state {
            ClientState::AwaitingHandshake(_) => self.read_handshake(),
            ClientState::Connected => self.read_frame(),
            _ => {}
        }
    }

    fn read_frame(&mut self) {
        let frame = WebSocketFrame::read(&mut self.socket);
        println!("recv frame: {:?}", frame);
        match frame {
            Ok(frame) => {
                match frame.get_opcode() {
                    OpCode::TextFrame => {
                        let payload = String::from_utf8(frame.payload).unwrap();
                        self.tx.lock().unwrap().send(WebSocketEvent::TextMessage(self.token, payload));
                    },
                    OpCode::Ping => {
                        println!("ping/pong");
                        //self.outgoing.send(WebSocketFrame::pong(&frame));
                    },
                    OpCode::ConnectionClose => {
                        self.tx.lock().unwrap().send(WebSocketEvent::Close(self.token));
                        //self.outgoing.send(WebSocketFrame::close_from(&frame));
                    },
                    _ => {}
                }

                self.interest.remove(EventSet::readable());
                self.interest.insert(EventSet::writable());
            }
            Err(e) => println!("error while reading frame: {}", e)
        }
    }

    fn read_handshake(&mut self) {
        loop {
            let mut buf = [0; 2048];
            match self.socket.try_read(&mut buf) {
                Err(e) => {
                    println!("Error while reading socket: {:?}", e);
                    return
                },
                Ok(None) =>
                    // Socket buffer has got no more bytes.
                    break,
                Ok(Some(len)) => {
                    let is_upgrade = if let ClientState::AwaitingHandshake(ref parser_state) = self.state {
                        let mut parser = parser_state.lock().unwrap();
                        parser.parse(&buf);
                        parser.is_upgrade()
                    } else { false };

                    if is_upgrade {
                        // Change the current state
                        self.state = ClientState::HandshakeResponse;

                        // Change current interest to `Writable`
                        self.interest.remove(EventSet::readable());
                        self.interest.insert(EventSet::writable());
                        break;
                    }
                }
            }
        }
    }
}
