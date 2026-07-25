/// Linear scan register allocator for MacroCore-X.
///
/// Maps VRegs to physical registers R8-R23 (16 total).
/// R1 = first parameter, R3-R8 = remaining parameters (R2 = SP, skipped),
/// R0 = hardwired zero, R2 = SP, R30 = FP, R31 = RA.
use std::collections::HashMap;

/// Map parameter index to physical register, skipping r2 (SP).
/// Parameter 0 → r1, 1 → r3, 2 → r4, ..., 6 → r8.
pub fn param_reg(index: usize) -> usize {
    if index == 0 { 1 } else { index + 2 }
}

use nova_nir::ir::{Function, Instruction};
use nova_nir::types::Value;

/// Live interval for a virtual register.
#[derive(Debug, Clone)]
struct LiveInterval {
    vreg_name: String,
    start: usize, // position of first definition
    end: usize,   // position of last use
}

/// Linear scan register allocator.
pub struct RegisterAllocator {
    /// VReg name -> physical register number
    vreg_to_preg: HashMap<String, usize>,
    /// VReg name -> LiveInterval
    intervals: HashMap<String, LiveInterval>,
    /// Spill slot assignments (vreg_name -> slot index)
    spill_slots: HashMap<String, usize>,
}

impl RegisterAllocator {
    /// Allocatable registers: R8-R15 (8 registers, within 4-bit encoding).
    pub const ALLOCATABLE_REGS: [usize; 8] = [8, 9, 10, 11, 12, 13, 14, 15];

    pub fn new() -> Self {
        RegisterAllocator {
            vreg_to_preg: HashMap::new(),
            intervals: HashMap::new(),
            spill_slots: HashMap::new(),
        }
    }

    /// Reset the allocator for a new function.
    pub fn reset(&mut self) {
        self.vreg_to_preg.clear();
        self.intervals.clear();
        self.spill_slots.clear();
    }

