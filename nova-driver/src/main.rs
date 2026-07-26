//! novac: Nova Compiler Driver
//!
//! The main CLI entry point for the Nova compiler toolchain.
//! Supports:
//! - Nova source compilation to MacroCore-X (MCU/Workstation/PC)
//! - Native cross-compilation to x86_64, aarch64, x86, arm
//! - C/C++/Rust compilation via external toolchains
//! - Three code generation modes: RISC, CISC, Hybrid
//!
//! # Usage
//!
//! ```bash
//! novac input.nova -o output.bin --target pc
//! novac input.nova -o output --target x86_64
//! novac input.nova --target aarch64 -S -o output.asm
//! novac input.nova --target mcu -S -o output.asm
//! ```

use std::path::Path;
use std::process;

use clap::Parser;

use nova_frontend::parse_source;
use nova_hir::lower_and_check;
use nova_mir::lower_to_mir;
use nova_codegen::{lower_mir_to_nir, generate_code, Arch, CodegenMode, Target};
use nova_asm::assemble_source;
use nova_link::{LinkConfig, Linker, OutputFormat, ObjectFile, Section, Symbol};

// =============================================================================
//  CLI Definition
// =============================================================================

#[derive(Parser, Debug)]
#[command(
    name = "novac",
    version = env!("CARGO_PKG_VERSION"),
    author = "aaaaaswe",
    about = "Nova Compiler",
    long_about = "Compile Nova source to MacroCore-X or native machine code.\n\nTargets:\n  MacroCore-X: mcu, workstation, pc\n  Native:       x86_64, aarch64, x86, arm\n\nCodegen modes (MacroCore-X only):\n  risc, cisc, hybrid"
)]
pub struct Cli {
    #[arg(value_name = "FILE", required = true, num_args = 1..)]
    pub input: Vec<String>,

    #[arg(short = 'o', long = "output", value_name = "FILE")]
    pub output: Option<String>,

    /// Target platform.
    #[arg(short = 't', long = "target", default_value = "pc")]
    pub target: String,

    /// Code generation mode (MacroCore-X only).
    #[arg(long = "codegen", default_value = "hybrid")]
    pub codegen: String,

    /// Optimization level (0-3).
    #[arg(short = 'O', default_value = "1")]
    pub opt_level: u8,

    /// Output assembly only (do not assemble/link).
    #[arg(short = 'S', long = "asm")]
    pub emit_asm: bool,

    /// Output object file only (do not link).
    #[arg(short = 'c', long = "compile-only")]
    pub compile_only: bool,

    /// Output format (binary/elf).
    #[arg(long = "format")]
    pub output_format: Option<String>,

    /// Verbose output.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Emit NIR debug output.
    #[arg(long = "emit-nir")]
    pub emit_nir: bool,

    /// Emit MIR debug output.
    #[arg(long = "emit-mir")]
    pub emit_mir: bool,

    /// Run simulation after compilation (MacroCore-X only).
    #[arg(long = "run")]
    pub run: bool,

    /// Maximum simulation steps.
    #[arg(long = "max-steps", default_value = "10000")]
    pub max_steps: u64,
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

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let target: Target = cli.target.parse().map_err(|e| format!("bad target: {e}"))?;
    let codegen: CodegenMode = cli.codegen.parse().map_err(|e| format!("bad codegen: {e}"))?;
    let spec = target.spec();

    if cli.verbose {
        eprintln!("novac: target={target}, arch={}, codegen={codegen}, opt={}",
            target.arch(), cli.opt_level);
    }

    let input_path = &cli.input[0];
    let output_path = cli.output.as_deref().unwrap_or(if spec.is_native { "a.out" } else { "a.out" });

    let ext = Path::new(input_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "nova" => compile_nova(input_path, output_path, &cli, target, codegen),
        "c" | "i" | "cpp" | "cxx" | "cc" | "c++" => {
            compile_c(input_path, output_path, &cli, target)
        }
        "rs" => compile_rust(input_path, output_path, &cli, target),
        _ => compile_nova(input_path, output_path, &cli, target, codegen),
    }
}

