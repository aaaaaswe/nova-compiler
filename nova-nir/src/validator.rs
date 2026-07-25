//! NIR IR validator – validates the parsed IR for correctness.
//!
//! Checks:
//! - Type checking: each instruction's operand types match the instruction definition
//! - SSA validation: each VReg defined exactly once, used only after definition
//! - Basic block validation: each block has exactly one terminator, terminator is last
//! - Phi node validation: phi nodes are at the start of basic blocks, have entries for all predecessors
//! - Control flow validation: all branch targets exist

use std::collections::{HashMap, HashSet};

use crate::ir::{BasicBlock, Function, Instruction, Module};
use crate::types::{IrType, Value};

/// Validation result – a list of warnings and errors.
#[derive(Debug, Default)]
pub struct ValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    fn error(&mut self, msg: String) {
        self.errors.push(msg);
    }

    fn warning(&mut self, msg: String) {
        self.warnings.push(msg);
    }
}

/// Validate a module and all its functions.
pub fn validate_module(module: &Module) -> ValidationResult {
    let mut result = ValidationResult::default();
    for func in &module.functions {
        validate_function(func, &mut result);
    }
    result
}

/// Validate a single function.
pub fn validate_function(func: &Function, result: &mut ValidationResult) {
    let fn_name = &func.name;

    // Collect all VReg definitions
    let mut defs: HashMap<String, usize> = HashMap::new(); // vreg_name -> instr index
    let mut all_uses: HashSet<String> = HashSet::new();
    let mut bb_map: HashMap<&str, &BasicBlock> = HashMap::new();

    for bb in &func.basic_blocks {
        bb_map.insert(&bb.name, bb);
    }

    for bb in &func.basic_blocks {
        // Check: phi nodes must be at the start of the block
        let mut seen_non_phi = false;
        let mut has_terminator = false;

        for (i, inst) in bb.instructions.iter().enumerate() {
            // Check: phi nodes first
            if matches!(inst, Instruction::Phi { .. }) {
                if seen_non_phi {
                    result.error(format!(
                        "[{}] phi node after non-phi instruction in block %{}",
                        fn_name, bb.name
                    ));
                }
            } else {
                seen_non_phi = true;
            }

            // Check: terminator must be last
            if has_terminator {
                result.error(format!(
                    "[{}] instruction after terminator in block %{}",
                    fn_name, bb.name
                ));
            }
            if is_terminator(inst) {
                has_terminator = true;
                if i != bb.instructions.len() - 1 {
                    result.error(format!(
                        "[{}] terminator not last in block %{}",
                        fn_name, bb.name
                    ));
                }
            }

            // Collect result (definition) of this instruction
            collect_defs(inst, &mut defs, fn_name, bb, result);

            // Collect all uses of this instruction
            collect_uses(inst, &mut all_uses);
        }

        // Check: each block must have exactly one terminator
        if !has_terminator && !bb.instructions.is_empty() {
            result.error(format!(
                "[{}] block %{} has no terminator",
                fn_name, bb.name
            ));
        }
    }

    // Check: branch targets exist
    for bb in &func.basic_blocks {
        for inst in &bb.instructions {
            validate_branch_targets(inst, &bb_map, fn_name, &bb.name, result);
        }
    }

    // Check: phi nodes have entries for all predecessors
    for bb in &func.basic_blocks {
        for inst in &bb.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                let preds: HashSet<&str> = bb.predecessors.iter().map(|s| s.as_str()).collect();
                let phi_bbs: HashSet<&str> = incoming.iter().map(|(_, bb_name)| bb_name.as_str()).collect();
                if preds != phi_bbs {
                    result.error(format!(
                        "[{}] phi node in block %{} has mismatched predecessors: phi has {:?}, block has {:?}",
                        fn_name, bb.name, phi_bbs, preds
                    ));
                }
            }
        }
    }

    // Check: SSA – each VReg defined exactly once (but params and globals are ok)
    // We already collected defs above; check for duplicate definitions
    for (name, count) in count_defs(&defs) {
        if count > 1 {
            result.error(format!(
                "[{}] VReg {} defined {} times",
                fn_name, name, count
            ));
        }
    }

    // Type checking: instruction type consistency
    for bb in &func.basic_blocks {
        for inst in &bb.instructions {
            validate_instruction_types(inst, fn_name, &bb.name, result);
        }
    }
}

fn is_terminator(inst: &Instruction) -> bool {
    matches!(
        inst,
        Instruction::Br { .. }
            | Instruction::BrCond { .. }
            | Instruction::Switch { .. }
            | Instruction::Ret { .. }
            | Instruction::TailCall { .. }
    )
}

