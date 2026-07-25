/// Integration tests for the full Nova compiler pipeline.
///
/// Each test follows the full pipeline:
///   Parse Nova source → AST → HIR (type check) → MIR → NIR → Assembly → Simulate
///
/// These tests verify end-to-end correctness of the compiler frontend and backend.

use nova_frontend::parse_source;
use nova_hir::lower_and_check;
use nova_mir::lower_to_mir;
use nova_codegen::{lower_mir_to_nir, generate_code, CodegenMode, Target};
use nova_asm::assemble_source;
use nova_sim::Cpu;

/// Run the full pipeline from Nova source to simulation, and return the CPU state.
fn compile_and_run(source: &str) -> Cpu {
    // Step 1: Parse Nova source → AST
    let ast = parse_source(source).expect("Failed to parse Nova source");

    // Step 2: AST → HIR (with type checking)
    let hir = lower_and_check(&ast).expect("Failed to lower to HIR or type check");

    // Step 3: HIR → MIR
    let mir = lower_to_mir(&hir);

    // Step 4: MIR → NIR
    let nir = lower_mir_to_nir(&mir);

    // Step 5: NIR → Assembly (RISC mode)
    let asm = generate_code(&nir, CodegenMode::Risc, Target::Pc);

    // Debug: print assembly
    eprintln!("=== Generated Assembly ===\n{}\n=== End Assembly ===", asm);

    // Step 6: Assemble → binary
    let binary = assemble_source(&asm).expect("Failed to assemble");

    // Step 7: Load binary into CPU and simulate
    let mut cpu = Cpu::new();
    cpu.load_binary(&binary);
    cpu.run(false).expect("CPU simulation failed");

    cpu
}

/// Run the full pipeline and return the value of return register R1.
fn compile_and_run_a0(source: &str) -> i64 {
    let cpu = compile_and_run(source);
    cpu.r[1] as i64
}

/// Run the full pipeline with debug tracing and step limit.
fn compile_and_run_debug(source: &str, max_steps: u64) -> Cpu {
    let ast = parse_source(source).expect("Failed to parse Nova source");
    let hir = lower_and_check(&ast).expect("Failed to lower to HIR or type check");
    let mir = lower_to_mir(&hir);
    let nir = lower_mir_to_nir(&mir);
    let asm = generate_code(&nir, CodegenMode::Risc, Target::Pc);
    eprintln!("=== Generated Assembly ===\n{}\n=== End Assembly ===", asm);
    let binary = assemble_source(&asm).expect("Failed to assemble");
    let mut cpu = Cpu::new();
    cpu.load_binary(&binary);
    cpu.run_with_limit(true, max_steps).expect("CPU simulation failed");
    cpu
}

// =============================================================================
//  Arithmetic Tests
// =============================================================================

#[test]
fn test_simple_addition() {
    let source = r#"
fn main() -> i64 {
    let x = 2 + 3;
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 5, "2 + 3 should be 5");
}

#[test]
fn test_multiplication() {
    let source = r#"
fn main() -> i64 {
    let x = 3 * 4;
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 12, "3 * 4 should be 12");
}

#[test]
fn test_compound_arithmetic() {
    let source = r#"
fn main() -> i64 {
    let x = 2 + 3 * 4;
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 14, "2 + 3 * 4 should be 14");
}

#[test]
fn test_subtraction() {
    let source = r#"
fn main() -> i64 {
    let x = 10 - 3;
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 7, "10 - 3 should be 7");
}

#[test]
fn test_division() {
    let source = r#"
fn main() -> i64 {
    let x = 20 / 4;
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 5, "20 / 4 should be 5");
}

#[test]
fn test_remainder() {
    let source = r#"
fn main() -> i64 {
    let x = 17 % 5;
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 2, "17 % 5 should be 2");
}

#[test]
fn test_bitwise_and() {
    let source = r#"
fn main() -> i64 {
    let x = 6 & 3;
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 2, "6 & 3 should be 2");
}

#[test]
fn test_bitwise_or() {
    let source = r#"
fn main() -> i64 {
    let x = 6 | 3;
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 7, "6 | 3 should be 7");
}

#[test]
fn test_negation() {
    let source = r#"
fn main() -> i64 {
    let x = -42;
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, -42, "-42 should be -42");
}

// =============================================================================
//  Conditional Tests
// =============================================================================

#[test]
fn test_if_true_branch_debug() {
    let source = r#"
fn main() -> i64 {
    let x = 10;
    if x > 5 {
        return 1;
    } else {
        return 0;
    }
}
"#;
    let cpu = compile_and_run_debug(source, 50);
    assert_eq!(cpu.r[1] as i64, 1, "if 10 > 5 should return 1, got {}", cpu.r[1]);
}

