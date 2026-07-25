//! Tests for the NIR parser, validator, and optimizer.

use crate::parser;
use crate::validator;
use crate::optimizer;

/// Parse a .nir file and return the module.
fn parse_test_file(name: &str) -> crate::ir::Module {
    let path = format!("../../tests/{}", name);
    parser::parse_file(&path).expect("failed to parse test file")
}

#[test]
fn test_parse_alu() {
    let module = parse_test_file("test_alu.nir");
    assert_eq!(module.functions.len(), 1);
    let func = &module.functions[0];
    assert_eq!(func.name, "test_alu");
    assert_eq!(func.basic_blocks.len(), 1);
    let bb = &func.basic_blocks[0];
    assert_eq!(bb.name, "entry");
    assert!(bb.is_entry);
    // Should have many instructions: addi, add, subi, muli, andi, ori, xori, shli, shri, div, rem, neg, not, add, add, ret
    assert!(bb.instructions.len() >= 16);
    // Verify the last instruction is a ret
    assert!(matches!(bb.instructions.last(), Some(crate::ir::Instruction::Ret { .. })));
}

#[test]
fn test_parse_memory() {
    let module = parse_test_file("test_memory.nir");
    assert_eq!(module.globals.len(), 2);
    assert_eq!(module.functions.len(), 1);
    let func = &module.functions[0];
    assert_eq!(func.name, "test_memory");
    assert_eq!(func.basic_blocks.len(), 1);
    let bb = &func.basic_blocks[0];
    // Should have load, addi, store, load, loadi, lea, muli, storei, load, ret
    assert!(bb.instructions.len() >= 10);
    assert!(matches!(bb.instructions.last(), Some(crate::ir::Instruction::Ret { .. })));
}

#[test]
fn test_parse_controlflow() {
    let module = parse_test_file("test_controlflow.nir");
    assert_eq!(module.functions.len(), 1);
    let func = &module.functions[0];
    assert_eq!(func.name, "control_flow_test");
    assert_eq!(func.basic_blocks.len(), 3);
    // Check blocks: entry, loop, done
    let names: Vec<&str> = func.basic_blocks.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"entry"));
    assert!(names.contains(&"loop"));
    assert!(names.contains(&"done"));
    // Check predecessors
    let loop_bb = func.basic_blocks.iter().find(|b| b.name == "loop").unwrap();
    assert!(loop_bb.predecessors.contains(&"entry".to_string()));
    assert!(loop_bb.predecessors.contains(&"loop".to_string()));
    // Check phi nodes
    let has_phi = loop_bb.instructions.iter().any(|i| matches!(i, crate::ir::Instruction::Phi { .. }));
    assert!(has_phi);
}

#[test]
fn test_parse_composite() {
    let module = parse_test_file("test_composite.nir");
    assert_eq!(module.functions.len(), 1);
    let func = &module.functions[0];
    assert_eq!(func.name, "test_composite");
    assert_eq!(func.basic_blocks.len(), 1);
    let bb = &func.basic_blocks[0];
    // Should have mem_add, mem_sub, load, mem_xchg, push, push, pop, pop, enter, addi, store, leave, load, ret
    assert!(bb.instructions.len() >= 14);
    // Check for specific instruction types
    let has_mem_add = bb.instructions.iter().any(|i| matches!(i, crate::ir::Instruction::MemAdd { .. }));
    let has_mem_sub = bb.instructions.iter().any(|i| matches!(i, crate::ir::Instruction::MemSub { .. }));
    let has_mem_xchg = bb.instructions.iter().any(|i| matches!(i, crate::ir::Instruction::MemXchg { .. }));
    let has_push = bb.instructions.iter().any(|i| matches!(i, crate::ir::Instruction::Push { .. }));
    let has_pop = bb.instructions.iter().any(|i| matches!(i, crate::ir::Instruction::Pop { .. }));
    let has_enter = bb.instructions.iter().any(|i| matches!(i, crate::ir::Instruction::Enter { .. }));
    let has_leave = bb.instructions.iter().any(|i| matches!(i, crate::ir::Instruction::Leave));
    assert!(has_mem_add);
    assert!(has_mem_sub);
    assert!(has_mem_xchg);
    assert!(has_push);
    assert!(has_pop);
    assert!(has_enter);
    assert!(has_leave);
}

