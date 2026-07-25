//! NIR optimizer – basic optimization passes.
//!
//! Passes:
//! - Dead Code Elimination (DCE): remove instructions whose results are not used
//! - Constant Folding: evaluate arithmetic expressions with constant operands
//! - Basic Block Merging: merge consecutive blocks with single successor/predecessor

use std::collections::HashSet;

use crate::ir::{BasicBlock, Function, Instruction, Module};
use crate::types::Value;

// =============================================================================
//  Dead Code Elimination
// =============================================================================

/// Remove instructions whose results are not used, respecting side effects.
pub fn dead_code_elimination(func: &mut Function) {
    let mut changed = true;
    while changed {
        changed = false;
        let used = collect_used_vregs(&func.basic_blocks);
        for bb in &mut func.basic_blocks {
            let mut new_instructions = Vec::new();
            let mut removed = false;

            for inst in bb.instructions.drain(..) {
                // Never remove instructions with side effects
                if inst.has_side_effects() {
                    new_instructions.push(inst);
                    continue;
                }

                // Check if the result (and flags_result) are used
                let result_used = instruction_result_vreg(&inst)
                    .map(|v| used.contains(v))
                    .unwrap_or(true);
                let flags_used = instruction_flags_vreg(&inst)
                    .map(|v| used.contains(v))
                    .unwrap_or(true);

                if result_used || flags_used {
                    new_instructions.push(inst);
                } else {
                    removed = true;
                }
            }

            bb.instructions = new_instructions;
            if removed {
                changed = true;
            }
        }
    }
}

fn collect_used_vregs(bbs: &[BasicBlock]) -> HashSet<String> {
    let mut used = HashSet::new();
    for bb in bbs {
        for inst in &bb.instructions {
            // Collect operands of all instructions
            for operand in instruction_operands_vreg(inst) {
                used.insert(operand.to_string());
            }
            // Also collect the condition register for BrCond
            if let Instruction::BrCond { cond, .. } = inst {
                if let Value::VReg { name, .. } = cond {
                    used.insert(name.clone());
                }
            }
            // Collect switch value
            if let Instruction::Switch { value, .. } = inst {
                if let Value::VReg { name, .. } = value {
                    used.insert(name.clone());
                }
            }
            // Collect ret value
            if let Instruction::Ret {
                value: Some(value), ..
            } = inst
            {
                if let Value::VReg { name, .. } = value {
                    used.insert(name.clone());
                }
            }
            // Collect mem/composite operands
            if let Instruction::MemAdd { value, .. }
            | Instruction::MemSub { value, .. }
            | Instruction::MemAnd { value, .. }
            | Instruction::MemOr { value, .. }
            | Instruction::MemXor { value, .. }
            | Instruction::MemXchg { value, .. } = inst
            {
                if let Value::VReg { name, .. } = value {
                    used.insert(name.clone());
                }
            }
            if let Instruction::Store { value, .. } = inst {
                if let Value::VReg { name, .. } = value {
                    used.insert(name.clone());
                }
            }
            if let Instruction::Storei { value, .. } = inst {
                if let Value::VReg { name, .. } = value {
                    used.insert(name.clone());
                }
            }
            if let Instruction::Push { value, .. } = inst {
                if let Value::VReg { name, .. } = value {
                    used.insert(name.clone());
                }
            }
            // Phi incoming values
            if let Instruction::Phi { incoming, .. } = inst {
                for (val, _) in incoming {
                    if let Value::VReg { name, .. } = val {
                        used.insert(name.clone());
                    }
                }
            }
        }
    }
    used
}