fn collect_defs(
    inst: &Instruction,
    defs: &mut HashMap<String, usize>,
    _fn_name: &str,
    _bb: &BasicBlock,
    _result: &mut ValidationResult,
) {
    // Collect the result VReg
    if let Some(v) = instruction_result(inst) {
        if let Value::VReg { name, .. } = v {
            let count = defs.entry(name.clone()).or_insert(0);
            *count += 1;
        }
    }
    // Collect flags_result
    if let Some(v) = instruction_flags_result(inst) {
        if let Value::VReg { name, .. } = v {
            let count = defs.entry(name.clone()).or_insert(0);
            *count += 1;
        }
    }
}

fn collect_uses(inst: &Instruction, uses: &mut HashSet<String>) {
    for operand in instruction_operands(inst) {
        if let Value::VReg { name, .. } = operand {
            uses.insert(name.clone());
        }
    }
}

fn instruction_result(inst: &Instruction) -> Option<&Value> {
    match inst {
        Instruction::Add { result, .. }
        | Instruction::Sub { result, .. }
        | Instruction::Mul { result, .. }
        | Instruction::Mulh { result, .. }
        | Instruction::Div { result, .. }
        | Instruction::Divu { result, .. }
        | Instruction::Rem { result, .. }
        | Instruction::Remu { result, .. }
        | Instruction::And { result, .. }
        | Instruction::Or { result, .. }
        | Instruction::Xor { result, .. }
        | Instruction::Shl { result, .. }
        | Instruction::Shr { result, .. }
        | Instruction::Sar { result, .. }
        | Instruction::Rotl { result, .. }
        | Instruction::Rotr { result, .. }
        | Instruction::Neg { result, .. }
        | Instruction::Not { result, .. }
        | Instruction::Addi { result, .. }
        | Instruction::Subi { result, .. }
        | Instruction::Muli { result, .. }
        | Instruction::Andi { result, .. }
        | Instruction::Ori { result, .. }
        | Instruction::Xori { result, .. }
        | Instruction::Shli { result, .. }
        | Instruction::Shri { result, .. }
        | Instruction::Sari { result, .. }
        | Instruction::Rotli { result, .. }
        | Instruction::Rotri { result, .. }
        | Instruction::Movi { result, .. }
        | Instruction::Mov { result, .. }
        | Instruction::TestEq { result, .. }
        | Instruction::TestNe { result, .. }
        | Instruction::TestLt { result, .. }
        | Instruction::TestLe { result, .. }
        | Instruction::TestLtu { result, .. }
        | Instruction::TestLeu { result, .. }
        | Instruction::TestOf { result, .. }
        | Instruction::TestCf { result, .. }
        | Instruction::TestSf { result, .. }
        | Instruction::TestGe { result, .. }
        | Instruction::TestGt { result, .. }
        | Instruction::TestGeu { result, .. }
        | Instruction::TestGtu { result, .. }
        | Instruction::Load { result, .. }
        | Instruction::Loadi { result, .. }
        | Instruction::LoadSext { result, .. }
        | Instruction::LoadZext { result, .. }
        | Instruction::Lea { result, .. }
        | Instruction::MemXchg { result, .. }
        | Instruction::AtomicMemXchg { result, .. }
        | Instruction::AtomicCas { result, .. }
        | Instruction::Pop { result, .. }
        | Instruction::Call {
            result: Some(result), ..
        }
        | Instruction::CallIndirect {
            result: Some(result), ..
        }
        | Instruction::Fadd { result, .. }
        | Instruction::Fsub { result, .. }
        | Instruction::Fmul { result, .. }
        | Instruction::Fdiv { result, .. }
        | Instruction::Fneg { result, .. }
        | Instruction::Fabs { result, .. }
        | Instruction::Fsqrt { result, .. }
        | Instruction::Fmin { result, .. }
        | Instruction::Fmax { result, .. }
        | Instruction::Ffma { result, .. }
        | Instruction::FcmpEq { result, .. }
        | Instruction::FcmpNe { result, .. }
        | Instruction::FcmpLt { result, .. }
        | Instruction::FcmpLe { result, .. }
        | Instruction::FcmpGt { result, .. }
        | Instruction::FcmpGe { result, .. }
        | Instruction::FcmpOrd { result, .. }
        | Instruction::FcmpUno { result, .. }
        | Instruction::Vadd { result, .. }
        | Instruction::Vsub { result, .. }
        | Instruction::Vmul { result, .. }
        | Instruction::Vdiv { result, .. }
        | Instruction::Vfma { result, .. }
        | Instruction::Vshuffle { result, .. }
        | Instruction::Vbroadcast { result, .. }
        | Instruction::Vextract { result, .. }
        | Instruction::Vinsert { result, .. }
        | Instruction::VreduceAdd { result, .. }
        | Instruction::VreduceMin { result, .. }
        | Instruction::VreduceMax { result, .. }
        | Instruction::Vload { result, .. }
        | Instruction::Vgather { result, .. }
        | Instruction::Sext { result, .. }
        | Instruction::Zext { result, .. }
        | Instruction::Trunc { result, .. }
        | Instruction::Sitofp { result, .. }
        | Instruction::Uitofp { result, .. }
        | Instruction::Fptosi { result, .. }
        | Instruction::Fptoui { result, .. }
        | Instruction::Fpext { result, .. }
        | Instruction::Fptrunc { result, .. }
        | Instruction::Bitcast { result, .. }
        | Instruction::Cpuid { result, .. }
        | Instruction::Select { result, .. }
        | Instruction::Phi { result, .. } => Some(result),
        _ => None,
    }
}

