# Nova Compiler

A programming language and compiler toolchain for the [MacroCore-X](https://github.com/aaaaaswe/MacroCore-X) instruction set. Built entirely in Rust, zero LLVM dependency.

> [中文文档](README_CN.md)

## Features

- **Nova Language** — C-like syntax for systems programming: functions, variables, loops, conditionals
- **Three Codegen Modes** — Pure RISC, Pure CISC, RISC/CISC Hybrid (default)
- **Multi-Language Frontend** — Nova / C / C++ / Rust / Assembly / NIR via `nova-ffi`
- **Cross-Compilation** — MCU, Workstation, and PC targets
- **Custom NIR** — Novel Intermediate Representation with 126 instructions across 13 categories
- **Full Pipeline** — Frontend → HIR → MIR → NIR → Codegen → Assembly → Link
- **Simulator** — Built-in MacroCore-X ISA simulator (compile and run in one step)
- **Python Helpers** — Convenience scripts wrapping the compiler tools

## Architecture

```
.nova source
    │
    ▼
┌──────────────┐
│  nova-frontend │  Lexing → Parsing → AST
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   nova-hir    │  Type checking → HIR
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   nova-mir    │  Control flow lowering → MIR
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   nova-nir    │  NIR IR (opt: DCE, constant folding, BB merging)
└──────┬───────┘
       │
       ├──→ nova-codegen (risc) ──→ Pure RISC
       ├──→ nova-codegen (cisc) ──→ Pure CISC
       └──→ nova-codegen (hybrid) → RISC/CISC hybrid
                │
                ▼
       ┌──────────────┐
       │   nova-asm    │  Two-pass assembler
       └──────┬───────┘
              │
              ▼
       ┌──────────────┐
       │  nova-link    │  Symbol resolution + section merge + relocation
       └──────────────┘
```

## Crates

| Crate | Purpose |
|-------|---------|
| `nova-frontend` | Nova lexer and parser |
| `nova-hir` | High-level IR, type checking |
| `nova-mir` | Mid-level IR, control flow graphs |
| `nova-nir` | Low-level IR (NIR), optimizer |
| `nova-codegen` | Code generation (RISC/CISC/Hybrid), register allocation |
| `nova-asm` | MacroCore-X assembler |
| `nova-sim` | MacroCore-X ISA simulator |
| `nova-link` | Linker (ELF / binary output) |
| `nova-ffi` | Multi-language frontend interface (C/C++/Rust) |
| `nova-driver` | CLI driver (`novac`) |

## Quick Start

### Requirements

- Rust 1.70+
- Python 3.10+ (optional, for helper scripts)

### Build

```bash
git clone https://github.com/aaaaaswe/nova-compiler.git
cd nova-compiler
cargo build
```

### Usage

**Rust CLI (`novac`):**

```bash
# Compile Nova source to binary
./target/debug/novac hello.nova -o hello.bin --target pc

# Compile and run in simulator
./target/debug/novac hello.nova --target pc --run

# Emit assembly only
./target/debug/novac hello.nova --target mcu -S -o hello.asm

# Select codegen mode
./target/debug/novac hello.nova --target pc --codegen risc
./target/debug/novac hello.nova --target pc --codegen cisc
./target/debug/novac hello.nova --target pc --codegen hybrid  # default

# Compile C source
CC=gcc ./target/debug/novac hello.c --target pc -o hello.bin
```

**Python helpers (`novatool`):**

```bash
# Compile
python3 scripts/novatool.py compile hello.nova -o hello.bin --target pc

# Compile + simulate
python3 scripts/novatool.py run hello.nova --target pc

# Assemble
python3 scripts/novatool.py assemble program.asm -o program.bin

# Simulate
python3 scripts/novatool.py simulate hello.bin

# Show toolchain info
python3 scripts/novatool.py info
```

Or via the shell wrapper:

```bash
./novatool compile hello.nova -o hello.bin --target pc
./novatool run hello.nova --target pc
```

### Nova Language Example

```nova
fn fib(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

fn main() -> i64 {
    return fib(10);  // 55
}
```

More examples in [scripts/examples/](scripts/examples/).

## Codegen Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| **RISC** | Pure RISC instructions, simple and fast decode | MCU, low power |
| **CISC** | Pure CISC instructions, composite ops, high code density | High performance |
| **Hybrid** | Intelligent RISC/CISC selection based on register pressure | Default, general purpose |

## Target Platforms

| Target | Format | Base Address | Notes |
|--------|--------|-------------|-------|
| `mcu` | Binary | 0x08000000 | RISC-first, small memory |
| `workstation` | ELF | 0x00000000 | Hybrid, CISC + FP enabled |
| `pc` | Binary | 0x00001000 | Hybrid, general purpose |

## Running Tests

```bash
cargo test
```

## License

MIT