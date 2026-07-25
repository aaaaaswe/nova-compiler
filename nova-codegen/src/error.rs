/// Code generation error types.
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodegenError {
    /// I/O error during code generation.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// NIR parse error.
    #[error("parse error: {0}")]
    Parse(#[from] nova_nir::parser::ParseError),

    /// Register allocation failure.
    #[error("register allocation error: {0}")]
    RegAlloc(String),

    /// Unsupported instruction for the current code generation mode.
    #[error("unsupported instruction: {0}")]
    UnsupportedInstruction(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}