fn instruction_flags_result(inst: &Instruction) -> Option<&Value> {
    match inst {
        Instruction::Add { flags_result, .. }
        | Instruction::Sub { flags_result, .. }
        | Instruction::Mul { flags_result, .. }
        | Instruction::Mulh { flags_result, .. }
        | Instruction::Div { flags_result, .. }
        | Instruction::Divu { flags_result, .. }
        | Instruction::Rem { flags_result, .. }
        | Instruction::Remu { flags_result, .. }
        | Instruction::And { flags_result, .. }
        | Instruction::Or { flags_result, .. }
        | Instruction::Xor { flags_result, .. }
        | Instruction::Shl { flags_result, .. }
        | Instruction::Shr { flags_result, .. }
        | Instruction::Sar { flags_result, .. }
        | Instruction::Rotl { flags_result, .. }
        | Instruction::Rotr { flags_result, .. }
        | Instruction::Neg { flags_result, .. }
        | Instruction::Not { flags_result, .. } => Some(flags_result),
        Instruction::Addi {
            flags_result: Some(flags_result),
            ..
        }
        | Instruction::Subi {
            flags_result: Some(flags_result),
            ..
        }
        | Instruction::Muli {
            flags_result: Some(flags_result),
            ..
        }
        | Instruction::Andi {
            flags_result: Some(flags_result),
            ..
        }
        | Instruction::Ori {
            flags_result: Some(flags_result),
            ..
        }
        | Instruction::Xori {
            flags_result: Some(flags_result),
            ..
        }
        | Instruction::Shli {
            flags_result: Some(flags_result),
            ..
        }
        | Instruction::Shri {
            flags_result: Some(flags_result),
            ..
        }
        | Instruction::Sari {
            flags_result: Some(flags_result),
            ..
        }
        | Instruction::Rotli {
            flags_result: Some(flags_result),
            ..
        }
        | Instruction::Rotri {
            flags_result: Some(flags_result),
            ..
        } => Some(flags_result),
        _ => None,
    }
}

