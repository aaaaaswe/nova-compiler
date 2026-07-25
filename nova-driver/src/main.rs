//! novac: Nova Compiler Driver
//!
//! The main CLI entry point for the Nova compiler toolchain.
//! Supports:
//! - Nova source compilation to MacroCore-X
//! - C/C++/Rust compilation via external toolchains
//! - Three code generation modes: RISC, CISC, Hybrid
//! - Cross-compilation: MCU, Workstation, PC
//! - Output formats: flat binary, ELF, assembly
//!
//! # Usage
//!
//! ```bash
//! novac input.nova -o output.bin --target pc --codegen hybrid
//! novac input.nova -o output.elf --target workstation
//! novac input.nova --target mcu -S -o output.asm
//! novac input.c --target pc --cc gcc -o output.bin
//! ```

use std::path::Path;
use std::process;

use clap::{Parser, ValueEnum};

use nova_frontend::parse_source;
use nova_hir::lower_and_check;
use nova_mir::lower_to_mir;
use nova_codegen::{lower_mir_to_nir, generate_code, CodegenMode, Target};
use nova_asm::assemble_source;
use nova_link::{LinkConfig, Linker, OutputFormat, ObjectFile, Section, Symbol};

// =============================================================================
//  CLI Definition
// =============================================================================

/// Nova Compiler: compile Nova/C/C++/Rust source to MacroCore-X machine code.
#[derive(Parser, Debug)]
#[command(
    name = "novac",
    version = env!("CARGO_PKG_VERSION"),
    author = "aaaaaswe",
    about = "Nova Compiler for MacroCore-X",
    long_about = "Compile Nova, C, C++, or Rust source code to MacroCore-X machine code.\n\nSupports three code generation modes:\n  - risc:   Pure RISC instructions\n  - cisc:   Pure CISC instructions (composite ops)\n  - hybrid: Intelligent RISC/CISC selection (default)\n\nTarget platforms:\n  - mcu:         Microcontroller (0x08000000, binary output)\n  - workstation: Workstation/Server (ELF output, CISC+FP enabled)\n  - pc:          Personal Computer (0x1000, binary output)"
)]
pub struct Cli {
    /// Input source file(s).
    #[arg(value_name = "FILE", required = true, num_args = 1..)]
    pub input: Vec<String>,

    /// Output file.
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    pub output: Option<String>,

    /// Target platform.
    #[arg(short = 't', long = "target", value_enum, default_value = "pc")]
    pub target: CliTarget,

    /// Code generation mode.
    #[arg(long = "codegen", value_enum, default_value = "hybrid")]
    pub codegen: CliCodegen,

    /// Optimization level (0-3).
    #[arg(short = 'O', default_value = "1")]
    pub opt_level: u8,

    /// Output assembly only (do not assemble/link).
    #[arg(short = 'S', long = "asm")]
    pub emit_asm: bool,

    /// Output object file only (do not link).
    #[arg(short = 'c', long = "compile-only")]
    pub compile_only: bool,

    /// Output format.
    #[arg(long = "format", value_enum)]
    pub output_format: Option<CliOutputFormat>,

    /// Verbose output.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Emit NIR debug output.
    #[arg(long = "emit-nir")]
    pub emit_nir: bool,

    /// Emit MIR debug output.
    #[arg(long = "emit-mir")]
    pub emit_mir: bool,

    /// Run simulation after compilation.
    #[arg(long = "run")]
    pub run: bool,

    /// Maximum simulation steps (for --run).
    #[arg(long = "max-steps", default_value = "10000")]
    pub max_steps: u64,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliTarget {
    /// Microcontroller (RISC, binary, 0x08000000)
    Mcu,
    /// Workstation (Hybrid, ELF, CISC+FP)
    Workstation,
    /// Personal Computer (Hybrid, binary, 0x1000)
    Pc,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliCodegen {
    /// Pure RISC mode
    Risc,
    /// Pure CISC mode
    Cisc,
    /// Hybrid RISC/CISC (default)
    Hybrid,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliOutputFormat {
    /// Flat binary
    Binary,
    /// ELF executable
    Elf,
}

// =============================================================================
//  Main entry point
// =============================================================================

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("novac: error: {e}");
        process::exit(1);
    }
}

/// Run the compilation pipeline.
fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    // Convert CLI types to internal types
    let target: Target = match cli.target {
        CliTarget::Mcu => Target::Mcu,
        CliTarget::Workstation => Target::Workstation,
        CliTarget::Pc => Target::Pc,
    };

    let codegen: CodegenMode = match cli.codegen {
        CliCodegen::Risc => CodegenMode::Risc,
        CliCodegen::Cisc => CodegenMode::Cisc,
        CliCodegen::Hybrid => CodegenMode::Hybrid,
    };

    if cli.verbose {
        eprintln!("novac: target={target}, codegen={codegen}, opt={}", cli.opt_level);
    }

