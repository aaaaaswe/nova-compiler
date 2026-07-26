# Nova Compiler

Nova 编译器 — 为 [MacroCore-X](https://github.com/aaaaaswe/MacroCore-X) 指令集设计的编程语言与编译器工具链，纯 Rust 实现，不依赖 LLVM。

## 特性

- **Nova 语言** — 类 C 语法的系统编程语言，支持函数、变量、循环、条件分支
- **三种编译模式** — 纯 RISC、纯 CISC、RISC/CISC 混合（默认）
- **多语言前端** — 通过 `nova-ffi` 支持 Nova / C / C++ / Rust / Assembly / NIR
- **交叉编译** — 支持 MCU（微控制器）、Workstation（工作站）、PC（个人电脑）三个目标平台
- **自研 NIR** — 自定义中间表示，126 条指令，13 个类别
- **完整管线** — 前端 → HIR → MIR → NIR → 代码生成 → 汇编 → 链接
- **模拟器** — 内置 MacroCore-X ISA 模拟器，可编译后直接运行
- **Python 辅助** — 提供 Python 包装脚本，方便调用编译工具

## 架构

```
.nova 源文件
    │
    ▼
┌──────────────┐
│  nova-frontend │  词法分析 → 语法分析 → AST
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   nova-hir    │  类型检查 → HIR
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   nova-mir    │  控制流 lowering → MIR
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   nova-nir    │  NIR 中间表示（优化：DCE、常量折叠、基本块合并）
└──────┬───────┘
       │
       ├──→ nova-codegen (risc) ──→ 纯 RISC 指令
       ├──→ nova-codegen (cisc) ──→ 纯 CISC 指令
       └──→ nova-codegen (hybrid) → 混合 RISC/CISC
                │
                ▼
       ┌──────────────┐
       │   nova-asm    │  二遍汇编
       └──────┬───────┘
              │
              ▼
       ┌──────────────┐
       │  nova-link    │  符号解析 + 段合并 + 重定位 → ELF / Binary
       └──────────────┘
```

## 模块

| 模块 | 功能 |
|------|------|
| `nova-frontend` | Nova 词法分析器与解析器 |
| `nova-hir` | 高级中间表示，类型检查 |
| `nova-mir` | 中级中间表示，控制流图 |
| `nova-nir` | 低级中间表示（NIR），优化器 |
| `nova-codegen` | 代码生成（RISC/CISC/Hybrid），寄存器分配 |
| `nova-asm` | MacroCore-X 汇编器 |
| `nova-sim` | MacroCore-X ISA 模拟器 |
| `nova-link` | 链接器（ELF / 二进制输出） |
| `nova-ffi` | 多语言前端接口（C/C++/Rust） |
| `nova-driver` | CLI 驱动（`novac`） |

## 快速开始

### 环境要求

- Rust 1.70+
- Python 3.10+（可选，用于辅助脚本）

### 构建

```bash
git clone https://github.com/aaaaaswe/nova-compiler.git
cd nova-compiler
cargo build
```

### 用法

**Rust CLI（`novac`）：**

```bash
# 编译 Nova 源码到二进制
./target/debug/novac hello.nova -o hello.bin --target pc

# 编译并模拟运行
./target/debug/novac hello.nova --target pc --run

# 输出汇编代码
./target/debug/novac hello.nova --target mcu -S -o hello.asm

# 指定编译模式
./target/debug/novac hello.nova --target pc --codegen risc
./target/debug/novac hello.nova --target pc --codegen cisc
./target/debug/novac hello.nova --target pc --codegen hybrid  # 默认

# 编译 C 源码
CC=gcc ./target/debug/novac hello.c --target pc -o hello.bin
```

**Python 辅助脚本（`novatool`）：**

```bash
# 编译
python3 scripts/novatool.py compile hello.nova -o hello.bin --target pc

# 编译 + 模拟
python3 scripts/novatool.py run hello.nova --target pc

# 汇编
python3 scripts/novatool.py assemble program.asm -o program.bin

# 模拟
python3 scripts/novatool.py simulate hello.bin

# 查看工具链信息
python3 scripts/novatool.py info
```

或使用 shell 包装：

```bash
./novatool compile hello.nova -o hello.bin --target pc
./novatool run hello.nova --target pc
```

### Nova 语言示例

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

更多示例见 [scripts/examples/](scripts/examples/)。

## 编译模式

| 模式 | 描述 | 适用场景 |
|------|------|----------|
| **RISC** | 纯 RISC 指令，指令简单、解码快 | MCU 低功耗场景 |
| **CISC** | 纯 CISC 指令，复合操作、代码密度高 | 高性能计算 |
| **Hybrid** | 智能混合，根据寄存器压力自动选择 | 默认模式，通用场景 |

## 目标平台

| 目标 | 格式 | 起始地址 | 特点 |
|------|------|----------|------|
| `mcu` | Binary | 0x08000000 | RISC 优先，小内存 |
| `workstation` | ELF | 0x00000000 | Hybrid，启用 CISC + 浮点 |
| `pc` | Binary | 0x00001000 | Hybrid，通用计算 |

## 运行测试

```bash
cargo test
```

## 许可证

MIT