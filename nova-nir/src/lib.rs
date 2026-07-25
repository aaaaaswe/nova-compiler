//! NIR (Nova Intermediate Representation) – the core IR for the MacroCore-X compiler.
//!
//! This crate defines:
//! - The IR type system ([`types::IrType`])
//! - Values ([`types::Value`]) – virtual registers, constants, globals, parameters
//! - Address expressions ([`ir::AddrExpr`])
//! - Instructions ([`ir::Instruction`]) – 126 opcodes across 13 categories
//! - Basic blocks ([`ir::BasicBlock`])
//! - Functions ([`ir::Function`])
//! - Modules ([`ir::Module`])

pub mod types;
pub mod ir;
pub mod parser;
pub mod validator;
pub mod optimizer;

pub use types::IrType;
pub use types::NirError;
pub use types::Value;
pub use ir::AddrExpr;
pub use ir::BasicBlock;
pub use ir::Function;
pub use ir::Instruction;
pub use ir::Module;

#[cfg(test)]
mod tests;