fn instruction_result_vreg(inst: &Instruction) -> Option<&str> {
    match inst {
        Instruction::Add {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Sub {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Mul {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Mulh {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Div {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Divu {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Rem {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Remu {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::And {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Or {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Xor {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Shl {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Shr {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Sar {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Rotl {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Rotr {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Neg {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Not {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Addi {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Subi {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Muli {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Andi {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Ori {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Xori {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Shli {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Shri {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Sari {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Rotli {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Rotri {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Movi {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Mov {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestEq {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestNe {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestLt {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestLe {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestLtu {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestLeu {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestOf {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestCf {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestSf {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestGe {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestGt {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestGeu {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestGtu {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Load {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Loadi {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::LoadSext {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::LoadZext {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Lea {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::MemXchg {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::AtomicMemXchg {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::AtomicCas {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Pop {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Call {
            result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::CallIndirect {
            result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::Fadd {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fsub {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fmul {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fdiv {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fneg {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fabs {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fsqrt {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fmin {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fmax {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Ffma {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::FcmpEq {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::FcmpNe {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::FcmpLt {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::FcmpLe {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::FcmpGt {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::FcmpGe {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::FcmpOrd {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::FcmpUno {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Vadd {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Vsub {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Vmul {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Vdiv {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Vfma {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Vshuffle {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Vbroadcast {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Vextract {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Vinsert {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::VreduceAdd {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::VreduceMin {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::VreduceMax {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Vload {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Vgather {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Sext {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Zext {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Trunc {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Sitofp {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Uitofp {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fptosi {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fptoui {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fpext {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Fptrunc {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Bitcast {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Cpuid {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Select {
            result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Phi {
            result: Value::VReg { name, .. },
            ..
        } => Some(name),
        _ => None,
    }
}

fn instruction_flags_vreg(inst: &Instruction) -> Option<&str> {
    match inst {
        Instruction::Add {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Sub {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Mul {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Mulh {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Div {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Divu {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Rem {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Remu {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::And {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Or {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Xor {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Shl {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Shr {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Sar {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Rotl {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Rotr {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Neg {
            flags_result: Value::VReg { name, .. },
            ..
        }
        | Instruction::Not {
            flags_result: Value::VReg { name, .. },
            ..
        } => Some(name),
        Instruction::Addi {
            flags_result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::Subi {
            flags_result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::Muli {
            flags_result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::Andi {
            flags_result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::Ori {
            flags_result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::Xori {
            flags_result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::Shli {
            flags_result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::Shri {
            flags_result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::Sari {
            flags_result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::Rotli {
            flags_result: Some(Value::VReg { name, .. }),
            ..
        }
        | Instruction::Rotri {
            flags_result: Some(Value::VReg { name, .. }),
            ..
        } => Some(name),
        _ => None,
    }
}

fn instruction_operands_vreg(inst: &Instruction) -> Vec<&str> {
    let mut ops = Vec::new();
    match inst {
        Instruction::Add {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Sub {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Mul {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Mulh {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Div {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Divu {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Rem {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Remu {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::And {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Or {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Xor {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Shl {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Shr {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Sar {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Rotl {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        }
        | Instruction::Rotr {
            lhs: Value::VReg { name, .. },
            rhs: Value::VReg { name: name2, .. },
            ..
        } => {
            ops.push(name.as_str());
            ops.push(name2.as_str());
        }
        Instruction::Neg {
            operand: Value::VReg { name, .. },
            ..
        }
        | Instruction::Not {
            operand: Value::VReg { name, .. },
            ..
        } => {
            ops.push(name.as_str());
        }
        Instruction::Addi {
            lhs: Value::VReg { name, .. },
            ..
        }
        | Instruction::Subi {
            lhs: Value::VReg { name, .. },
            ..
        }
        | Instruction::Muli {
            lhs: Value::VReg { name, .. },
            ..
        }
        | Instruction::Andi {
            lhs: Value::VReg { name, .. },
            ..
        }
        | Instruction::Ori {
            lhs: Value::VReg { name, .. },
            ..
        }
        | Instruction::Xori {
            lhs: Value::VReg { name, .. },
            ..
        }
        | Instruction::Shli {
            lhs: Value::VReg { name, .. },
            ..
        }
        | Instruction::Shri {
            lhs: Value::VReg { name, .. },
            ..
        }
        | Instruction::Sari {
            lhs: Value::VReg { name, .. },
            ..
        }
        | Instruction::Rotli {
            lhs: Value::VReg { name, .. },
            ..
        }
        | Instruction::Rotri {
            lhs: Value::VReg { name, .. },
            ..
        } => {
            ops.push(name.as_str());
        }
        Instruction::Mov {
            src: Value::VReg { name, .. },
            ..
        } => {
            ops.push(name.as_str());
        }
        Instruction::TestEq {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestNe {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestLt {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestLe {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestLtu {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestLeu {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestOf {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestCf {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestSf {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestGe {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestGt {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestGeu {
            flags: Value::VReg { name, .. },
            ..
        }
        | Instruction::TestGtu {
            flags: Value::VReg { name, .. },
            ..
        } => {
            ops.push(name.as_str());
        }
        _ => {}
    }
    ops
}

// =============================================================================
//  Constant Folding
// =============================================================================

/// Evaluate arithmetic expressions with constant operands at compile time.
pub fn constant_folding(func: &mut Function) {
    for bb in &mut func.basic_blocks {
        for inst in &mut bb.instructions {
            *inst = fold_instruction(inst.clone());
        }
    }
}

fn fold_instruction(inst: Instruction) -> Instruction {
    match inst {
        Instruction::Addi {
            result,
            lhs,
            imm,
            flags_result,
        } => {
            if let Value::ConstInt { value, .. } = &lhs {
                let new_val = value.wrapping_add(imm);
                return Instruction::Movi {
                    result,
                    imm: new_val,
                };
            }
            Instruction::Addi {
                result,
                lhs,
                imm,
                flags_result,
            }
        }
        Instruction::Subi {
            result,
            lhs,
            imm,
            flags_result,
        } => {
            if let Value::ConstInt { value, .. } = &lhs {
                let new_val = value.wrapping_sub(imm);
                return Instruction::Movi {
                    result,
                    imm: new_val,
                };
            }
            Instruction::Subi {
                result,
                lhs,
                imm,
                flags_result,
            }
        }
        Instruction::Muli {
            result,
            lhs,
            imm,
            flags_result,
        } => {
            if let Value::ConstInt { value, .. } = &lhs {
                let new_val = value.wrapping_mul(imm);
                return Instruction::Movi {
                    result,
                    imm: new_val,
                };
            }
            Instruction::Muli {
                result,
                lhs,
                imm,
                flags_result,
            }
        }
        Instruction::Andi {
            result,
            lhs,
            imm,
            flags_result,
        } => {
            if let Value::ConstInt { value, .. } = &lhs {
                return Instruction::Movi {
                    result,
                    imm: value & imm,
                };
            }
            Instruction::Andi {
                result,
                lhs,
                imm,
                flags_result,
            }
        }
        Instruction::Ori {
            result,
            lhs,
            imm,
            flags_result,
        } => {
            if let Value::ConstInt { value, .. } = &lhs {
                return Instruction::Movi {
                    result,
                    imm: value | imm,
                };
            }
            Instruction::Ori {
                result,
                lhs,
                imm,
                flags_result,
            }
        }
        Instruction::Xori {
            result,
            lhs,
            imm,
            flags_result,
        } => {
            if let Value::ConstInt { value, .. } = &lhs {
                return Instruction::Movi {
                    result,
                    imm: value ^ imm,
                };
            }
            Instruction::Xori {
                result,
                lhs,
                imm,
                flags_result,
            }
        }
        Instruction::Shli {
            result,
            lhs,
            imm,
            flags_result,
        } => {
            if let Value::ConstInt { value, .. } = &lhs {
                let new_val = value.wrapping_shl(imm as u32);
                return Instruction::Movi {
                    result,
                    imm: new_val,
                };
            }
            Instruction::Shli {
                result,
                lhs,
                imm,
                flags_result,
            }
        }
        Instruction::Shri {
            result,
            lhs,
            imm,
            flags_result,
        } => {
            if let Value::ConstInt { value, .. } = &lhs {
                let new_val = (*value as u64).wrapping_shr(imm as u32) as i64;
                return Instruction::Movi {
                    result,
                    imm: new_val,
                };
            }
            Instruction::Shri {
                result,
                lhs,
                imm,
                flags_result,
            }
        }
        Instruction::Add {
            result,
            lhs,
            rhs,
            flags_result,
        } => {
            if let (Value::ConstInt { value: v1, .. }, Value::ConstInt { value: v2, .. }) =
                (&lhs, &rhs)
            {
                return Instruction::Movi {
                    result,
                    imm: v1.wrapping_add(*v2),
                };
            }
            Instruction::Add {
                result,
                lhs,
                rhs,
                flags_result,
            }
        }
        Instruction::Sub {
            result,
            lhs,
            rhs,
            flags_result,
        } => {
            if let (Value::ConstInt { value: v1, .. }, Value::ConstInt { value: v2, .. }) =
                (&lhs, &rhs)
            {
                return Instruction::Movi {
                    result,
                    imm: v1.wrapping_sub(*v2),
                };
            }
            Instruction::Sub {
                result,
                lhs,
                rhs,
                flags_result,
            }
        }
        Instruction::Mul {
            result,
            lhs,
            rhs,
            flags_result,
        } => {
            if let (Value::ConstInt { value: v1, .. }, Value::ConstInt { value: v2, .. }) =
                (&lhs, &rhs)
            {
                return Instruction::Movi {
                    result,
                    imm: v1.wrapping_mul(*v2),
                };
            }
            Instruction::Mul {
                result,
                lhs,
                rhs,
                flags_result,
            }
        }
        _ => inst,
    }
}

// =============================================================================
//  Basic Block Merging
// =============================================================================

/// Merge consecutive blocks where the first has only one successor and the
/// second has only one predecessor.
pub fn merge_basic_blocks(func: &mut Function) {
    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i < func.basic_blocks.len() {
            let bb = &func.basic_blocks[i];
            if bb.successors.len() != 1 {
                i += 1;
                continue;
            }
            let succ_name = bb.successors[0].clone();
            // Find the successor block
            let succ_idx = func
                .basic_blocks
                .iter()
                .position(|b| b.name == succ_name);
            if let Some(succ_idx) = succ_idx {
                if succ_idx <= i {
                    i += 1;
                    continue;
                }
                let succ = &func.basic_blocks[succ_idx];
                if succ.predecessors.len() != 1 {
                    i += 1;
                    continue;
                }
                // Don't merge if the first block ends with a terminator that's not a Br
                if !matches!(
                    bb.instructions.last(),
                    Some(Instruction::Br { .. })
                ) {
                    i += 1;
                    continue;
                }
                // Don't merge if succ is entry
                if succ.is_entry {
                    i += 1;
                    continue;
                }
                // Merge: remove the Br from current block, append succ's instructions
                let succ_instructions = func.basic_blocks[succ_idx].instructions.clone();
                let succ_successors = func.basic_blocks[succ_idx].successors.clone();

                // Remove the Br terminator
                func.basic_blocks[i].instructions.pop();
                // Append succ's instructions
                for inst in succ_instructions {
                    func.basic_blocks[i].add_instruction(inst);
                }
                // Update successors
                func.basic_blocks[i].successors = succ_successors;

                // Remove the successor block
                func.basic_blocks.remove(succ_idx);

                // Update predecessors of blocks that were succ's successors
                let merged_name = func.basic_blocks[i].name.clone();
                for bb in &mut func.basic_blocks {
                    for pred in &mut bb.predecessors {
                        if *pred == succ_name {
                            *pred = merged_name.clone();
                        }
                    }
                }

                changed = true;
            }
            i += 1;
        }
    }
}

// =============================================================================
//  Module-level optimization
// =============================================================================

/// Run all optimization passes on a module.
pub fn optimize_module(module: &mut Module) {
    for func in &mut module.functions {
        optimize_function(func);
    }
}

/// Run all optimization passes on a single function.
pub fn optimize_function(func: &mut Function) {
    constant_folding(func);
    dead_code_elimination(func);
    merge_basic_blocks(func);
    // Run DCE again after merging to clean up
    dead_code_elimination(func);
}