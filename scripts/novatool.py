#!/usr/bin/env python3
"""
novatool.py - Nova Compiler Toolchain Python Wrapper

Unified CLI tool for the Nova compiler ecosystem. Wraps the Rust-based
`novac`, `nova-asm`, and `nova-sim` tools with a user-friendly Python interface.

Supports:
  - Compiling Nova/C/C++/Rust source to MacroCore-X machine code
  - Three code generation modes: RISC, CISC, Hybrid
  - Cross-compilation targets: MCU, Workstation, PC
  - Assembling MacroCore-X assembly to binary
  - Simulating MacroCore-X binaries

Usage:
  python3 novatool.py compile input.nova -o output.bin --target pc
  python3 novatool.py compile input.nova --target mcu --codegen risc -S
  python3 novatool.py assemble input.asm -o output.bin
  python3 novatool.py simulate output.bin
  python3 novatool.py run input.nova --target pc          # compile + simulate
  python3 novatool.py build input.nova --target workstation  # compile + link
"""

import argparse
import os
import subprocess
import sys
from pathlib import Path
from typing import Optional, List, Dict, Any


# ── Configuration ────────────────────────────────────────────────────────────

WORKSPACE_ROOT = Path(__file__).resolve().parent.parent
TARGET_DIR = WORKSPACE_ROOT / "target" / "debug"

# Binary paths
NOVAC_BIN = TARGET_DIR / "novac"
NOVA_ASM_BIN = TARGET_DIR / "nova-asm"
NOVA_SIM_BIN = TARGET_DIR / "nova-sim"

# Target specifications
TARGETS = {
    "mcu": {
        "description": "Microcontroller (RISC, binary, 0x08000000)",
        "default_format": "binary",
        "base_addr": "0x08000000",
    },
    "workstation": {
        "description": "Workstation/Server (Hybrid, ELF, CISC+FP)",
        "default_format": "elf",
        "base_addr": "0x00000000",
    },
    "pc": {
        "description": "Personal Computer (Hybrid, binary, 0x1000)",
        "default_format": "binary",
        "base_addr": "0x00001000",
    },
}

CODEGEN_MODES = ["risc", "cisc", "hybrid"]

LANGUAGE_EXTENSIONS = {
    ".nova": "Nova",
    ".c": "C",
    ".cpp": "C++",
    ".cxx": "C++",
    ".cc": "C++",
    ".rs": "Rust",
    ".asm": "Assembly",
    ".s": "Assembly",
    ".nir": "NIR",
}


# ── Helpers ──────────────────────────────────────────────────────────────────

def find_binary(name: str, bin_path: Path) -> Path:
    """Find a binary, trying the Rust target dir first, then PATH."""
    if bin_path.exists():
        return bin_path
    # Try PATH
    result = subprocess.run(["which", name], capture_output=True, text=True)
    if result.returncode == 0:
        return Path(result.stdout.strip())
    raise FileNotFoundError(
        f"Cannot find '{name}'. Build the Rust project first: cargo build\n"
        f"  Looked at: {bin_path}"
    )


def detect_language(filepath: str) -> str:
    """Detect source language from file extension."""
    ext = Path(filepath).suffix.lower()
    return LANGUAGE_EXTENSIONS.get(ext, "Unknown")


def run_cmd(cmd: List[str], verbose: bool = False, env: Optional[Dict] = None) -> subprocess.CompletedProcess:
    """Run a command and return the result."""
    if verbose:
        print(f"[CMD] {' '.join(cmd)}", file=sys.stderr)
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    return subprocess.run(cmd, env=merged_env, capture_output=True, text=True)


def check_build() -> Dict[str, Path]:
    """Ensure all Rust binaries are built. Returns dict of binary paths."""
    binaries = {
        "novac": find_binary("novac", NOVAC_BIN),
        "nova-asm": find_binary("nova-asm", NOVA_ASM_BIN),
        "nova-sim": find_binary("nova-sim", NOVA_SIM_BIN),
    }
    return binaries


# ── Compile ──────────────────────────────────────────────────────────────────

