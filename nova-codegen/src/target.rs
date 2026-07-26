/// Target specification system for Nova code generation.
///
/// Supports MacroCore-X targets (MCU, Workstation, PC) and native
/// cross-compilation targets (x86_64, aarch64, x86, arm).
use serde::{Deserialize, Serialize};

/// Target architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arch {
    /// MacroCore-X custom ISA.
    MacroCoreX,
    /// x86-64 (AMD64).
    X86_64,
    /// AArch64 (ARM64).
    Aarch64,
    /// x86 32-bit (IA-32).
    X86,
    /// ARM 32-bit.
    Arm,
}

impl std::fmt::Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Arch::MacroCoreX => write!(f, "macrocorex"),
            Arch::X86_64 => write!(f, "x86_64"),
            Arch::Aarch64 => write!(f, "aarch64"),
            Arch::X86 => write!(f, "x86"),
            Arch::Arm => write!(f, "arm"),
        }
    }
}

impl std::str::FromStr for Arch {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "macrocorex" | "macrocore-x" | "mcu" | "workstation" | "pc" => Ok(Arch::MacroCoreX),
            "x86_64" | "x64" | "amd64" => Ok(Arch::X86_64),
            "aarch64" | "arm64" => Ok(Arch::Aarch64),
            "x86" | "i386" | "i686" | "ia32" => Ok(Arch::X86),
            "arm" | "arm32" | "armv7" => Ok(Arch::Arm),
            _ => Err(format!("unknown architecture: {s}")),
        }
    }
}

/// Memory layout configuration for a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLayout {
    pub code_start: u64,
    pub data_start: u64,
    pub stack_start: u64,
    pub stack_size: u64,
    pub heap_start: u64,
}

/// ABI configuration for a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbiConfig {
    pub call_conv: String,
    pub param_regs: usize,
    pub return_regs: usize,
    pub stack_alignment: usize,
}

/// Target specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetSpec {
    pub name: String,
    pub arch: Arch,
    pub pointer_width: u32,
    pub endianness: Endianness,
    pub register_count: u32,
    pub available_instructions: Vec<String>,
    pub memory_layout: MemoryLayout,
    pub abi: AbiConfig,
    pub preferred_mode: CodegenMode,
    pub cisc_enabled: bool,
    pub output_format: OutputFormat,
    /// Whether this is a native target (uses system assembler/linker).
    pub is_native: bool,
}

/// Endianness of the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Endianness {
    Little,
    Big,
}

/// Code generation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodegenMode {
    Risc,
    Cisc,
    Hybrid,
}

impl std::fmt::Display for CodegenMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenMode::Risc => write!(f, "risc"),
            CodegenMode::Cisc => write!(f, "cisc"),
            CodegenMode::Hybrid => write!(f, "hybrid"),
        }
    }
}

impl std::str::FromStr for CodegenMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "risc" => Ok(CodegenMode::Risc),
            "cisc" => Ok(CodegenMode::Cisc),
            "hybrid" => Ok(CodegenMode::Hybrid),
            _ => Err(format!("unknown codegen mode: {s}")),
        }
    }
}

/// Output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    Binary,
    Elf,
    Assembly,
}

/// Target platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    // ── MacroCore-X targets ──
    Mcu,
    Workstation,
    Pc,
    // ── Native targets ──
    X86_64,
    Aarch64,
    X86,
    Arm,
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Mcu => write!(f, "mcu"),
            Target::Workstation => write!(f, "workstation"),
            Target::Pc => write!(f, "pc"),
            Target::X86_64 => write!(f, "x86_64"),
            Target::Aarch64 => write!(f, "aarch64"),
            Target::X86 => write!(f, "x86"),
            Target::Arm => write!(f, "arm"),
        }
    }
}

impl std::str::FromStr for Target {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mcu" => Ok(Target::Mcu),
            "workstation" => Ok(Target::Workstation),
            "pc" => Ok(Target::Pc),
            "x86_64" | "x64" | "amd64" => Ok(Target::X86_64),
            "aarch64" | "arm64" => Ok(Target::Aarch64),
            "x86" | "i386" | "i686" | "ia32" => Ok(Target::X86),
            "arm" | "arm32" | "armv7" => Ok(Target::Arm),
            _ => Err(format!("unknown target: {s}")),
        }
    }
}

impl Target {
    pub fn arch(&self) -> Arch {
        match self {
            Target::Mcu | Target::Workstation | Target::Pc => Arch::MacroCoreX,
            Target::X86_64 => Arch::X86_64,
            Target::Aarch64 => Arch::Aarch64,
            Target::X86 => Arch::X86,
            Target::Arm => Arch::Arm,
        }
    }

    pub fn is_native(&self) -> bool {
        !matches!(self.arch(), Arch::MacroCoreX)
    }