    /// Allocate physical registers for all VRegs in a function.
    pub fn allocate(&mut self, func: &Function) {
        self.reset();

        // Map parameters to R1, R3-R8 (skip R2=SP)
        for (i, param) in func.parameters.iter().enumerate() {
            if i < 7 {
                if let Value::FuncParam { ref name, .. } = param {
                    self.vreg_to_preg.insert(name.clone(), param_reg(i));
                }
            }
        }

        // Compute live intervals
        self.compute_live_intervals(func);

        // Sort intervals by start position
        let mut sorted_intervals: Vec<LiveInterval> =
            self.intervals.values().cloned().collect();
        sorted_intervals.sort_by_key(|iv| (iv.start, iv.end));

        // Linear scan allocation
        let mut active: Vec<(LiveInterval, usize)> = Vec::new();

        for interval in &sorted_intervals {
            if self.vreg_to_preg.contains_key(&interval.vreg_name) {
                continue;
            }

            // Expire old intervals
            active.retain(|(a_iv, _)| a_iv.end >= interval.start);

            let used_regs: Vec<usize> = active.iter().map(|(_, r)| *r).collect();
            let free_regs: Vec<usize> = Self::ALLOCATABLE_REGS
                .iter()
                .filter(|r| !used_regs.contains(r))
                .copied()
                .collect();

            if !free_regs.is_empty() {
                let reg = free_regs[0];
                self.vreg_to_preg.insert(interval.vreg_name.clone(), reg);
                active.push((interval.clone(), reg));
            } else {
                // Spill the interval with the farthest end
                let spill_idx = active
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, (a_iv, _))| a_iv.end)
                    .map(|(i, _)| i)
                    .unwrap_or(0);

                let spill_iv = &active[spill_idx].0;
                let spill_reg = active[spill_idx].1;

                if spill_iv.end > interval.end {
                    self.vreg_to_preg.remove(&spill_iv.vreg_name);
                    self.vreg_to_preg.insert(interval.vreg_name.clone(), spill_reg);
                    active[spill_idx] = (interval.clone(), spill_reg);
                }
                // Otherwise, interval is spilled (not assigned a register)
            }
        }

        // Post-allocation: ensure all VRegs in all instructions are mapped.
        // This catches any VRegs missed by live interval analysis.
        self.ensure_all_allocated(func);
    }

    /// Ensure all VRegs in a function have physical register assignments.
    fn ensure_all_allocated(&mut self, func: &Function) {
        for bb in &func.basic_blocks {
            for inst in &bb.instructions {
                if let Some(result) = inst_result_vreg(inst) {
                    self.ensure_allocated(&result);
                }
                if let Some(flags) = inst_flags_result_vreg(inst) {
                    self.ensure_allocated(&flags);
                }
                for op in inst_operands(inst) {
                    match op {
                        Value::VReg { ref name, .. } | Value::FuncParam { ref name, .. } => {
                            self.ensure_allocated(name);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Get the physical register number for a VReg, or None.
    pub fn get_reg(&self, vreg: &Value) -> Option<usize> {
        match vreg {
            Value::VReg { ref name, .. } => self.vreg_to_preg.get(name).copied(),
            Value::FuncParam { ref name, .. } => self.vreg_to_preg.get(name).copied(),
            _ => None,
        }
    }

    /// Get the register name as an assembly string (e.g. "r8").
    /// If the VReg hasn't been allocated yet, allocates one on demand.
    pub fn get_reg_name(&mut self, vreg: &Value) -> String {
        match vreg {
            Value::ConstInt { value, .. } => value.to_string(),
            Value::ConstFloat { value, .. } => value.to_string(),
            Value::GlobalVar { ref name, .. } => format!("@{}", name),
            Value::FuncParam { ref name, .. } | Value::VReg { ref name, .. } => {
                if let Some(preg) = self.vreg_to_preg.get(name) {
                    format!("r{}", preg)
                } else {
                    self.ensure_allocated(name)
                }
            }
        }
    }

    /// Check if a VReg has been allocated a physical register.
    pub fn is_allocated(&self, vreg: &Value) -> bool {
        match vreg {
            Value::ConstInt { .. } | Value::ConstFloat { .. } | Value::GlobalVar { .. } => true,
            Value::FuncParam { ref name, .. } | Value::VReg { ref name, .. } => {
                self.vreg_to_preg.contains_key(name)
            }
        }
    }

    /// Ensure a VReg has a physical register, allocating one if possible.
    /// Returns the physical register name (e.g. "r8") or a spill slot reference.
    pub fn ensure_allocated(&mut self, vreg_name: &str) -> String {
        if let Some(preg) = self.vreg_to_preg.get(vreg_name) {
            return format!("r{}", preg);
        }
        // Find a free register
        let used: Vec<usize> = self.vreg_to_preg.values().copied().collect();
        for r in &Self::ALLOCATABLE_REGS {
            if !used.contains(r) {
                self.vreg_to_preg.insert(vreg_name.to_string(), *r);
                return format!("r{}", r);
            }
        }
        // Fallback: spill slot
        let slot = self.spill_slots.len();
        self.spill_slots.insert(vreg_name.to_string(), slot);
        format!("[r15 + {}]", slot * 8)
    }

    /// Compute live intervals for all VRegs in a function.
    fn compute_live_intervals(&mut self, func: &Function) {
        // positions: vreg_name -> (first_def, last_use)
        let mut positions: HashMap<String, (Option<usize>, usize)> = HashMap::new();

        let record_def = |positions: &mut HashMap<String, (Option<usize>, usize)>, name: &str, pos: usize| {
            let entry = positions.entry(name.to_string()).or_insert((None, pos));
            if let Some(def) = entry.0 {
                entry.0 = Some(def.min(pos));
            } else {
                entry.0 = Some(pos);
            }
        };

        let record_use = |positions: &mut HashMap<String, (Option<usize>, usize)>, name: &str, pos: usize| {
            let entry = positions.entry(name.to_string()).or_insert((None, pos));
            entry.1 = entry.1.max(pos);
        };

        // First pass: assign positions
        let mut pos = 0usize;
        let mut block_end_pos: HashMap<String, usize> = HashMap::new();

        for param in &func.parameters {
            let name = param_name(param);
            record_def(&mut positions, &name, 0);
        }

        for bb in &func.basic_blocks {
            for inst in &bb.instructions {
                // Record defs
                if let Some(result) = inst_result_vreg(inst) {
                    record_def(&mut positions, &result, pos);
                }
                if let Some(flags) = inst_flags_result_vreg(inst) {
                    record_def(&mut positions, &flags, pos);
                }

                // Record uses
                for op in inst_operands(inst) {
                    if let Value::VReg { ref name, .. } = op {
                        record_use(&mut positions, name, pos);
                    }
                }

                pos += 1;
            }
            if !bb.instructions.is_empty() {
                block_end_pos.insert(bb.name.clone(), pos - 1);
            }
        }

        // Second pass: record phi uses at predecessor block terminator position
        pos = 0;
        for bb in &func.basic_blocks {
            for inst in &bb.instructions {
                if let Instruction::Phi { ref incoming, .. } = inst {
                    for (val, pred_bb_name) in incoming {
                        if let Value::VReg { ref name, .. } = val {
                            if let Some(&pred_end) = block_end_pos.get(pred_bb_name) {
                                record_use(&mut positions, name, pred_end);
                            } else {
                                record_use(&mut positions, name, pos);
                            }
                        }
                    }
                }
                pos += 1;
            }
        }

        // Build intervals
        for (name, (def_pos, last_use)) in positions {
            if let Some(def) = def_pos {
                let end = if last_use >= def { last_use } else { def };
                self.intervals.insert(
                    name.clone(),
                    LiveInterval {
                        vreg_name: name,
                        start: def,
                        end,
                    },
                );
            }
        }
    }
}

/// Extract the VReg name from a function parameter value.
fn param_name(val: &Value) -> String {
    match val {
        Value::FuncParam { ref name, .. } => name.clone(),
        _ => String::new(),
    }
}

/// Get the result VReg name from an instruction, if it has one.
fn inst_result_vreg(inst: &Instruction) -> Option<String> {
    let result = match inst {
        Instruction::Add { ref result, .. }
        | Instruction::Sub { ref result, .. }
        | Instruction::Mul { ref result, .. }
        | Instruction::Mulh { ref result, .. }
        | Instruction::Div { ref result, .. }
        | Instruction::Divu { ref result, .. }
        | Instruction::Rem { ref result, .. }
        | Instruction::Remu { ref result, .. }
        | Instruction::And { ref result, .. }
        | Instruction::Or { ref result, .. }
        | Instruction::Xor { ref result, .. }
        | Instruction::Shl { ref result, .. }
        | Instruction::Shr { ref result, .. }
        | Instruction::Sar { ref result, .. }
        | Instruction::Rotl { ref result, .. }
        | Instruction::Rotr { ref result, .. }
        | Instruction::Neg { ref result, .. }
        | Instruction::Not { ref result, .. }
        | Instruction::Addi { ref result, .. }
        | Instruction::Subi { ref result, .. }
        | Instruction::Muli { ref result, .. }
        | Instruction::Andi { ref result, .. }
        | Instruction::Ori { ref result, .. }
        | Instruction::Xori { ref result, .. }
        | Instruction::Shli { ref result, .. }
        | Instruction::Shri { ref result, .. }
        | Instruction::Sari { ref result, .. }
        | Instruction::Rotli { ref result, .. }
        | Instruction::Rotri { ref result, .. }
        | Instruction::Movi { ref result, .. }
        | Instruction::Mov { ref result, .. }
        | Instruction::TestEq { ref result, .. }
        | Instruction::TestNe { ref result, .. }
        | Instruction::TestLt { ref result, .. }
        | Instruction::TestLe { ref result, .. }
        | Instruction::TestLtu { ref result, .. }
        | Instruction::TestLeu { ref result, .. }
        | Instruction::TestOf { ref result, .. }
        | Instruction::TestCf { ref result, .. }
        | Instruction::TestSf { ref result, .. }
        | Instruction::TestGe { ref result, .. }
        | Instruction::TestGt { ref result, .. }
        | Instruction::TestGeu { ref result, .. }
        | Instruction::TestGtu { ref result, .. }
        | Instruction::Load { ref result, .. }
        | Instruction::Loadi { ref result, .. }
        | Instruction::LoadSext { ref result, .. }
        | Instruction::LoadZext { ref result, .. }
        | Instruction::Lea { ref result, .. }
        | Instruction::MemXchg { ref result, .. }
        | Instruction::AtomicMemXchg { ref result, .. }
        | Instruction::AtomicCas { ref result, .. }
        | Instruction::Pop { ref result, .. }
        | Instruction::Call { result: Some(ref result), .. }
        | Instruction::CallIndirect { result: Some(ref result), .. }
        | Instruction::Fadd { ref result, .. }
        | Instruction::Fsub { ref result, .. }
        | Instruction::Fmul { ref result, .. }
        | Instruction::Fdiv { ref result, .. }
        | Instruction::Fneg { ref result, .. }
        | Instruction::Fabs { ref result, .. }
        | Instruction::Fsqrt { ref result, .. }
        | Instruction::Fmin { ref result, .. }
        | Instruction::Fmax { ref result, .. }
        | Instruction::Ffma { ref result, .. }
        | Instruction::FcmpEq { ref result, .. }
        | Instruction::FcmpNe { ref result, .. }
        | Instruction::FcmpLt { ref result, .. }
        | Instruction::FcmpLe { ref result, .. }
        | Instruction::FcmpGt { ref result, .. }
        | Instruction::FcmpGe { ref result, .. }
        | Instruction::FcmpOrd { ref result, .. }
        | Instruction::FcmpUno { ref result, .. }
        | Instruction::Vadd { ref result, .. }
        | Instruction::Vsub { ref result, .. }
        | Instruction::Vmul { ref result, .. }
        | Instruction::Vdiv { ref result, .. }
        | Instruction::Vfma { ref result, .. }
        | Instruction::Vshuffle { ref result, .. }
        | Instruction::Vbroadcast { ref result, .. }
        | Instruction::Vextract { ref result, .. }
        | Instruction::Vinsert { ref result, .. }
        | Instruction::VreduceAdd { ref result, .. }
        | Instruction::VreduceMin { ref result, .. }
        | Instruction::VreduceMax { ref result, .. }
        | Instruction::Vload { ref result, .. }
        | Instruction::Vgather { ref result, .. }
        | Instruction::Sext { ref result, .. }
        | Instruction::Zext { ref result, .. }
        | Instruction::Trunc { ref result, .. }
        | Instruction::Sitofp { ref result, .. }
        | Instruction::Uitofp { ref result, .. }
        | Instruction::Fptosi { ref result, .. }
        | Instruction::Fptoui { ref result, .. }
        | Instruction::Fpext { ref result, .. }
        | Instruction::Fptrunc { ref result, .. }
        | Instruction::Bitcast { ref result, .. }
        | Instruction::Cpuid { ref result, .. }
        | Instruction::Select { ref result, .. }
        | Instruction::Phi { ref result, .. } => Some(result),
        _ => None,
    };
    result.and_then(|v| {
        if let Value::VReg { ref name, .. } = v {
            Some(name.clone())
        } else {
            None
        }
    })
}

/// Get the flags result VReg name from an instruction, if it has one.
fn inst_flags_result_vreg(inst: &Instruction) -> Option<String> {
    let flags = match inst {
        Instruction::Add {
            ref flags_result, ..
        }
        | Instruction::Sub {
            ref flags_result, ..
        }
        | Instruction::Mul {
            ref flags_result, ..
        }
        | Instruction::Mulh {
            ref flags_result, ..
        }
        | Instruction::Div {
            ref flags_result, ..
        }
        | Instruction::Divu {
            ref flags_result, ..
        }
        | Instruction::Rem {
            ref flags_result, ..
        }
        | Instruction::Remu {
            ref flags_result, ..
        }
        | Instruction::And {
            ref flags_result, ..
        }
        | Instruction::Or {
            ref flags_result, ..
        }
        | Instruction::Xor {
            ref flags_result, ..
        }
        | Instruction::Shl {
            ref flags_result, ..
        }
        | Instruction::Shr {
            ref flags_result, ..
        }
        | Instruction::Sar {
            ref flags_result, ..
        }
        | Instruction::Rotl {
            ref flags_result, ..
        }
        | Instruction::Rotr {
            ref flags_result, ..
        }
        | Instruction::Neg {
            ref flags_result, ..
        }
        | Instruction::Not {
            ref flags_result, ..
        } => Some(flags_result),
        Instruction::Addi {
            flags_result: Some(ref f),
            ..
        }
        | Instruction::Subi {
            flags_result: Some(ref f),
            ..
        }
        | Instruction::Muli {
            flags_result: Some(ref f),
            ..
        }
        | Instruction::Andi {
            flags_result: Some(ref f),
            ..
        }
        | Instruction::Ori {
            flags_result: Some(ref f),
            ..
        }
        | Instruction::Xori {
            flags_result: Some(ref f),
            ..
        }
        | Instruction::Shli {
            flags_result: Some(ref f),
            ..
        }
        | Instruction::Shri {
            flags_result: Some(ref f),
            ..
        }
        | Instruction::Sari {
            flags_result: Some(ref f),
            ..
        }
        | Instruction::Rotli {
            flags_result: Some(ref f),
            ..
        }
        | Instruction::Rotri {
            flags_result: Some(ref f),
            ..
        } => Some(f),
        _ => None,
    };
    flags.and_then(|v| {
        if let Value::VReg { ref name, .. } = v {
            Some(name.clone())
        } else {
            None
        }
    })
}

/// Get all operand values from an instruction.
fn inst_operands(inst: &Instruction) -> Vec<&Value> {
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
        Instruction::Load { ref addr, .. }
        | Instruction::Store { ref addr, .. }
        | Instruction::LoadSext { ref addr, .. }
        | Instruction::LoadZext { ref addr, .. }
        | Instruction::Vload { ref addr, .. }
        | Instruction::Vstore { ref addr, .. } => {
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
        }
        Instruction::Loadi { base, .. } | Instruction::Storei { base, .. } => {
            ops.push(base);
        }
        Instruction::MemAdd { ref addr, value, .. }
        | Instruction::MemSub { ref addr, value, .. }
        | Instruction::MemAnd { ref addr, value, .. }
        | Instruction::MemOr { ref addr, value, .. }
        | Instruction::MemXor { ref addr, value, .. }
        | Instruction::MemXchg { ref addr, value, .. }
        | Instruction::AtomicMemAdd { ref addr, value, .. }
        | Instruction::AtomicMemXchg { ref addr, value, .. } => {
            ops.push(&addr.base);
            if let Some(ref idx) = addr.index {
                ops.push(idx);
            }
            ops.push(value);
        }
        Instruction::AtomicCas { ref addr, expected, desired, .. } => {
            ops.push(&addr.base);
            ops.push(expected);
            ops.push(desired);
        }
        Instruction::Push { value } => {
            ops.push(value);
        }
        Instruction::BrCond { cond, .. } => {
            ops.push(cond);
        }
        Instruction::Switch { value, cases, .. } => {
            ops.push(value);
            for (cv, _) in cases {
                ops.push(cv);
            }
        }
        Instruction::Call { ref args, .. }
        | Instruction::TailCall { ref args, .. } => {
            for a in args {
                ops.push(a);
            }
        }
        Instruction::CallIndirect { ref args, fnptr, .. } => {
            for a in args {
                ops.push(a);
            }
            ops.push(fnptr);
        }
        Instruction::Ret { value: Some(ref v) } => {
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
        Instruction::Vshuffle { lhs, rhs, mask, .. } => {
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
        Instruction::Vinsert { vector, value, .. } => {
            ops.push(vector);
            ops.push(value);
        }
        Instruction::VreduceAdd { vector, .. }
        | Instruction::VreduceMin { vector, .. }
        | Instruction::VreduceMax { vector, .. } => {
            ops.push(vector);
        }
        Instruction::Vgather { ref addr, mask, .. } => {
            ops.push(&addr.base);
            ops.push(mask);
        }
        Instruction::Vscatter { value, ref addr, mask, .. } => {
            ops.push(value);
            ops.push(&addr.base);
            ops.push(mask);
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
        Instruction::Select { cond, true_val, false_val, .. } => {
            ops.push(cond);
            ops.push(true_val);
            ops.push(false_val);
        }
        Instruction::Phi { incoming, .. } => {
            for (v, _) in incoming {
                ops.push(v);
            }
        }
        _ => {}
    }
    ops
}