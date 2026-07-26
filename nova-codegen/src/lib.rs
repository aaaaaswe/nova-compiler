/// nova-codegen: NIR to assembly code generation.
///
/// Provides code generation backends for:
/// ── MacroCore-X ──
/// - **RISC**: Pure RISC mode, expands all composite instructions.
/// - **CISC**: Pure CISC mode, uses composite instructions directly.
/// - **Hybrid**: Dynamically selects RISC/CISC based on register pressure.
///
/// ── Native ──
/// - **x86_64**: AMD64 GAS/AT&T assembly, System V ABI.
/// - **aarch64**: ARM64 GAS assembly, AAPCS64.
/// - **x86**: IA-32 (uses x86_64 backend).
/// - **arm**: ARM32 (uses aarch64 backend).
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
pub mod native;

use nova_nir::ir::Module;

pub use nova_lower::lower_mir_to_nir;
pub use nova_lower::NovaLowerer;
pub use target::Arch;
pub use target::CodegenMode;
pub use target::Target;

/// Generate assembly from a NIR module.
pub fn generate_code(module: &Module, mode: CodegenMode, target: Target) -> String {
    let spec = target.spec();

    if spec.is_native {
        return native::generate_native(module, &spec);
    }

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