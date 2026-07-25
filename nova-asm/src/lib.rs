//! MacroCore-X Assembler for the nova compiler toolchain.
//!
//! Converts MacroCore-X assembly source to flat binary machine code.
//!
//! # Modules
//! - `isa` — ISA instruction encoding definitions and opcode tables
//! - `lexer` — Tokenizer (using logos) and parser
//! - `assembler` — Two-pass assembler with label resolution
//! - `error` — Error types

pub mod isa;
pub mod lexer;
pub mod assembler;
pub mod error;

pub use assembler::Assembler;
pub use error::AsmError;
pub use lexer::{tokenize, parse, ParsedInstruction, Token};

/// Assemble source code into binary.
pub fn assemble_source(source: &str) -> Result<Vec<u8>, AsmError> {
    let tokens = tokenize(source);
    let instructions = parse(&tokens)?;
    let mut asm = Assembler::default();
    asm.assemble(&instructions)
}