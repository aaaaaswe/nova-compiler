/// MacroCore-X assembler CLI.
///
/// Usage: nova-asm input.asm [-o output.bin]
use std::env;
use std::fs;
use std::process;

use nova_asm::{tokenize, parse, Assembler};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} input.asm [-o output.bin]", args[0]);
        process::exit(1);
    }

    let input_path = &args[1];
    let output_path = if let Some(pos) = args.iter().position(|a| a == "-o") {
        if pos + 1 < args.len() {
            args[pos + 1].clone()
        } else {
            eprintln!("Error: -o requires an output path");
            process::exit(1);
        }
    } else {
        // Default: replace .asm with .bin
        input_path
            .rsplit('.')
            .nth(1)
            .map(|stem| format!("{}.bin", stem))
            .unwrap_or_else(|| format!("{}.bin", input_path))
    };

    let source = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", input_path, e);
            process::exit(1);
        }
    };

    // Tokenize and parse
    let tokens = tokenize(&source);
    let instructions = match parse(&tokens) {
        Ok(insts) => insts,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    };

    // Assemble
    let mut assembler = Assembler::default();
    let binary = match assembler.assemble(&instructions) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Assembly error: {}", e);
            process::exit(1);
        }
    };

    // Write output
    if let Err(e) = fs::write(&output_path, &binary) {
        eprintln!("Error writing {}: {}", output_path, e);
        process::exit(1);
    }

    println!(
        "Assembled {} instructions → {} bytes",
        instructions.len(),
        binary.len()
    );
    println!("Output: {}", output_path);

    // Print hex dump
    println!("\nHex dump:");
    for (i, chunk) in binary.chunks(16).enumerate() {
        let hex_str: String = chunk.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
        let ascii_str: String = chunk
            .iter()
            .map(|&b| if (32..127).contains(&b) { b as char } else { '.' })
            .collect();
        println!("  {:04x}: {:<48} {}", i * 16, hex_str, ascii_str);
    }
}