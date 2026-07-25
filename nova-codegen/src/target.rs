/// Target specification system for MacroCore-X code generation.
///
/// Defines target-specific configurations for different deployment scenarios:
/// MCU, Workstation, and PC.
use serde::{Deserialize, Serialize};

/// Memory layout configuration for a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLayout {
    /// Start address of code (.text) section.
    pub code_start: u64,
    /// Start address of data section.
    pub data_start: u64,
    /// Start address of stack.
    pub stack_start: u64,
    /// Maximum stack size in bytes.
    pub stack_size: u64,
    /// Start address of heap.
    pub heap_start: u64,
}

/// ABI configuration for a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbiConfig {
    /// Calling convention name.
    pub call_conv: String,
    /// Number of parameter registers.
    pub param_regs: usize,
    /// Number of return value registers.
    pub return_regs: usize,
    /// Stack alignment in bytes.
    pub stack_alignment: usize,
}

/// Target specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetSpec {
    /// Target name.
    pub name: String,
    /// Pointer width in bits.
    pub pointer_width: u32,
    /// Endianness.
    pub endianness: Endianness,
    /// Total number of physical registers.
    pub register_count: u32,
    /// Available instruction categories.
    pub available_instructions: Vec<String>,
    /// Memory layout.
    pub memory_layout: MemoryLayout,
    /// ABI configuration.
    pub abi: AbiConfig,
    /// Preferred code generation mode.
    pub preferred_mode: CodegenMode,
    /// Whether CISC composite instructions are available.
    pub cisc_enabled: bool,
    /// Output format preference.
    pub output_format: OutputFormat,
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
    /// Pure RISC mode: expand all composite instructions.
    Risc,
    /// Pure CISC mode: use composite instructions directly.
    Cisc,
    /// Hybrid mode: intelligently select between RISC and CISC.
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
            _ => Err(format!("unknown codegen mode: {}", s)),
        }
    }
}

/// Output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    /// Flat binary.
    Binary,
    /// ELF executable.
    Elf,
    /// Assembly source.
    Assembly,
}

/// Target platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// Microcontroller target: RISC mode preferred, small code model,
    /// memory from 0x08000000.
    Mcu,
    /// Workstation target: Hybrid mode, all CISC enabled, ELF output.
    Workstation,
    /// PC target: Hybrid mode, balanced, memory from 0x1000.
    Pc,
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Mcu => write!(f, "mcu"),
            Target::Workstation => write!(f, "workstation"),
            Target::Pc => write!(f, "pc"),
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
            _ => Err(format!("unknown target: {}", s)),
        }
    }
}

impl Target {
    /// Get the target specification for this target.
    pub fn spec(&self) -> TargetSpec {
        match self {
            Target::Mcu => TargetSpec {
                name: "macrocore-x-mcu".to_string(),
                pointer_width: 32,
                endianness: Endianness::Little,
                register_count: 32,
                available_instructions: vec![
                    "r_type".to_string(),
                    "i_type".to_string(),
                    "l4_type".to_string(),
                    "b_type".to_string(),
                    "sys2".to_string(),
                ],
                memory_layout: MemoryLayout {
                    code_start: 0x0800_0000,
                    data_start: 0x2000_0000,
                    stack_start: 0x2001_0000,
                    stack_size: 0x10000,
                    heap_start: 0x2000_8000,
                },
                abi: AbiConfig {
                    call_conv: "nova".to_string(),
                    param_regs: 7,
                    return_regs: 1,
                    stack_alignment: 8,
                },
                preferred_mode: CodegenMode::Risc,
                cisc_enabled: false,
                output_format: OutputFormat::Binary,
            },
            Target::Workstation => TargetSpec {
                name: "macrocore-x-ws".to_string(),
                pointer_width: 64,
                endianness: Endianness::Little,
                register_count: 32,
                available_instructions: vec![
                    "r_type".to_string(),
                    "i_type".to_string(),
                    "l4_type".to_string(),
                    "l6_type".to_string(),
                    "b_type".to_string(),
                    "c_type".to_string(),
                    "v_type".to_string(),
                    "f_type".to_string(),
                    "sys2".to_string(),
                    "sys4".to_string(),
                ],
                memory_layout: MemoryLayout {
                    code_start: 0x400000,
                    data_start: 0x600000,
                    stack_start: 0x7FFF_0000,
                    stack_size: 0x10000,
                    heap_start: 0x601000,
                },
                abi: AbiConfig {
                    call_conv: "nova".to_string(),
                    param_regs: 7,
                    return_regs: 1,
                    stack_alignment: 16,
                },
                preferred_mode: CodegenMode::Hybrid,
                cisc_enabled: true,
                output_format: OutputFormat::Elf,
            },
            Target::Pc => TargetSpec {
                name: "macrocore-x-pc".to_string(),
                pointer_width: 64,
                endianness: Endianness::Little,
                register_count: 32,
                available_instructions: vec![
                    "r_type".to_string(),
                    "i_type".to_string(),
                    "l4_type".to_string(),
                    "l6_type".to_string(),
                    "b_type".to_string(),
                    "c_type".to_string(),
                    "v_type".to_string(),
                    "f_type".to_string(),
                    "sys2".to_string(),
                    "sys4".to_string(),
                ],
                memory_layout: MemoryLayout {
                    code_start: 0x1000,
                    data_start: 0x10000,
                    stack_start: 0x7FFF_0000,
                    stack_size: 0x10000,
                    heap_start: 0x20000,
                },
                abi: AbiConfig {
                    call_conv: "nova".to_string(),
                    param_regs: 7,
                    return_regs: 1,
                    stack_alignment: 16,
                },
                preferred_mode: CodegenMode::Hybrid,
                cisc_enabled: true,
                output_format: OutputFormat::Binary,
            },
        }
    }
}