    // Determine file type from extension
    let input_path = &cli.input[0];
    let ext = Path::new(input_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let output_path = cli.output.as_deref().unwrap_or("a.out");

    match ext.as_str() {
        "nova" => compile_nova(input_path, output_path, &cli, target, codegen),
        "c" | "i" | "cpp" | "cxx" | "cc" | "c++" => {
            compile_c(input_path, output_path, &cli, target, codegen)
        }
        "rs" => compile_rust(input_path, output_path, &cli, target, codegen),
        _ => {
            // Try to detect as Nova source
            compile_nova(input_path, output_path, &cli, target, codegen)
        }
    }
}

// =============================================================================
//  Nova Compilation Pipeline
// =============================================================================

/// Compile a Nova source file through the full pipeline.
fn compile_nova(
    input: &str,
    output: &str,
    cli: &Cli,
    target: Target,
    codegen: CodegenMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(input)?;
    let spec = target.spec();

    if cli.verbose {
        eprintln!("novac: parsing {input}...");
    }

    // Step 1: Parse Nova source → AST
    let ast = parse_source(&source).map_err(|e| format!("parse error: {e:?}"))?;

    if cli.verbose {
        eprintln!("novac: type checking...");
    }

    // Step 2: AST → HIR (with type checking)
    let hir = lower_and_check(&ast).map_err(|e| format!("type error: {e:?}"))?;

    if cli.verbose {
        eprintln!("novac: lowering to MIR...");
    }

    // Step 3: HIR → MIR
    let mir = lower_to_mir(&hir);

    if cli.emit_mir {
        eprintln!("=== MIR ===\n{mir:?}\n=== End MIR ===");
    }

    if cli.verbose {
        eprintln!("novac: lowering to NIR...");
    }

    // Step 4: MIR → NIR
    let nir = lower_mir_to_nir(&mir);

    if cli.emit_nir {
        eprintln!("=== NIR ===\n{nir}\n=== End NIR ===");
    }

    if cli.verbose {
        eprintln!("novac: generating code ({codegen})...");
    }

    // Step 5: NIR → Assembly
    let asm = generate_code(&nir, codegen, target);

    if cli.emit_asm {
        // Write assembly to output file
        std::fs::write(output, &asm)?;
        if cli.verbose {
            eprintln!("novac: assembly written to {output}");
        } else {
            println!("Assembly written to {output}");
        }
        return Ok(());
    }

    if cli.verbose {
        eprintln!("novac: assembling...");
    }

    // Step 6: Assemble → binary
    let binary = assemble_source(&asm).map_err(|e| format!("assembly error: {e}"))?;

    if cli.compile_only {
        // Write object file
        let obj = create_object_file(input, &spec.name, &binary);
        let serialized = obj.serialize();
        std::fs::write(output, &serialized)?;
        if cli.verbose {
            eprintln!("novac: object file written to {output}");
        } else {
            println!("Object file written to {output}");
        }
        return Ok(());
    }

    // Step 7: Link → executable
    let output_format = match cli.output_format {
        Some(CliOutputFormat::Elf) => OutputFormat::Elf,
        Some(CliOutputFormat::Binary) => OutputFormat::Binary,
        None => match spec.output_format {
            nova_codegen::target::OutputFormat::Elf => OutputFormat::Elf,
            _ => OutputFormat::Binary,
        },
    };

    let config = LinkConfig {
        format: output_format,
        text_base: spec.memory_layout.code_start,
        data_base: spec.memory_layout.data_start,
        entry: "_start".to_string(),
        output: output.to_string(),
    };

    let mut linker = Linker::new(config);
    let obj = create_object_file(input, &spec.name, &binary);
    linker.add_object(obj);

    if cli.verbose {
        eprintln!("novac: linking ({output_format:?})...");
    }

    let executable = linker.link().map_err(|e| format!("link error: {e}"))?;
    std::fs::write(output, &executable)?;

    if cli.verbose {
        let size = executable.len();
        let sections = if matches!(output_format, OutputFormat::Elf) {
            "ELF"
        } else {
            "binary"
        };
        eprintln!("novac: {sections} executable written to {output} ({size} bytes)");
    } else {
        println!("Executable written to {output}");
    }

    // Step 8 (optional): Run simulation
    if cli.run {
        run_simulation(&binary, cli.max_steps)?;
    }

    Ok(())
}

// =============================================================================
//  C/C++ Compilation (via external toolchain)
// =============================================================================

/// Compile C/C++ source using an external compiler and link with Nova objects.
fn compile_c(
    input: &str,
    output: &str,
    cli: &Cli,
    target: Target,
    _codegen: CodegenMode,
) -> Result<(), Box<dyn std::error::Error>> {
    // For C/C++ compilation, we use an external cross-compiler (e.g., GCC cross)
    // and then link with Nova libraries.
    let spec = target.spec();

    if cli.verbose {
        eprintln!("novac: compiling C/C++ source {input} for {target}...");
    }

    // Determine the C compiler
    let cc = std::env::var("NOVA_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "gcc".to_string());

    // For now, invoke the external compiler to produce a Nova object file
    // In a full implementation, this would use a cross-compiler targeting MacroCore-X
    let obj_file = format!("{input}.o");

    let status = process::Command::new(&cc)
        .args([
            "-c",
            input,
            "-o", &obj_file,
            "-target", &spec.name,
            "-nostdinc",
            "-nostdlib",
        ])
        .status()
        .map_err(|e| format!("failed to invoke C compiler '{cc}': {e}"))?;

    if !status.success() {
        return Err(format!("C compiler '{cc}' failed with exit code {status}").into());
    }

    // Read the object file
    let obj_data = std::fs::read(&obj_file)?;
    let _ = std::fs::remove_file(&obj_file);

    // Create Nova object file
    let obj = create_object_file(input, &spec.name, &obj_data);

    // Link
    let output_format = match cli.output_format {
        Some(CliOutputFormat::Elf) => OutputFormat::Elf,
        Some(CliOutputFormat::Binary) => OutputFormat::Binary,
        None => match spec.output_format {
            nova_codegen::target::OutputFormat::Elf => OutputFormat::Elf,
            _ => OutputFormat::Binary,
        },
    };

    let config = LinkConfig {
        format: output_format,
        text_base: spec.memory_layout.code_start,
        data_base: spec.memory_layout.data_start,
        entry: "_start".to_string(),
        output: output.to_string(),
    };

    let mut linker = Linker::new(config);
    linker.add_object(obj);
    let executable = linker.link().map_err(|e| format!("link error: {e}"))?;
    std::fs::write(output, &executable)?;

    println!("C/C++ executable written to {output}");
    Ok(())
}

// =============================================================================
//  Rust Compilation (via external toolchain)
// =============================================================================

/// Compile Rust source using an external compiler and link with Nova objects.
fn compile_rust(
    input: &str,
    output: &str,
    cli: &Cli,
    target: Target,
    _codegen: CodegenMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = target.spec();

    if cli.verbose {
        eprintln!("novac: compiling Rust source {input} for {target}...");
    }

    // Use rustc with a custom target specification
    let rustc = std::env::var("NOVA_RUSTC")
        .unwrap_or_else(|_| "rustc".to_string());

    let obj_file = format!("{input}.o");

    let status = process::Command::new(&rustc)
        .args([
            "--emit", "obj",
            "-o", &obj_file,
            "--target", &spec.name,
            "-C", "panic=abort",
            "-C", "linker=novac",
            input,
        ])
        .status()
        .map_err(|e| format!("failed to invoke rustc: {e}"))?;

    if !status.success() {
        return Err(format!("rustc failed with exit code {status}").into());
    }

    // Read the object file
    let obj_data = std::fs::read(&obj_file)?;
    let _ = std::fs::remove_file(&obj_file);

    let obj = create_object_file(input, &spec.name, &obj_data);

    let output_format = match cli.output_format {
        Some(CliOutputFormat::Elf) => OutputFormat::Elf,
        Some(CliOutputFormat::Binary) => OutputFormat::Binary,
        None => match spec.output_format {
            nova_codegen::target::OutputFormat::Elf => OutputFormat::Elf,
            _ => OutputFormat::Binary,
        },
    };

    let config = LinkConfig {
        format: output_format,
        text_base: spec.memory_layout.code_start,
        data_base: spec.memory_layout.data_start,
        entry: "_start".to_string(),
        output: output.to_string(),
    };

    let mut linker = Linker::new(config);
    linker.add_object(obj);
    let executable = linker.link().map_err(|e| format!("link error: {e}"))?;
    std::fs::write(output, &executable)?;

    println!("Rust executable written to {output}");
    Ok(())
}

// =============================================================================
//  Simulation
// =============================================================================

/// Run the compiled binary in the simulator.
fn run_simulation(binary: &[u8], max_steps: u64) -> Result<(), Box<dyn std::error::Error>> {
    use nova_sim::Cpu;

    let mut cpu = Cpu::new();
    cpu.load_binary(binary);
    match cpu.run_with_limit(false, max_steps) {
        Ok(steps) => {
            println!("Simulation completed in {steps} steps. R1 (return value) = {}", cpu.r[1]);
            Ok(())
        }
        Err(e) => {
            eprintln!("Simulation error: {e}");
            eprintln!("CPU state: PC=0x{:08X}, R1={}", cpu.pc, cpu.r[1]);
            Err(e)
        }
    }
}

// =============================================================================
//  Helpers
// =============================================================================

/// Create a Nova object file from a binary blob.
fn create_object_file(name: &str, target: &str, data: &[u8]) -> ObjectFile {
    ObjectFile {
        name: name.to_string(),
        target: target.to_string(),
        sections: vec![Section {
            name: ".text".to_string(),
            data: data.to_vec(),
            flags: 7,   // alloc + exec + write
            alignment: 4,
        }],
        symbols: vec![Symbol {
            name: "_start".to_string(),
            section_index: 1,
            offset: 0,
            size: data.len() as u32,
            sym_type: 1, // func
            binding: 1,  // global
        }],
        relocations: Vec::new(),
    }
}