    pub fn spec(&self) -> TargetSpec {
        match self {
            // ── MacroCore-X targets ──
            Target::Mcu => TargetSpec {
                name: "macrocore-x-mcu".to_string(),
                arch: Arch::MacroCoreX,
                pointer_width: 32,
                endianness: Endianness::Little,
                register_count: 32,
                available_instructions: vec![
                    "r_type".into(), "i_type".into(), "l4_type".into(),
                    "b_type".into(), "sys2".into(),
                ],
                memory_layout: MemoryLayout {
                    code_start: 0x0800_0000, data_start: 0x2000_0000,
                    stack_start: 0x2001_0000, stack_size: 0x10000, heap_start: 0x2000_8000,
                },
                abi: AbiConfig {
                    call_conv: "nova".into(), param_regs: 7, return_regs: 1, stack_alignment: 8,
                },
                preferred_mode: CodegenMode::Risc,
                cisc_enabled: false,
                output_format: OutputFormat::Binary,
                is_native: false,
            },
            Target::Workstation => TargetSpec {
                name: "macrocore-x-ws".to_string(),
                arch: Arch::MacroCoreX,
                pointer_width: 64,
                endianness: Endianness::Little,
                register_count: 32,
                available_instructions: vec![
                    "r_type".into(), "i_type".into(), "l4_type".into(), "l6_type".into(),
                    "b_type".into(), "c_type".into(), "v_type".into(), "f_type".into(),
                    "sys2".into(), "sys4".into(),
                ],
                memory_layout: MemoryLayout {
                    code_start: 0x400000, data_start: 0x600000,
                    stack_start: 0x7FFF_0000, stack_size: 0x10000, heap_start: 0x601000,
                },
                abi: AbiConfig {
                    call_conv: "nova".into(), param_regs: 7, return_regs: 1, stack_alignment: 16,
                },
                preferred_mode: CodegenMode::Hybrid,
                cisc_enabled: true,
                output_format: OutputFormat::Elf,
                is_native: false,
            },
            Target::Pc => TargetSpec {
                name: "macrocore-x-pc".to_string(),
                arch: Arch::MacroCoreX,
                pointer_width: 64,
                endianness: Endianness::Little,
                register_count: 32,
                available_instructions: vec![
                    "r_type".into(), "i_type".into(), "l4_type".into(), "l6_type".into(),
                    "b_type".into(), "c_type".into(), "v_type".into(), "f_type".into(),
                    "sys2".into(), "sys4".into(),
                ],
                memory_layout: MemoryLayout {
                    code_start: 0x1000, data_start: 0x10000,
                    stack_start: 0x7FFF_0000, stack_size: 0x10000, heap_start: 0x20000,
                },
                abi: AbiConfig {
                    call_conv: "nova".into(), param_regs: 7, return_regs: 1, stack_alignment: 16,
                },
                preferred_mode: CodegenMode::Hybrid,
                cisc_enabled: true,
                output_format: OutputFormat::Binary,
                is_native: false,
            },
            // ── Native: x86_64 ──
            Target::X86_64 => TargetSpec {
                name: "x86_64-unknown-linux-gnu".to_string(),
                arch: Arch::X86_64,
                pointer_width: 64,
                endianness: Endianness::Little,
                register_count: 16,
                available_instructions: vec![],
                memory_layout: MemoryLayout {
                    code_start: 0x400000, data_start: 0x600000,
                    stack_start: 0x7FFF_0000, stack_size: 0x80000, heap_start: 0x601000,
                },
                abi: AbiConfig {
                    call_conv: "systemv".into(), param_regs: 6, return_regs: 1, stack_alignment: 16,
                },
                preferred_mode: CodegenMode::Hybrid,
                cisc_enabled: true,
                output_format: OutputFormat::Assembly,
                is_native: true,
            },
            // ── Native: aarch64 ──
            Target::Aarch64 => TargetSpec {
                name: "aarch64-unknown-linux-gnu".to_string(),
                arch: Arch::Aarch64,
                pointer_width: 64,
                endianness: Endianness::Little,
                register_count: 31,
                available_instructions: vec![],
                memory_layout: MemoryLayout {
                    code_start: 0x400000, data_start: 0x600000,
                    stack_start: 0x7FFF_0000, stack_size: 0x80000, heap_start: 0x601000,
                },
                abi: AbiConfig {
                    call_conv: "aapcs64".into(), param_regs: 8, return_regs: 1, stack_alignment: 16,
                },
                preferred_mode: CodegenMode::Hybrid,
                cisc_enabled: false,
                output_format: OutputFormat::Assembly,
                is_native: true,
            },
            // ── Native: x86 ──
            Target::X86 => TargetSpec {
                name: "i686-unknown-linux-gnu".to_string(),
                arch: Arch::X86,
                pointer_width: 32,
                endianness: Endianness::Little,
                register_count: 8,
                available_instructions: vec![],
                memory_layout: MemoryLayout {
                    code_start: 0x08048000, data_start: 0x08049000,
                    stack_start: 0xBF80_0000, stack_size: 0x80000, heap_start: 0x0804A000,
                },
                abi: AbiConfig {
                    call_conv: "cdecl".into(), param_regs: 0, return_regs: 1, stack_alignment: 16,
                },
                preferred_mode: CodegenMode::Hybrid,
                cisc_enabled: true,
                output_format: OutputFormat::Assembly,
                is_native: true,
            },
            // ── Native: arm ──
            Target::Arm => TargetSpec {
                name: "armv7-unknown-linux-gnueabihf".to_string(),
                arch: Arch::Arm,
                pointer_width: 32,
                endianness: Endianness::Little,
                register_count: 16,
                available_instructions: vec![],
                memory_layout: MemoryLayout {
                    code_start: 0x8000, data_start: 0x10000,
                    stack_start: 0x7E00_0000, stack_size: 0x80000, heap_start: 0x20000,
                },
                abi: AbiConfig {
                    call_conv: "aapcs".into(), param_regs: 4, return_regs: 1, stack_alignment: 8,
                },
                preferred_mode: CodegenMode::Hybrid,
                cisc_enabled: false,
                output_format: OutputFormat::Assembly,
                is_native: true,
            },
        }
    }
}