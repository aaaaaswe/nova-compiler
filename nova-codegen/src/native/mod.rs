/// Native code generation backends for real architectures.
///
/// Maps NIR instructions to native assembly for:
/// - x86_64 (AMD64) — GAS/AT&T syntax, System V ABI
/// - aarch64 (ARM64) — GAS syntax, AAPCS64
/// - x86 (IA-32) — aliased to x86_64 (with 32-bit output)
/// - arm (ARM32) — aliased to aarch64 (with 32-bit output)
pub mod x86_64;
pub mod aarch64;

use nova_nir::ir::Module;
use crate::target::{Arch, TargetSpec};

pub use x86_64::X86_64Generator;
pub use aarch64::Aarch64Generator;

/// Generate native assembly for the given target architecture.
pub fn generate_native(module: &Module, spec: &TargetSpec) -> String {
    match spec.arch {
        Arch::X86_64 | Arch::X86 => {
            let mut gen = X86_64Generator::new(spec.clone());
            gen.generate(module)
        }
        Arch::Aarch64 | Arch::Arm => {
            let mut gen = Aarch64Generator::new(spec.clone());
            gen.generate(module)
        }
        Arch::MacroCoreX => {
            // Should not be called for MacroCoreX; use the existing codegen path.
            String::from("; Native codegen not applicable for MacroCoreX\n")
        }
    }
}