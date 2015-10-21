use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use std::fmt;

use mio::*;
use mio::tcp::*;
use http_muncher::Parser;
use rustc_serialize::base64::{ToBase64, STANDARD};
use sha1::Sha1;

use http::HttpParser;
use frame::{WebSocketFrame, OpCode};

fn gen_key(key: &str) -> String {
    let mut m = Sha1::new();
    let mut buf = [0u8; 20];

    m.update(key.as_bytes());
    m.update("258EAFA5-E914-47DA-95CA-C5AB0DC85B11".as_bytes());

    m.output(&mut buf);

    return buf.to_base64(STANDARD);
}

enum ClientState {
    AwaitingHandshake(RefCell<Parser<HttpParser>>),
    HandshakeResponse,
    Connected
}

pub struct WebSocketClient {
    pub socket: TcpStream,
    headers: Rc<RefCell<HashMap<String, String>>>,
    pub interest: EventSet,
    state: ClientState,
    outgoing: Vec<WebSocketFrame>
}

impl WebSocketClient {
    pub fn new(socket: TcpStream) -> WebSocketClient {
        let headers = Rc::new(RefCell::new(HashMap::new()));

        WebSocketClient {
            socket: socket,
            headers: headers.clone(),
            interest: EventSet::readable(),
            state: ClientState::AwaitingHandshake(RefCell::new(Parser::request(HttpParser {
                current_key: None,
                headers: headers.clone()
            }))),
            outgoing: Vec::new()
        }
    }

    pub fn write(&mut self) {
        match self.state {
            ClientState::HandshakeResponse => {
                self.write_handshake();
            },
            ClientState::Connected => {
                println!("sending {} frames", self.outgoing.len());
                for frame in self.outgoing.iter() {
                    if let Err(e) = frame.write(&mut self.socket) {
                        println!("error on write: {}", e);
                    }
                }

                self.outgoing.clear();

                self.interest.remove(EventSet::writable());
                self.interest.insert(EventSet::readable());
            },
            _ => {}
        }
    }

    fn write_handshake(&mut self) {
        let headers = self.headers.borrow();
        let response_key = gen_key(&headers.get("Sec-WebSocket-Key").unwrap());
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
                        let reply = WebSocketFrame::from(&*format!("Hello, {}!", payload));

                        self.outgoing.push(reply);
                    },
                    OpCode::Ping => {
                        println!("ping/pong");
                        self.outgoing.push(WebSocketFrame::pong(&frame));
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
                        let mut parser = parser_state.borrow_mut();
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
