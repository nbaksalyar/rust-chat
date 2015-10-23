use mio::Token;

#[derive(Clone)]
pub enum WebSocketEvent {
    Connect(Token),
    Close(Token),
    Ping(Token),
    TextMessage(Token, String),
    BinaryMessage(Token, Vec<u8>),
    // Internal service messages
    Reregister(Token)
}
