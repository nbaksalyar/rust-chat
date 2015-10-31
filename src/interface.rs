use mio::Token;

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
