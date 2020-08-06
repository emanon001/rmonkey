#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    Assign,
    Asterisk,
    Bang,
    Comma,
    Else,
    Eq,
    False,
    Function,
    GT,
    Identifier(String),
    If,
    Illegal(String),
    Int(String),
    LBrace,
    LParen,
    LT,
    Let,
    Minus,
    NotEq,
    Plus,
    RBrace,
    RParen,
    Return,
    Semicolon,
    Slash,
    String(String),
    True,
}