def cmd_compile(args: argparse.Namespace) -> int:
    """
    Compile source code to MacroCore-X machine code.

    Wraps `novac` with the same CLI interface.
    """
    try:
        binaries = check_build()
    except FileNotFoundError as e:
        print(f"Error: {e}", file=sys.stderr)
        print("Run: cd nova && cargo build", file=sys.stderr)
        return 1

    novac = binaries["novac"]

    cmd = [str(novac)]

    # Input files
    for f in args.input:
        cmd.append(f)

    # Output
    if args.output:
        cmd.extend(["-o", args.output])

    # Target
    if args.target:
        cmd.extend(["-t", args.target])

    # Codegen mode
    if args.codegen:
        cmd.extend(["--codegen", args.codegen])

    # Optimization
    if args.opt_level is not None:
        cmd.extend(["-O", str(args.opt_level)])

    # Flags
    if args.emit_asm:
        cmd.append("-S")
    if args.compile_only:
        cmd.append("-c")
    if args.verbose:
        cmd.append("-v")
    if args.emit_nir:
        cmd.append("--emit-nir")
    if args.emit_mir:
        cmd.append("--emit-mir")
    if args.format:
        cmd.extend(["--format", args.format])

    # Run simulation
    if args.run:
        cmd.append("--run")
        if args.max_steps:
            cmd.extend(["--max-steps", str(args.max_steps)])

    if args.verbose:
        lang = detect_language(args.input[0])
        print(f"[novatool] Compiling {lang} source: {args.input[0]}", file=sys.stderr)

    result = run_cmd(cmd, verbose=args.verbose)

    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)

    return result.returncode


# ── Assemble ─────────────────────────────────────────────────────────────────

def cmd_assemble(args: argparse.Namespace) -> int:
    """
    Assemble MacroCore-X assembly to binary.

    Wraps `nova-asm` with a Python-friendly interface.
    """
    try:
        binaries = check_build()
    except FileNotFoundError as e:
        print(f"Error: {e}", file=sys.stderr)
        print("Run: cd nova && cargo build", file=sys.stderr)
        return 1

    asm_bin = binaries["nova-asm"]
    input_path = args.input[0]
    output_path = args.output

    if not os.path.exists(input_path):
        print(f"Error: input file not found: {input_path}", file=sys.stderr)
        return 1

    cmd = [str(asm_bin), input_path]
    if output_path:
        cmd.extend(["-o", output_path])

    if args.verbose:
        print(f"[novatool] Assembling: {input_path} -> {output_path or 'default'}", file=sys.stderr)

    result = run_cmd(cmd, verbose=args.verbose)

    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)

    if result.returncode == 0 and not args.verbose:
        print(f"Assembled: {output_path or input_path.replace('.asm', '.bin')}")

    return result.returncode


# ── Simulate ─────────────────────────────────────────────────────────────────

def cmd_simulate(args: argparse.Namespace) -> int:
    """
    Simulate a MacroCore-X binary.

    Wraps `nova-sim` with a Python-friendly interface.
    """
    try:
        binaries = check_build()
    except FileNotFoundError as e:
        print(f"Error: {e}", file=sys.stderr)
        print("Run: cd nova && cargo build", file=sys.stderr)
        return 1

    sim_bin = binaries["nova-sim"]
    binary_path = args.input[0]

    if not os.path.exists(binary_path):
        print(f"Error: binary file not found: {binary_path}", file=sys.stderr)
        return 1

    cmd = [str(sim_bin), binary_path]
    if args.debug:
        cmd.append("-d")

    if args.verbose:
        print(f"[novatool] Simulating: {binary_path}", file=sys.stderr)
        print(f"[novatool] Binary size: {os.path.getsize(binary_path)} bytes", file=sys.stderr)

    result = run_cmd(cmd, verbose=args.verbose)

    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)

    # nova-sim exits with the program's return value (may be non-zero),
    # which is NOT an error. Return 0 unless the simulator itself crashed.
    return 0


# ── Run (compile + simulate) ─────────────────────────────────────────────────

def cmd_run(args: argparse.Namespace) -> int:
    """Compile and immediately simulate the result."""
    # First compile
    compile_args = argparse.Namespace(
        input=args.input,
        output=args.output or "a.out",
        target=args.target,
        codegen=args.codegen,
        opt_level=args.opt_level,
        emit_asm=False,
        compile_only=False,
        verbose=args.verbose,
        emit_nir=False,
        emit_mir=False,
        run=False,
        format=args.format,
        max_steps=args.max_steps,
    )
    ret = cmd_compile(compile_args)
    if ret != 0:
        return ret

    # Then simulate
    output_file = compile_args.output
    sim_args = argparse.Namespace(
        input=[output_file],
        debug=args.debug,
        verbose=args.verbose,
    )
    # Clean up the binary after simulation
    ret = cmd_simulate(sim_args)
    if args.output is None:
        # Clean up default output
        try:
            os.remove(output_file)
        except OSError:
            pass
    return ret


