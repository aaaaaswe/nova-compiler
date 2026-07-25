use clap::Parser;
use nova_sim::Cpu;
use std::path::PathBuf;

/// MacroCore-X Simulator
#[derive(Parser)]
#[command(name = "nova-sim")]
#[command(about = "MacroCore-X ISA Simulator")]
struct Args {
    /// Binary file to execute
    binary: PathBuf,

    /// Enable debug trace mode
    #[arg(short = 'd')]
    debug: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let data = std::fs::read(&args.binary)?;

    let mut cpu = Cpu::new();
    cpu.load_binary(&data);

    let exit_code = cpu.run(args.debug)?;

    std::process::exit(exit_code as i32);
}