// =============================================================================
//  Nova Compilation Pipeline
// =============================================================================

fn compile_nova(
    input: &str,
    output: &str,
    cli: &Cli,
    target: Target,
    codegen: CodegenMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(input)?;
    let spec = target.spec();

    if cli.verbose { eprintln!("novac: parsing {input}..."); }

    // Step 1: Parse → AST
    let ast = parse_source(&source).map_err(|e| format!("parse error: {e:?}"))?;

    if cli.verbose { eprintln!("novac: type checking..."); }

    // Step 2: AST → HIR
    let hir = lower_and_check(&ast).map_err(|e| format!("type error: {e:?}"))?;

    if cli.verbose { eprintln!("novac: lowering to MIR..."); }

    // Step 3: HIR → MIR
    let mir = lower_to_mir(&hir);
    if cli.emit_mir { eprintln!("=== MIR ===\n{mir:?}\n=== End MIR ==="); }

    if cli.verbose { eprintln!("novac: lowering to NIR..."); }

    // Step 4: MIR → NIR
    let nir = lower_mir_to_nir(&mir);
    if cli.emit_nir { eprintln!("=== NIR ===\n{nir}\n=== End NIR ==="); }

    if cli.verbose {
        if spec.is_native {
            eprintln!("novac: generating native code ({})...", spec.arch);
        } else {
            eprintln!("novac: generating code ({codegen})...");
        }
    }

    // Step 5: NIR → Assembly
    let asm = generate_code(&nir, codegen, target);

    if cli.emit_asm {
        std::fs::write(output, &asm)?;
        println!("Assembly written to {output}");
        return Ok(());
    }

    if spec.is_native {
        // ── Native pipeline: assemble with system as/ld ──
        compile_native(&asm, output, &spec, cli)
    } else {
        // ── MacroCore-X pipeline: internal assembler + linker ──
        compile_macrocorex(&asm, input, output, &spec, cli)
    }
}

/// Native compilation: write assembly, invoke system assembler + linker.
fn compile_native(
    asm: &str,
    output: &str,
    spec: &nova_codegen::target::TargetSpec,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let asm_file = format!("{output}.s");
    let obj_file = format!("{output}.o");

    std::fs::write(&asm_file, asm)?;

    if cli.verbose { eprintln!("novac: assembling with system as ({})...", spec.arch); }

    let as_cmd = match spec.arch {
        Arch::X86_64 => "as",
        Arch::Aarch64 => "aarch64-linux-gnu-as",
        Arch::X86 => "as",
        Arch::Arm => "arm-linux-gnueabihf-as",
        Arch::MacroCoreX => unreachable!(),
    };

    let as_status = process::Command::new(as_cmd)
        .args(["-o", &obj_file, &asm_file])
        .status();

    match as_status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            let _ = std::fs::remove_file(&asm_file);
            return Err(format!("assembler '{as_cmd}' failed with exit code {s}").into());
        }
        Err(_) => {
            // System assembler not available; write assembly as output
            if cli.verbose {
                eprintln!("novac: system assembler not found, writing assembly as output");
            }
            std::fs::rename(&asm_file, output)?;
            println!("Assembly written to {output} (install binutils to produce binary)");
            return Ok(());
        }
    }

    let _ = std::fs::remove_file(&asm_file);

    if cli.compile_only {
        std::fs::rename(&obj_file, output)?;
        println!("Object file written to {output}");
        return Ok(());
    }

    if cli.verbose { eprintln!("novac: linking with system ld..."); }

    let ld_cmd = match spec.arch {
        Arch::X86_64 => "ld",
        Arch::Aarch64 => "aarch64-linux-gnu-ld",
        Arch::X86 => "ld",
        Arch::Arm => "arm-linux-gnueabihf-ld",
        Arch::MacroCoreX => unreachable!(),
    };

    let ld_status = process::Command::new(ld_cmd)
        .args(["-o", output, &obj_file])
        .status();

    let _ = std::fs::remove_file(&obj_file);

    match ld_status {
        Ok(s) if s.success() => {
            println!("Executable written to {output}");
            Ok(())
        }
        Ok(s) => {
            Err(format!("linker '{ld_cmd}' failed with exit code {s}").into())
        }
        Err(e) => {
            Err(format!("failed to invoke linker '{ld_cmd}': {e}").into())
        }
    }
}

