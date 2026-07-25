use thiserror::Error;

/// Errors that can occur during assembly.
#[derive(Error, Debug)]
pub enum AsmError {
    #[error("lexer error at line {line}: {msg}")]
    LexerError { line: usize, msg: String },

    #[error("parse error at line {line}: {msg}")]
    ParseError { line: usize, msg: String },

    #[error("unknown mnemonic '{mnemonic}' at line {line}")]
    UnknownMnemonic { mnemonic: String, line: usize },

    #[error("undefined label: {label}")]
    UndefinedLabel { label: String },

    #[error("invalid operand at line {line}: {msg}")]
    InvalidOperand { line: usize, msg: String },

    #[error("immediate out of range at line {line}: {msg}")]
    ImmediateRange { line: usize, msg: String },

    #[error("register {reg} not allowed in this context at line {line} (only R0-R15 supported)")]
    RegisterOutOfRange { reg: String, line: usize },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Generic(String),
}