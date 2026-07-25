/// nova-codegen: NIR to MacroCore-X assembly code generation.
///
/// Provides three code generation backends:
/// - **RISC**: Pure RISC mode, expands all composite instructions.
/// - **CISC**: Pure CISC mode, uses composite instructions directly.
/// - **Hybrid**: Dynamically selects RISC/CISC based on register pressure.
///
/// Also provides Nova frontend pipeline support:
/// - **nova_lower**: MIR → NIR lowering for the Nova language frontend.
pub mod error;
pub mod regalloc;
pub mod target;
pub mod risc;
pub mod cisc;
pub mod hybrid;
pub mod nova_lower;

use nova_nir::ir::Module;

pub use nova_lower::lower_mir_to_nir;
pub use nova_lower::NovaLowerer;
pub use target::CodegenMode;
pub use target::Target;

/// Generate MacroCore-X assembly from a NIR module.
///
/// # Arguments
/// * `module` - The parsed NIR module.
/// * `mode` - The code generation mode (Risc, Cisc, or Hybrid).
/// * `_target` - The target platform (currently uses defaults).
///
/// # Returns
/// The generated assembly source as a string.
pub fn generate_code(module: &Module, mode: CodegenMode, _target: Target) -> String {
    match mode {
        CodegenMode::Risc => {
            let mut gen = risc::RiscGenerator::new();
            gen.generate(module)
        }
        CodegenMode::Cisc => {
            let mut gen = cisc::CiscGenerator::new();
            gen.generate(module)
        }
        CodegenMode::Hybrid => {
            let mut gen = hybrid::HybridGenerator::new();
            gen.generate(module)
        }
    }
}