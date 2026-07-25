/// CLI binary for the nova-codegen crate.
///
/// Usage:
///   nova-codegen <input.nir> --mode hybrid --target workstation
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use nova_codegen::{generate_code, CodegenMode, Target};
use nova_nir::parser;

/// Code generation mode for the CLI.
#[derive(Clone, Copy, ValueEnum)]
enum CliMode {
    Risc,
    Cisc,
    Hybrid,
}

impl From<CliMode> for CodegenMode {
    fn from(m: CliMode) -> Self {
        match m {
            CliMode::Risc => CodegenMode::Risc,
            CliMode::Cisc => CodegenMode::Cisc,
            CliMode::Hybrid => CodegenMode::Hybrid,
        }
    }
}

/// Target platform for the CLI.
#[derive(Clone, Copy, ValueEnum)]
enum CliTarget {
    Mcu,
    Workstation,
    Pc,
}

impl From<CliTarget> for Target {
    fn from(t: CliTarget) -> Self {
        match t {
            CliTarget::Mcu => Target::Mcu,
            CliTarget::Workstation => Target::Workstation,
            CliTarget::Pc => Target::Pc,
        }
    }
}

#[derive(Parser)]
#[command(name = "nova-codegen")]
#[command(about = "NIR to MacroCore-X assembly code generator")]
struct Cli {
    /// Input .nir file
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// Code generation mode
    #[arg(short, long, value_enum, default_value = "hybrid")]
    mode: CliMode,

    /// Target platform
    #[arg(short, long, value_enum, default_value = "workstation")]
    target: CliTarget,

    /// Output assembly file (stdout if not specified)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Parse the input .nir file
    let module = parser::parse_file(&cli.input.to_string_lossy())?;

    // Generate assembly
    let mode: CodegenMode = cli.mode.into();
    let target: Target = cli.target.into();
    let asm = generate_code(&module, mode, target);

    // Output
    if let Some(ref out_path) = cli.output {
        std::fs::write(out_path, &asm)?;
        eprintln!("Wrote assembly to {}", out_path.display());
    } else {
        println!("{}", asm);
    }

    Ok(())
}