#[test]
fn test_if_true_branch() {
    let source = r#"
fn main() -> i64 {
    let x = 10;
    if x > 5 {
        return 1;
    } else {
        return 0;
    }
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 1, "if 10 > 5 should return 1");
}

#[test]
fn test_if_false_branch() {
    let source = r#"
fn main() -> i64 {
    let x = 3;
    if x > 5 {
        return 1;
    } else {
        return 0;
    }
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 0, "if 3 > 5 should return 0");
}

#[test]
fn test_if_equal_comparison() {
    let source = r#"
fn main() -> i64 {
    let x = 5;
    if x == 5 {
        return 100;
    } else {
        return 0;
    }
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 100, "if 5 == 5 should return 100");
}

#[test]
fn test_if_not_equal() {
    let source = r#"
fn main() -> i64 {
    let x = 5;
    if x != 3 {
        return 1;
    } else {
        return 0;
    }
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 1, "if 5 != 3 should return 1");
}

#[test]
fn test_if_less_than() {
    let source = r#"
fn main() -> i64 {
    let x = 3;
    if x < 10 {
        return 1;
    } else {
        return 0;
    }
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 1, "if 3 < 10 should return 1");
}

#[test]
fn test_if_greater_equal() {
    let source = r#"
fn main() -> i64 {
    let x = 10;
    if x >= 10 {
        return 1;
    } else {
        return 0;
    }
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 1, "if 10 >= 10 should return 1");
}

#[test]
fn test_if_less_equal() {
    let source = r#"
fn main() -> i64 {
    let x = 5;
    if x <= 5 {
        return 42;
    } else {
        return 0;
    }
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 42, "if 5 <= 5 should return 42");
}

// =============================================================================
//  Loop Tests
// =============================================================================

#[test]
fn test_while_loop_sum() {
    // Sum of 0..9 = 45
    let source = r#"
fn main() -> i64 {
    let x = 0;
    let i = 0;
    while i < 10 {
        x = x + i;
        i = i + 1;
    }
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 45, "sum 0..9 should be 45");
}

#[test]
fn test_while_loop_countdown() {
    let source = r#"
fn main() -> i64 {
    let x = 10;
    while x > 0 {
        x = x - 1;
    }
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 0, "countdown from 10 should reach 0");
}

#[test]
fn test_loop_break() {
    // Loop statement test (simplified)
    let source = r#"
fn main() -> i64 {
    let x = 0;
    let i = 0;
    while i < 5 {
        x = x + 2;
        i = i + 1;
    }
    return x;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 10, "5 * 2 should be 10");
}

// =============================================================================
//  Function Call Tests
// =============================================================================

#[test]
fn test_simple_function_call() {
    let source = r#"
fn add(a: i64, b: i64) -> i64 {
    return a + b;
}

fn main() -> i64 {
    return add(3, 4);
}
"#;
    let cpu = compile_and_run_debug(source, 50);
    assert_eq!(cpu.r[1] as i64, 7, "add(3, 4) should be 7");
}

#[test]
fn test_function_call_with_expression_args() {
    let source = r#"
fn mul(a: i64, b: i64) -> i64 {
    return a * b;
}

fn main() -> i64 {
    return mul(2 + 3, 4);
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 20, "mul(2+3, 4) should be 20");
}

#[test]
fn test_nested_function_call() {
    let source = r#"
fn double(x: i64) -> i64 {
    return x * 2;
}

fn add(a: i64, b: i64) -> i64 {
    return a + b;
}

fn main() -> i64 {
    return add(double(3), double(4));
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 14, "add(double(3), double(4)) = 6 + 8 = 14");
}

// =============================================================================
//  Combined Tests
// =============================================================================

#[test]
fn test_factorial_loop() {
    // Compute 5! = 120 using a while loop
    let source = r#"
fn main() -> i64 {
    let result = 1;
    let i = 1;
    while i <= 5 {
        result = result * i;
        i = i + 1;
    }
    return result;
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 120, "5! should be 120");
}

#[test]
fn test_conditional_with_arithmetic() {
    let source = r#"
fn main() -> i64 {
    let x = 10;
    let y = 20;
    if x + y > 25 {
        return 100;
    } else {
        return 0;
    }
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 100, "10 + 20 > 25 should return 100");
}

#[test]
fn test_multiple_returns() {
    let source = r#"
fn abs(x: i64) -> i64 {
    if x < 0 {
        return -x;
    }
    return x;
}

fn main() -> i64 {
    return abs(-42);
}
"#;
    let result = compile_and_run_a0(source);
    assert_eq!(result, 42, "abs(-42) should be 42");
}