# ── CLI Setup ────────────────────────────────────────────────────────────────

def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser."""
    parser = argparse.ArgumentParser(
        prog="novatool",
        description="Nova Compiler Toolchain - Python Wrapper",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python3 novatool.py compile hello.nova -o hello.bin --target pc
  python3 novatool.py compile hello.nova --target mcu --codegen risc -S
  python3 novatool.py compile hello.c --target pc
  python3 novatool.py assemble program.asm -o program.bin
  python3 novatool.py simulate program.bin
  python3 novatool.py run hello.nova --target pc
  python3 novatool.py build hello.nova --target workstation

Targets:
  mcu         - Microcontroller (RISC, binary, 0x08000000)
  workstation - Workstation/Server (Hybrid, ELF, CISC+FP)
  pc          - Personal Computer (Hybrid, binary, 0x1000) [default]

Codegen modes:
  risc   - Pure RISC instructions
  cisc   - Pure CISC instructions (composite ops)
  hybrid - Intelligent RISC/CISC selection [default]
        """,
    )

    subparsers = parser.add_subparsers(dest="command", help="Sub-command to execute")

    # ── compile ──
    compile_parser = subparsers.add_parser(
        "compile",
        aliases=["c"],
        help="Compile source code to MacroCore-X machine code",
    )
    compile_parser.add_argument(
        "input", nargs="+", help="Input source file(s) (.nova, .c, .cpp, .rs)"
    )
    compile_parser.add_argument("-o", "--output", help="Output file path")
    compile_parser.add_argument(
        "-t", "--target", choices=list(TARGETS.keys()), default="pc",
        help="Target platform (default: pc)"
    )
    compile_parser.add_argument(
        "--codegen", choices=CODEGEN_MODES, default="hybrid",
        help="Code generation mode (default: hybrid)"
    )
    compile_parser.add_argument(
        "-O", "--opt-level", type=int, choices=[0, 1, 2, 3], default=1,
        help="Optimization level (default: 1)"
    )
    compile_parser.add_argument(
        "-S", "--emit-asm", action="store_true",
        help="Output assembly only (do not assemble/link)"
    )
    compile_parser.add_argument(
        "-c", "--compile-only", action="store_true",
        help="Output object file only (do not link)"
    )
    compile_parser.add_argument(
        "-v", "--verbose", action="store_true", help="Verbose output"
    )
    compile_parser.add_argument(
        "--emit-nir", action="store_true", help="Emit NIR debug output"
    )
    compile_parser.add_argument(
        "--emit-mir", action="store_true", help="Emit MIR debug output"
    )
    compile_parser.add_argument(
        "--format", choices=["binary", "elf"], help="Output format"
    )
    compile_parser.add_argument(
        "--run", action="store_true", help="Run simulation after compilation"
    )
    compile_parser.add_argument(
        "--max-steps", type=int, default=10000, help="Maximum simulation steps"
    )

    # ── assemble ──
    asm_parser = subparsers.add_parser(
        "assemble",
        aliases=["a", "asm"],
        help="Assemble MacroCore-X assembly to binary",
    )
    asm_parser.add_argument("input", nargs=1, help="Input assembly file (.asm)")
    asm_parser.add_argument("-o", "--output", help="Output binary file")
    asm_parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")

    # ── simulate ──
    sim_parser = subparsers.add_parser(
        "simulate",
        aliases=["s", "sim"],
        help="Simulate a MacroCore-X binary",
    )
    sim_parser.add_argument("input", nargs=1, help="Input binary file")
    sim_parser.add_argument("-d", "--debug", action="store_true", help="Enable debug trace")
    sim_parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")

    # ── run ──
    run_parser = subparsers.add_parser(
        "run",
        aliases=["r"],
        help="Compile and immediately simulate the result",
    )
    run_parser.add_argument("input", nargs=1, help="Input source file")
    run_parser.add_argument("-o", "--output", help="Output file path")
    run_parser.add_argument(
        "-t", "--target", choices=list(TARGETS.keys()), default="pc",
        help="Target platform"
    )
    run_parser.add_argument(
        "--codegen", choices=CODEGEN_MODES, default="hybrid",
        help="Code generation mode"
    )
    run_parser.add_argument(
        "-O", "--opt-level", type=int, choices=[0, 1, 2, 3], default=1,
        help="Optimization level"
    )
    run_parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")
    run_parser.add_argument("-d", "--debug", action="store_true", help="Enable debug trace in simulator")
    run_parser.add_argument(
        "--format", choices=["binary", "elf"], help="Output format"
    )
    run_parser.add_argument(
        "--max-steps", type=int, default=10000, help="Maximum simulation steps"
    )

    # ── build ──
    build_parser = subparsers.add_parser(
        "build",
        aliases=["b"],
        help="Compile and link to executable (alias for compile without -S/-c)",
    )
    build_parser.add_argument("input", nargs=1, help="Input source file")
    build_parser.add_argument("-o", "--output", help="Output file path")
    build_parser.add_argument(
        "-t", "--target", choices=list(TARGETS.keys()), default="pc",
        help="Target platform"
    )
    build_parser.add_argument(
        "--codegen", choices=CODEGEN_MODES, default="hybrid",
        help="Code generation mode"
    )
    build_parser.add_argument(
        "-O", "--opt-level", type=int, choices=[0, 1, 2, 3], default=1,
        help="Optimization level"
    )
    build_parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")
    build_parser.add_argument(
        "--format", choices=["binary", "elf"], help="Output format"
    )

    # ── info ──
    info_parser = subparsers.add_parser(
        "info",
        aliases=["i"],
        help="Show toolchain information",
    )
    info_parser.add_argument(
        "-v", "--verbose", action="store_true", help="Show detailed info"
    )

    return parser