/// MacroCore-X compilation: internal assembler + linker.
fn compile_macrocorex(
    asm: &str,
    input: &str,
    output: &str,
    spec: &nova_codegen::target::TargetSpec,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    if cli.verbose { eprintln!("novac: assembling..."); }

    let binary = assemble_source(asm).map_err(|e| format!("assembly error: {e}"))?;

    if cli.compile_only {
        let obj = create_object_file(input, &spec.name, &binary);
        let serialized = obj.serialize();
        std::fs::write(output, &serialized)?;
        println!("Object file written to {output}");
        return Ok(());
    }

    let output_format = match cli.output_format.as_deref() {
        Some("elf") => OutputFormat::Elf,
        Some("binary") => OutputFormat::Binary,
        _ => match spec.output_format {
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

    if cli.verbose { eprintln!("novac: linking ({output_format:?})..."); }

    let executable = linker.link().map_err(|e| format!("link error: {e}"))?;
    std::fs::write(output, &executable)?;

    if cli.verbose {
        eprintln!("novac: executable written to {output} ({} bytes)", executable.len());
    } else {
        println!("Executable written to {output}");
    }

    if cli.run {
        run_simulation(&binary, cli.max_steps)?;
    }

    Ok(())
}

// =============================================================================
//  C/C++ Compilation
// =============================================================================

fn compile_c(
    input: &str,
    output: &str,
    cli: &Cli,
    target: Target,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = target.spec();

    if cli.verbose { eprintln!("novac: compiling C/C++ {input} for {target}..."); }

    let cc = std::env::var("NOVA_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "gcc".to_string());

    let obj_file = format!("{input}.o");
    let status = process::Command::new(&cc)
        .args(["-c", input, "-o", &obj_file, "-nostdinc", "-nostdlib", "-ffreestanding"])
        .status()
        .map_err(|e| format!("failed to invoke '{cc}': {e}"))?;

    if !status.success() {
        return Err(format!("'{cc}' failed").into());
    }

    let obj_data = std::fs::read(&obj_file)?;
    let _ = std::fs::remove_file(&obj_file);

    let obj = create_object_file(input, &spec.name, &obj_data);
    let output_format = match cli.output_format.as_deref() {
        Some("elf") => OutputFormat::Elf,
        _ => OutputFormat::Binary,
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
//  Rust Compilation
// =============================================================================

fn compile_rust(
    input: &str,
    output: &str,
    cli: &Cli,
    target: Target,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = target.spec();

    if cli.verbose { eprintln!("novac: compiling Rust {input} for {target}..."); }

    let rustc = std::env::var("NOVA_RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let obj_file = format!("{input}.o");

    let status = process::Command::new(&rustc)
        .args(["--emit", "obj", "-o", &obj_file, "-C", "panic=abort", input])
        .status()
        .map_err(|e| format!("failed to invoke rustc: {e}"))?;

    if !status.success() {
        return Err(format!("rustc failed").into());
    }

    let obj_data = std::fs::read(&obj_file)?;
    let _ = std::fs::remove_file(&obj_file);

    let obj = create_object_file(input, &spec.name, &obj_data);
    let output_format = match cli.output_format.as_deref() {
        Some("elf") => OutputFormat::Elf,
        _ => OutputFormat::Binary,
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
//  Simulation (MacroCore-X only)
// =============================================================================

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

fn create_object_file(name: &str, target: &str, data: &[u8]) -> ObjectFile {
    ObjectFile {
        name: name.to_string(),
        target: target.to_string(),
        sections: vec![Section {
            name: ".text".to_string(),
            data: data.to_vec(),
            flags: 7,
            alignment: 4,
        }],
        symbols: vec![Symbol {
            name: "_start".to_string(),
            section_index: 1,
            offset: 0,
            size: data.len() as u32,
            sym_type: 1,
            binding: 1,
        }],
        relocations: Vec::new(),
    }
}