#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    Assign,
    Asterisk,
    Bang,
    Comma,
    EOF,
    Function,
    GT,
    Ident(String),
    Illegal(String),
    Int(String),
    LBrace,
    LParen,
    LT,
    Let,
    Minus,
    Plus,
    RBrace,
    RParen,
    Semicolon,
    Slash,
}