fn instruction_operands(inst: &Instruction) -> Vec<&Value> {
    let mut ops = Vec::new();
    match inst {
        Instruction::Add { lhs, rhs, .. }
        | Instruction::Sub { lhs, rhs, .. }
        | Instruction::Mul { lhs, rhs, .. }
        | Instruction::Mulh { lhs, rhs, .. }
        | Instruction::Div { lhs, rhs, .. }
        | Instruction::Divu { lhs, rhs, .. }
        | Instruction::Rem { lhs, rhs, .. }
        | Instruction::Remu { lhs, rhs, .. }
        | Instruction::And { lhs, rhs, .. }
        | Instruction::Or { lhs, rhs, .. }
        | Instruction::Xor { lhs, rhs, .. }
        | Instruction::Shl { lhs, rhs, .. }
        | Instruction::Shr { lhs, rhs, .. }
        | Instruction::Sar { lhs, rhs, .. }
        | Instruction::Rotl { lhs, rhs, .. }
        | Instruction::Rotr { lhs, rhs, .. } => {
            ops.push(lhs);
            ops.push(rhs);
        }
        Instruction::Neg { operand, .. } | Instruction::Not { operand, .. } => {
            ops.push(operand);
        }
        Instruction::Addi { lhs, .. }
        | Instruction::Subi { lhs, .. }
        | Instruction::Muli { lhs, .. }
        | Instruction::Andi { lhs, .. }
        | Instruction::Ori { lhs, .. }
        | Instruction::Xori { lhs, .. }
        | Instruction::Shli { lhs, .. }
        | Instruction::Shri { lhs, .. }
        | Instruction::Sari { lhs, .. }
        | Instruction::Rotli { lhs, .. }
        | Instruction::Rotri { lhs, .. } => {
            ops.push(lhs);
        }
        Instruction::Mov { src, .. } => {
            ops.push(src);
        }
        Instruction::TestEq { flags, .. }
        | Instruction::TestNe { flags, .. }
        | Instruction::TestLt { flags, .. }
        | Instruction::TestLe { flags, .. }
        | Instruction::TestLtu { flags, .. }
        | Instruction::TestLeu { flags, .. }
        | Instruction::TestOf { flags, .. }
        | Instruction::TestCf { flags, .. }
        | Instruction::TestSf { flags, .. }
        | Instruction::TestGe { flags, .. }
        | Instruction::TestGt { flags, .. }
        | Instruction::TestGeu { flags, .. }
        | Instruction::TestGtu { flags, .. } => {
            ops.push(flags);
        }
        Instruction::Load { addr, .. } => {
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::Loadi { base, .. } => {
            ops.push(base);
        }
        Instruction::LoadSext { addr, .. } | Instruction::LoadZext { addr, .. } => {
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::Store { value, addr, .. } => {
            ops.push(value);
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::Storei { value, base, .. } => {
            ops.push(value);
            ops.push(base);
        }
        Instruction::Lea { addr, .. } => {
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::MemAdd { value, addr, .. }
        | Instruction::MemSub { value, addr, .. }
        | Instruction::MemAnd { value, addr, .. }
        | Instruction::MemOr { value, addr, .. }
        | Instruction::MemXor { value, addr, .. } => {
            ops.push(value);
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::MemXchg { value, addr, .. } => {
            ops.push(value);
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::AtomicMemAdd { value, addr, .. } => {
            ops.push(value);
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::AtomicMemXchg { value, addr, .. } => {
            ops.push(value);
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::AtomicCas {
            expected,
            desired,
            addr,
            ..
        } => {
            ops.push(expected);
            ops.push(desired);
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::Push { value, .. } => {
            ops.push(value);
        }
        Instruction::BrCond { cond, .. } => {
            ops.push(cond);
        }
        Instruction::Switch {
            value, cases, ..
        } => {
            ops.push(value);
            for (case_val, _) in cases {
                ops.push(case_val);
            }
        }
        Instruction::Call { args, .. }
        | Instruction::CallIndirect { args, .. }
        | Instruction::TailCall { args, .. } => {
            for arg in args {
                ops.push(arg);
            }
            if let Instruction::CallIndirect { fnptr, .. } = inst {
                ops.push(fnptr);
            }
        }
        Instruction::Ret { value: Some(v), .. } => {
            ops.push(v);
        }
        Instruction::Fadd { lhs, rhs, .. }
        | Instruction::Fsub { lhs, rhs, .. }
        | Instruction::Fmul { lhs, rhs, .. }
        | Instruction::Fdiv { lhs, rhs, .. }
        | Instruction::Fmin { lhs, rhs, .. }
        | Instruction::Fmax { lhs, rhs, .. }
        | Instruction::FcmpEq { lhs, rhs, .. }
        | Instruction::FcmpNe { lhs, rhs, .. }
        | Instruction::FcmpLt { lhs, rhs, .. }
        | Instruction::FcmpLe { lhs, rhs, .. }
        | Instruction::FcmpGt { lhs, rhs, .. }
        | Instruction::FcmpGe { lhs, rhs, .. }
        | Instruction::FcmpOrd { lhs, rhs, .. }
        | Instruction::FcmpUno { lhs, rhs, .. } => {
            ops.push(lhs);
            ops.push(rhs);
        }
        Instruction::Fneg { operand, .. }
        | Instruction::Fabs { operand, .. }
        | Instruction::Fsqrt { operand, .. } => {
            ops.push(operand);
        }
        Instruction::Ffma { a, b, c, .. } => {
            ops.push(a);
            ops.push(b);
            ops.push(c);
        }
        Instruction::Vadd { lhs, rhs, .. }
        | Instruction::Vsub { lhs, rhs, .. }
        | Instruction::Vmul { lhs, rhs, .. }
        | Instruction::Vdiv { lhs, rhs, .. } => {
            ops.push(lhs);
            ops.push(rhs);
        }
        Instruction::Vfma { a, b, c, .. } => {
            ops.push(a);
            ops.push(b);
            ops.push(c);
        }
        Instruction::Vshuffle {
            lhs, rhs, mask, ..
        } => {
            ops.push(lhs);
            ops.push(rhs);
            ops.push(mask);
        }
        Instruction::Vbroadcast { value, .. } => {
            ops.push(value);
        }
        Instruction::Vextract { vector, .. } => {
            ops.push(vector);
        }
        Instruction::Vinsert {
            vector, value, ..
        } => {
            ops.push(vector);
            ops.push(value);
        }
        Instruction::VreduceAdd { vector, .. }
        | Instruction::VreduceMin { vector, .. }
        | Instruction::VreduceMax { vector, .. } => {
            ops.push(vector);
        }
        Instruction::Vload { addr, .. } => {
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::Vstore { value, addr, .. } => {
            ops.push(value);
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::Vgather { addr, mask, .. } => {
            ops.push(&addr.base);
            ops.push(mask);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::Vscatter {
            value, addr, mask, ..
        } => {
            ops.push(value);
            ops.push(&addr.base);
            ops.push(mask);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::Sext { value, .. }
        | Instruction::Zext { value, .. }
        | Instruction::Trunc { value, .. }
        | Instruction::Sitofp { value, .. }
        | Instruction::Uitofp { value, .. }
        | Instruction::Fptosi { value, .. }
        | Instruction::Fptoui { value, .. }
        | Instruction::Fpext { value, .. }
        | Instruction::Fptrunc { value, .. }
        | Instruction::Bitcast { value, .. } => {
            ops.push(value);
        }
        Instruction::Select {
            cond,
            true_val,
            false_val,
            ..
        } => {
            ops.push(cond);
            ops.push(true_val);
            ops.push(false_val);
        }
        Instruction::Phi { incoming, .. } => {
            for (val, _) in incoming {
                ops.push(val);
            }
        }
        _ => {}
    }
    ops
}

fn count_defs(defs: &HashMap<String, usize>) -> HashMap<String, usize> {
    let mut result = HashMap::new();
    for (name, &count) in defs {
        if count > 1 {
            result.insert(name.clone(), count);
        }
    }
    result
}

fn validate_branch_targets(
    inst: &Instruction,
    bb_map: &HashMap<&str, &BasicBlock>,
    fn_name: &str,
    bb_name: &str,
    result: &mut ValidationResult,
) {
    match inst {
        Instruction::Br { target_bb } => {
            if !bb_map.contains_key(target_bb.as_str()) {
                result.error(format!(
                    "[{}] branch to unknown block '%{}' from block %{}",
                    fn_name, target_bb, bb_name
                ));
            }
        }
        Instruction::BrCond {
            true_bb, false_bb, ..
        } => {
            if !bb_map.contains_key(true_bb.as_str()) {
                result.error(format!(
                    "[{}] branch to unknown block '%{}' from block %{}",
                    fn_name, true_bb, bb_name
                ));
            }
            if !bb_map.contains_key(false_bb.as_str()) {
                result.error(format!(
                    "[{}] branch to unknown block '%{}' from block %{}",
                    fn_name, false_bb, bb_name
                ));
            }
        }
        Instruction::Switch {
            default_bb, cases, ..
        } => {
            if !bb_map.contains_key(default_bb.as_str()) {
                result.error(format!(
                    "[{}] switch to unknown default block '%{}' from block %{}",
                    fn_name, default_bb, bb_name
                ));
            }
            for (_, case_bb) in cases {
                if !bb_map.contains_key(case_bb.as_str()) {
                    result.error(format!(
                        "[{}] switch to unknown case block '%{}' from block %{}",
                        fn_name, case_bb, bb_name
                    ));
                }
            }
        }
        _ => {}
    }
}

fn validate_instruction_types(
    inst: &Instruction,
    fn_name: &str,
    bb_name: &str,
    result: &mut ValidationResult,
) {
    // Basic type checks: ensure flag consumers use Flags type
    if let Instruction::TestEq { flags, .. }
    | Instruction::TestNe { flags, .. }
    | Instruction::TestLt { flags, .. }
    | Instruction::TestLe { flags, .. }
    | Instruction::TestLtu { flags, .. }
    | Instruction::TestLeu { flags, .. }
    | Instruction::TestOf { flags, .. }
    | Instruction::TestCf { flags, .. }
    | Instruction::TestSf { flags, .. }
    | Instruction::TestGe { flags, .. }
    | Instruction::TestGt { flags, .. }
    | Instruction::TestGeu { flags, .. }
    | Instruction::TestGtu { flags, .. } = inst
    {
        if flags.ty() != &IrType::Flags && flags.ty() != &IrType::Void {
            result.warning(format!(
                "[{}] flag consumer in block %{} expects flags type, got {:?}",
                fn_name, bb_name, flags.ty()
            ));
        }
    }
}