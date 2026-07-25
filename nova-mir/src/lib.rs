/// nova-mir: Mid-level Intermediate Representation for Nova.
///
/// Provides:
/// - MIR data structures (`mir` module)
/// - HIR → MIR lowering (`lower` module)
pub mod mir;
pub mod lower;

pub use mir::*;
pub use lower::MirLowerer;

/// Lower an HIR program to MIR.
pub fn lower_to_mir(hir: &nova_hir::hir::HirProgram) -> MirProgram {
    let mut lowerer = MirLowerer::new();
    lowerer.lower(hir)
}