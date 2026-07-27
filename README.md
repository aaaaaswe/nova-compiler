# Nova Compiler

A programming language and compiler toolchain for the [MacroCore-X](https://github.com/aaaaaswe/MacroCore-X) instruction set. Built entirely in Rust, zero LLVM dependency.


## Features

- **Nova Language** — C-like syntax for systems programming: functions, variables, loops, conditionals
- **Three Codegen Modes** — Pure RISC, Pure CISC, RISC/CISC Hybrid (default)
- **Multi-Language Frontend** — Nova / C / C++ / Rust / Assembly / NIR via `nova-ffi`
- **Cross-Compilation** — MacroCore-X (MCU/Workstation/PC) + native (x86_64, aarch64, x86, arm)
- **Custom NIR** — Novel Intermediate Representation with 126 instructions across 13 categories
- **Full Pipeline** — Frontend → HIR → MIR → NIR → Codegen → Assembly → Link
- **Simulator** — Built-in MacroCore-X ISA simulator (compile and run in one step)
- **Pure Rust** — No scripting language dependencies; single binary `novac` for all targets

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
       ├──→ nova-codegen (risc)   ──→ MacroCore-X Pure RISC
       ├──→ nova-codegen (cisc)   ──→ MacroCore-X Pure CISC
       ├──→ nova-codegen (hybrid) ──→ MacroCore-X RISC/CISC hybrid
       └──→ nova-codegen (native) ──→ x86_64 / aarch64 / x86 / arm
                │
                ▼
       ┌──────────────┐
       │   nova-asm    │  MacroCore-X two-pass assembler
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
| `nova-codegen` | Code generation (RISC/CISC/Hybrid/Native), register allocation |
| `nova-asm` | MacroCore-X assembler |
| `nova-sim` | MacroCore-X ISA simulator |
| `nova-link` | Linker (ELF / binary output) |
| `nova-ffi` | Multi-language frontend interface (C/C++/Rust) |
| `nova-driver` | CLI driver (`novac`) |

## Quick Start

### Requirements

- Rust 1.70+

### Build

```bash
git clone https://github.com/aaaaaswe/nova-compiler.git
cd nova-compiler
cargo build
```

### Usage

**Compile Nova source:**

```bash
# ── MacroCore-X targets ──

# Compile to binary
./target/debug/novac hello.nova -o hello.bin --target pc

# Compile and run in simulator
./target/debug/novac hello.nova --target pc --run

# Emit assembly only
./target/debug/novac hello.nova --target mcu -S -o hello.asm

# Select codegen mode
./target/debug/novac hello.nova --target pc --codegen risc
./target/debug/novac hello.nova --target pc --codegen cisc
./target/debug/novac hello.nova --target pc --codegen hybrid  # default

# ── Native cross-compilation ──

# Emit x86_64 assembly
./target/debug/novac hello.nova --target x86_64 -S -o hello_x64.s

# Emit aarch64/ARM64 assembly
./target/debug/novac hello.nova --target aarch64 -S -o hello_arm64.s

# Emit x86 32-bit assembly
./target/debug/novac hello.nova --target x86 -S -o hello_x86.s

# Emit ARM32 assembly
./target/debug/novac hello.nova --target arm -S -o hello_arm.s

# Compile C source (MacroCore-X only)
CC=gcc ./target/debug/novac hello.c --target pc -o hello.bin
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

## Codegen Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| **RISC** | Pure RISC instructions, simple and fast decode | MCU, low power |
| **CISC** | Pure CISC instructions, composite ops, high code density | High performance |
| **Hybrid** | Intelligent RISC/CISC selection based on register pressure | Default, general purpose |

## Target Platforms

### MacroCore-X Targets

| Target | Format | Base Address | Notes |
|--------|--------|-------------|-------|
| `mcu` | Binary | 0x08000000 | RISC-first, small memory |
| `workstation` | ELF | 0x00400000 | Hybrid, CISC + FP enabled |
| `pc` | Binary | 0x00001000 | Hybrid, general purpose |

### Native Cross-Compilation Targets

| Target | Arch | Width | Assembler | Linker |
|--------|------|-------|-----------|--------|
| `x86_64` | AMD64 | 64-bit | `as` | `ld` |
| `aarch64` | ARM64 | 64-bit | `aarch64-linux-gnu-as` | `aarch64-linux-gnu-ld` |
| `x86` | IA-32 | 32-bit | `as` | `ld` |
| `arm` | ARM32 | 32-bit | `arm-linux-gnueabihf-as` | `arm-linux-gnueabihf-ld` |

## Running Tests

```bash
cargo test
```

## Contributing

Contributions are welcome! Whether it's bug fixes, new features, documentation improvements, or
new target backends — we'd love your help.

- **Report bugs** or **request features** via [GitHub Issues](https://github.com/aaaaaswe/nova-compiler/issues)
- **Submit PRs**: fork the repo, create a branch, and open a pull request
- **Add a backend**: implement `crate::native::YourArch` following the pattern in `nova-codegen/src/native/`
- **Improve the language**: extend the parser in `nova-frontend/src/parser.rs` and lower in `nova-hir/`

All contributions are reviewed. Let's build a great compiler together!

## License

Nova Compiler License