#[test]
fn test_parse_fib() {
    let module = parse_test_file("test_fib.nir");
    assert_eq!(module.functions.len(), 1);
    let func = &module.functions[0];
    assert_eq!(func.name, "fibonacci");
    // Check parameter
    assert_eq!(func.parameters.len(), 1);
    assert_eq!(func.parameters[0].ty(), &crate::types::IrType::I64);
    // Check return type
    assert_eq!(func.return_type, crate::types::IrType::I64);
    // Check blocks
    assert_eq!(func.basic_blocks.len(), 6);
    let names: Vec<&str> = func.basic_blocks.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"entry"));
    assert!(names.contains(&"return_one"));
    assert!(names.contains(&"compute"));
    assert!(names.contains(&"loop"));
    assert!(names.contains(&"loop_body"));
    assert!(names.contains(&"done"));
}

#[test]
fn test_validate_all() {
    let files = [
        "test_alu.nir",
        "test_memory.nir",
        "test_controlflow.nir",
        "test_composite.nir",
        "test_fib.nir",
    ];
    for file in &files {
        let module = parse_test_file(file);
        let result = validator::validate_module(&module);
        if !result.is_valid() {
            for err in &result.errors {
                eprintln!("[{}] validation error: {}", file, err);
            }
        }
        for warn in &result.warnings {
            eprintln!("[{}] validation warning: {}", file, warn);
        }
        assert!(result.is_valid(), "validation failed for {}: {:?}", file, result.errors);
    }
}

#[test]
fn test_optimize_all() {
    let files = [
        "test_alu.nir",
        "test_memory.nir",
        "test_controlflow.nir",
        "test_composite.nir",
        "test_fib.nir",
    ];
    for file in &files {
        let mut module = parse_test_file(file);
        optimizer::optimize_module(&mut module);
        // After optimization, validate again
        let result = validator::validate_module(&module);
        if !result.is_valid() {
            for err in &result.errors {
                eprintln!("[{}] post-optimization validation error: {}", file, err);
            }
        }
        assert!(result.is_valid(), "post-optimization validation failed for {}: {:?}", file, result.errors);
    }
}

#[test]
fn test_optimize_alu_constant_folding() {
    let mut module = parse_test_file("test_alu.nir");
    optimizer::optimize_module(&mut module);
    let func = &module.functions[0];
    let bb = &func.basic_blocks[0];
    // Check that constant folding happened: addi i64 0, 10 should become movi i64 10
    let has_movi = bb.instructions.iter().any(|i| matches!(i, crate::ir::Instruction::Movi { .. }));
    assert!(has_movi, "constant folding should produce movi instructions");
}

#[test]
fn test_parser_errors() {
    // Test invalid syntax
    let result = parser::parse("invalid @@@", "test");
    assert!(result.is_err());
}

#[test]
fn test_parse_empty_module() {
    let result = parser::parse("", "empty");
    assert!(result.is_ok());
    let module = result.unwrap();
    assert!(module.functions.is_empty());
    assert!(module.globals.is_empty());
}

#[test]
fn test_parse_global() {
    let src = "global @x : i64 = 42\n";
    let module = parser::parse(src, "test").unwrap();
    assert_eq!(module.globals.len(), 1);
    assert_eq!(module.globals[0].ty(), &crate::types::IrType::I64);
}

#[test]
fn test_parse_br_cond() {
    let src = "func @test() -> i64 @callconv(nova) {\nentry:\n%0, %f0 = addi i64 0, 10\n%1, %f1 = subi i64 %0, 5\n%cond = test_eq %f1\nbr_cond %cond, true_bb, false_bb\ntrue_bb:\nret i64 %0\nfalse_bb:\nret i64 %1\n}\n";
    let module = parser::parse(src, "test").unwrap();
    assert_eq!(module.functions.len(), 1);
    let func = &module.functions[0];
    assert_eq!(func.basic_blocks.len(), 3);
}

#[test]
fn test_parse_ret_void() {
    let src = "func @test() @callconv(nova) {\nentry:\nret void\n}\n";
    let module = parser::parse(src, "test").unwrap();
    assert_eq!(module.functions.len(), 1);
    let func = &module.functions[0];
    assert_eq!(func.return_type, crate::types::IrType::Void);
    let bb = &func.basic_blocks[0];
    assert!(matches!(bb.instructions[0], crate::ir::Instruction::Ret { value: None }));
}