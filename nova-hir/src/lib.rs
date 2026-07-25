/// nova-hir: High-level Intermediate Representation and Type Checking for Nova.
///
/// Provides:
/// - HIR data structures (`hir` module)
/// - AST → HIR lowering (`lower` module)
/// - Type checking (`typeck` module)
/// - Error types (`error` module)
pub mod error;
pub mod hir;
pub mod lower;
pub mod typeck;

pub use error::TypeError;
pub use hir::*;
pub use lower::Lowerer;

/// Lower an AST program to HIR and type-check it.
///
/// This is the main entry point for the HIR pipeline.
/// It performs name resolution, type conversion, and type checking.
pub fn lower_and_check(program: &nova_frontend::ast::Program) -> Result<HirProgram, Vec<TypeError>> {
    let mut lowerer = Lowerer::new();
    let hir = lowerer.lower(program)?;

    // Type check the HIR
    typeck::type_check(&hir)?;

    Ok(hir)
}