# ── Info Command ─────────────────────────────────────────────────────────────

def cmd_info(args: argparse.Namespace) -> int:
    """Show toolchain information."""
    print("Nova Compiler Toolchain")
    print("=" * 50)

    # Check binaries
    print("\n[Binaries]")
    for name, path in [("novac", NOVAC_BIN), ("nova-asm", NOVA_ASM_BIN), ("nova-sim", NOVA_SIM_BIN)]:
        exists = path.exists()
        status = "found" if exists else "NOT FOUND (run: cargo build)"
        print(f"  {name:12s} -> {path}")
        print(f"  {'':12s}    {status}")

    # Targets
    print("\n[Targets]")
    for name, info in TARGETS.items():
        print(f"  {name:12s} - {info['description']}")

    # Codegen modes
    print("\n[Codegen Modes]")
    print(f"  risc   - Pure RISC instructions")
    print(f"  cisc   - Pure CISC instructions (composite ops)")
    print(f"  hybrid - Intelligent RISC/CISC selection (default)")

    # Supported languages
    print("\n[Supported Languages]")
    for ext, lang in sorted(LANGUAGE_EXTENSIONS.items()):
        print(f"  {ext:8s} -> {lang}")

    if args.verbose:
        print("\n[Workspace]")
        print(f"  Root: {WORKSPACE_ROOT}")
        print(f"  Target: {TARGET_DIR}")

        # Check Rust version
        result = subprocess.run(["rustc", "--version"], capture_output=True, text=True)
        if result.returncode == 0:
            print(f"  Rust: {result.stdout.strip()}")

        # Check Python version
        print(f"  Python: {sys.version}")

    return 0


# ── Main ─────────────────────────────────────────────────────────────────────

def main():
    parser = build_parser()
    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
        return 1

    commands = {
        "compile": cmd_compile,
        "c": cmd_compile,
        "assemble": cmd_assemble,
        "a": cmd_assemble,
        "asm": cmd_assemble,
        "simulate": cmd_simulate,
        "s": cmd_simulate,
        "sim": cmd_simulate,
        "run": cmd_run,
        "r": cmd_run,
        "build": cmd_compile,  # build is same as compile without -S/-c
        "b": cmd_compile,
        "info": cmd_info,
        "i": cmd_info,
    }

    handler = commands.get(args.command)
    if handler is None:
        print(f"Unknown command: {args.command}", file=sys.stderr)
        parser.print_help()
        return 1

    return handler(args)


if __name__ == "__main__":
    sys.exit(main())