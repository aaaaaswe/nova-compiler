/// Hybrid code generator: intelligently selects between RISC and CISC
/// strategies for each instruction.
///
/// Strategy:
/// - enter/leave/push/pop: Always CISC
/// - Flag pattern matching (test_* + br_cond): Always CISC
/// - mem_add/mem_sub/mem_xchg: CISC when register pressure < 10
/// - mem_and/mem_or/mem_xor: Always RISC expansion
/// - Regular arithmetic: Direct emission
use std::collections::{HashMap, HashSet};

use nova_nir::ir::{AddrExpr, BasicBlock, Function, Instruction, Module};
use nova_nir::types::{IrType, Value};

use crate::regalloc::RegisterAllocator;
use crate::regalloc;

/// Register pressure tracker.
pub struct RegisterPressureTracker {
    /// Live VReg count at each instruction position.
    live_at_pos: HashMap<usize, usize>,
}

impl RegisterPressureTracker {
    /// Threshold for CISC composite memory instructions.
    pub const CISC_THRESHOLD: usize = 10;

    pub fn new() -> Self {
        RegisterPressureTracker {
            live_at_pos: HashMap::new(),
        }
    }

    /// Compute live register count at each instruction position.
    pub fn compute(&mut self, func: &Function) {
        self.live_at_pos.clear();
        // Build live intervals
        let mut intervals: HashMap<String, (usize, usize)> = HashMap::new();
        let mut pos = 0usize;

        for param in &func.parameters {
            if let Value::FuncParam { ref name, .. } = param {
                intervals.insert(name.clone(), (0, 0));
            }
        }

        for bb in &func.basic_blocks {
            for inst in &bb.instructions {
                // Record defs
                if let Some(result_name) = get_result_vreg_name(inst) {
                    let entry = intervals.entry(result_name.clone()).or_insert((pos, pos));
                    entry.1 = pos;
                }
                if let Some(flags_name) = get_flags_vreg_name(inst) {
                    let entry = intervals.entry(flags_name.clone()).or_insert((pos, pos));
                    entry.1 = pos;
                }

                // Record uses
                for op in get_inst_operands(inst) {
                    if let Value::VReg { ref name, .. } = op {
                        let entry = intervals.entry(name.clone()).or_insert((pos, pos));
                        entry.1 = pos;
                    }
                }

                pos += 1;
            }
        }

        // Compute live count at each position
        let total_positions = pos;
        for p in 0..total_positions {
            let mut count = 0usize;
            for (def_pos, last_use) in intervals.values() {
                if *def_pos <= p && p <= *last_use {
                    count += 1;
                }
            }
            self.live_at_pos.insert(p, count);
        }
    }

    /// Get the number of live VRegs at a given position.
    pub fn get_live_count(&self, position: usize) -> usize {
        self.live_at_pos.get(&position).copied().unwrap_or(0)
    }

    /// Check if register pressure is low enough for CISC.
    pub fn is_pressure_low(&self, position: usize) -> bool {
        self.get_live_count(position) < Self::CISC_THRESHOLD
    }
}

/// Hybrid code generator.
pub struct HybridGenerator {
    /// Output assembly lines.
    asm_lines: Vec<String>,
    /// Label counter.
    label_counter: usize,
    /// Register allocator.
    reg_allocator: RegisterAllocator,
    /// Pressure tracker.
    pressure_tracker: RegisterPressureTracker,
    /// Flag-to-arithmetic mapping.
    flags_to_arith: HashMap<String, (String, Value, Value)>,
    /// Test-to-flags mapping.
    test_to_flags: HashMap<String, (String, String)>,
    /// Optimized br_cond set.
    br_cond_optimized: HashSet<String>,
    /// Phi elimination inserts.
    phi_inserts: HashMap<String, Vec<String>>,
    /// Loop-back phi inserts.
    phi_loop_inserts: HashMap<String, Vec<String>>,
    /// Current block name.
    current_block_name: String,
    /// Current function name.
    current_func_name: String,
    /// Instruction position.
    inst_position: usize,
}

impl HybridGenerator {
    /// Mapping from test opcode to branch opcode.
    const TEST_TO_BRANCH: &'static [(&'static str, &'static str)] = &[
        ("test_eq", "beq"),
        ("test_ne", "bne"),
        ("test_lt", "blt"),
        ("test_le", "ble"),
        ("test_ltu", "bltu"),
        ("test_leu", "bleu"),
        ("test_ge", "bge"),
        ("test_gt", "bgt"),
        ("test_geu", "bgeu"),
        ("test_gtu", "bgtu"),
    ];

    pub fn new() -> Self {
        HybridGenerator {
            asm_lines: Vec::new(),
            label_counter: 0,
            reg_allocator: RegisterAllocator::new(),
            pressure_tracker: RegisterPressureTracker::new(),
            flags_to_arith: HashMap::new(),
            test_to_flags: HashMap::new(),
            br_cond_optimized: HashSet::new(),
            phi_inserts: HashMap::new(),
            phi_loop_inserts: HashMap::new(),
            current_block_name: String::new(),
            current_func_name: String::new(),
            inst_position: 0,
        }
    }

    fn get_test_to_branch(test_op: &str) -> Option<&'static str> {
        for (t, b) in Self::TEST_TO_BRANCH {
            if *t == test_op {
                return Some(b);
            }
        }
        None
    }

    /// Generate hybrid assembly for the entire module.
    pub fn generate(&mut self, module: &Module) -> String {
        self.asm_lines.clear();
        self.asm_lines.push("; Generated by NIR Hybrid Code Generator (default)".to_string());
        self.asm_lines.push(format!("; Module: {}", module.name));
        self.asm_lines.push(format!("; Target: {}", module.target_triple));
        self.asm_lines.push("; Strategy: CISC for stack/flag ops, selective for mem compos,".to_string());
        self.asm_lines.push(";          RISC expansion when register pressure is high".to_string());
        self.asm_lines.push(String::new());

        if !module.functions.is_empty() {
            let entry_name = module.functions.iter()
            .find(|f| f.name == "main")
            .map(|f| f.name.as_str())
            .unwrap_or(&module.functions[0].name);
        self.asm_lines.push("; Entry point".to_string());
        self.asm_lines.push("_start:".to_string());
        self.asm_lines.push(format!("    call .L_{}", entry_name));
            self.asm_lines.push("    ecall 0".to_string());
            self.asm_lines.push("    nop".to_string());
            self.asm_lines.push(String::new());
        }

        for func in &module.functions {
            self.generate_function(func);
        }

        if !module.globals.is_empty() {
            self.asm_lines.push("; Global variables".to_string());
            for gvar in &module.globals {
                if let Value::GlobalVar { ref name, .. } = gvar {
                    self.asm_lines.push(format!("@{}:", name));
                    self.asm_lines.push("    .word 0".to_string());
                }
            }
            self.asm_lines.push(String::new());
        }

        self.asm_lines.join("\n")
    }

    /// Generate hybrid assembly for a single function.
    fn generate_function(&mut self, func: &Function) {
        self.flags_to_arith.clear();
        self.test_to_flags.clear();
        self.br_cond_optimized.clear();
        self.phi_inserts.clear();
        self.phi_loop_inserts.clear();
        self.inst_position = 0;
        self.current_func_name = func.name.clone();

        // Pass 1: collect flag-optimization information
        self.collect_flag_info(func);

        // Pass 2: compute register pressure
        self.pressure_tracker.compute(func);

        // Pass 3: allocate registers
        self.reg_allocator.allocate(func);

        // Pass 4: eliminate phi nodes
        self.eliminate_phi_nodes(func);

        // Pass 5: emit assembly
        self.asm_lines.push(format!("; Function: @{}", func.name));
        self.asm_lines.push(format!(".L_{}:", func.name));

        for bb in &func.basic_blocks {
            self.emit_block(bb, func);
        }

        self.asm_lines.push(String::new());
    }

    /// Pass 1: Collect flag optimization information.
    fn collect_flag_info(&mut self, func: &Function) {
        for bb in &func.basic_blocks {
            for inst in &bb.instructions {
                match inst {
                    Instruction::Add { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Sub { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Mul { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Mulh { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Div { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Divu { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Rem { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Remu { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::And { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Or { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Xor { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Shl { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Shr { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Sar { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Rotl { ref flags_result, ref lhs, ref rhs, .. }
                    | Instruction::Rotr { ref flags_result, ref lhs, ref rhs, .. } => {
                        if let Value::VReg { ref name, .. } = flags_result {
                            self.flags_to_arith.insert(
                                name.clone(),
                                (inst.opcode().to_string(), lhs.clone(), rhs.clone()),
                            );
                        }
                    }
                    Instruction::Addi { flags_result: Some(ref flags_result), ref lhs, imm, .. }
                    | Instruction::Subi { flags_result: Some(ref flags_result), ref lhs, imm, .. }
                    | Instruction::Muli { flags_result: Some(ref flags_result), ref lhs, imm, .. }
                    | Instruction::Andi { flags_result: Some(ref flags_result), ref lhs, imm, .. }
                    | Instruction::Ori { flags_result: Some(ref flags_result), ref lhs, imm, .. }
                    | Instruction::Xori { flags_result: Some(ref flags_result), ref lhs, imm, .. }
                    | Instruction::Shli { flags_result: Some(ref flags_result), ref lhs, imm, .. }
                    | Instruction::Shri { flags_result: Some(ref flags_result), ref lhs, imm, .. }
                    | Instruction::Sari { flags_result: Some(ref flags_result), ref lhs, imm, .. }
                    | Instruction::Rotli { flags_result: Some(ref flags_result), ref lhs, imm, .. }
                    | Instruction::Rotri { flags_result: Some(ref flags_result), ref lhs, imm, .. } => {
                        if let Value::VReg { ref name, .. } = flags_result {
                            self.flags_to_arith.insert(
                                name.clone(),
                                (inst.opcode().to_string(), lhs.clone(), Value::ConstInt { value: *imm, ty: IrType::I64 }),
                            );
                        }
                    }
                    _ => {}
                }

                if let Some((result_name, flags)) = get_test_result_and_flags(inst) {
                    if let Value::VReg { name: ref flags_name, .. } = flags {
                        self.test_to_flags.insert(
                            result_name.clone(),
                            (inst.opcode().to_string(), flags_name.clone()),
                        );
                    }
                }
            }
        }

        for bb in &func.basic_blocks {
            for inst in &bb.instructions {
                if let Instruction::BrCond { ref cond, .. } = inst {
                    if let Value::VReg { name: ref cond_name, .. } = cond {
                        if let Some((test_type, flags_name)) = self.test_to_flags.get(cond_name) {
                            if self.flags_to_arith.contains_key(flags_name) {
                                if Self::get_test_to_branch(test_type).is_some() {
                                    self.br_cond_optimized.insert(cond_name.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Pass 4: Eliminate phi nodes.
    fn eliminate_phi_nodes(&mut self, func: &Function) {
        self.phi_inserts.clear();
        self.phi_loop_inserts.clear();

        for bb in &func.basic_blocks {
            let phi_insts: Vec<&Instruction> = bb
                .instructions
                .iter()
                .filter(|inst| matches!(inst, Instruction::Phi { .. }))
                .collect();

            if phi_insts.is_empty() {
                continue;
            }

            for phi in &phi_insts {
                if let Instruction::Phi { ref result, ref incoming } = phi {
                    if let Value::VReg { name: ref result_name, .. } = result {
                        self.reg_allocator.ensure_allocated(result_name);
                        for (val, pred_name) in incoming {
                            let src_name = self.reg_allocator.get_reg_name(val);
                            if pred_name == &bb.name {
                                self.phi_loop_inserts
                                    .entry(bb.name.clone())
                                    .or_default()
                                    .push(format!("mov {}, {}", result_name, src_name));
                            } else {
                                self.phi_inserts
                                    .entry(pred_name.clone())
                                    .or_default()
                                    .push(format!("mov {}, {}", result_name, src_name));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Emit a basic block.
    fn emit_block(&mut self, bb: &BasicBlock, func: &Function) {
        self.current_block_name = bb.name.clone();
        if !bb.is_entry {
            self.asm_lines.push(format!(".L_{}_{}:", func.name, bb.name));
        }

        let (non_terms, terms): (Vec<&Instruction>, Vec<&Instruction>) = bb
            .instructions
            .iter()
            .partition(|inst| !is_terminator(inst));

        for inst in &non_terms {
            if matches!(inst, Instruction::Phi { .. }) {
                continue;
            }
            self.emit_instruction(inst);
            self.inst_position += 1;
        }

        // Emit phi-inserted mov instructions
        if let Some(insert_lines) = self.phi_inserts.get(&bb.name) {
            let lines = insert_lines.clone();
            for line in &lines {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 && parts[0] == "mov" {
                    let dest_name = parts[1].trim_end_matches(',');
                    let src_name = parts[2];
                    let r_dest = self.reg_allocator.ensure_allocated(dest_name);
                    let r_src = self.reg_allocator.ensure_allocated(src_name);
                    if r_dest != r_src {
                        self.asm_lines.push(format!("    mov {}, {}", r_dest, r_src));
                    }
                }
            }
        }

        for inst in &terms {
            self.emit_instruction(inst);
            self.inst_position += 1;
        }
    }

    /// Dispatch instruction.
    fn emit_instruction(&mut self, inst: &Instruction) {
        match inst {
            // Stack (always CISC)
            Instruction::Push { value } => {
                let r_val = self.reg_allocator.get_reg_name(value);
                self.asm_lines.push(format!("    ; [CISC] push {}", r_val));
                self.asm_lines.push(format!("    push {}", r_val));
            }
            Instruction::Pop { result } => {
                let r_dest = self.reg_allocator.get_reg_name(result);
                self.asm_lines.push(format!("    ; [CISC] pop {}", r_dest));
                self.asm_lines.push(format!("    pop {}", r_dest));
            }
            Instruction::Enter { frame_size } => {
                self.asm_lines.push(format!("    ; [CISC] enter"));
                self.asm_lines.push(format!("    enter {}", frame_size));
            }
            Instruction::Leave => {
                self.asm_lines.push("    ; [CISC] leave".to_string());
                self.asm_lines.push("    leave".to_string());
            }

            // Arithmetic
            Instruction::Add { .. }
            | Instruction::Sub { .. }
            | Instruction::Mul { .. }
            | Instruction::Mulh { .. }
            | Instruction::Div { .. }
            | Instruction::Divu { .. }
            | Instruction::Rem { .. }
            | Instruction::Remu { .. }
            | Instruction::And { .. }
            | Instruction::Or { .. }
            | Instruction::Xor { .. }
            | Instruction::Shl { .. }
            | Instruction::Shr { .. }
            | Instruction::Sar { .. }
            | Instruction::Rotl { .. }
            | Instruction::Rotr { .. } => self.emit_arith(inst),
            Instruction::Neg { result, operand, .. } => self.emit_neg(result, operand),
            Instruction::Not { result, operand, .. } => self.emit_not(result, operand),

            // Immediate
            Instruction::Addi { .. }
            | Instruction::Subi { .. }
            | Instruction::Muli { .. }
            | Instruction::Andi { .. }
            | Instruction::Ori { .. }
            | Instruction::Xori { .. }
            | Instruction::Shli { .. }
            | Instruction::Shri { .. }
            | Instruction::Sari { .. }
            | Instruction::Rotli { .. }
            | Instruction::Rotri { .. } => self.emit_immediate(inst),
            Instruction::Movi { result, imm } => self.emit_movi(result, *imm),
            Instruction::Mov { result, src } => self.emit_mov(result, src),

            // Flag consumers (CISC pattern matching)
            Instruction::TestEq { .. }
            | Instruction::TestNe { .. }
            | Instruction::TestLt { .. }
            | Instruction::TestLe { .. }
            | Instruction::TestLtu { .. }
            | Instruction::TestLeu { .. }
            | Instruction::TestOf { .. }
            | Instruction::TestCf { .. }
            | Instruction::TestSf { .. }
            | Instruction::TestGe { .. }
            | Instruction::TestGt { .. }
            | Instruction::TestGeu { .. }
            | Instruction::TestGtu { .. } => self.emit_test(inst),

            // Memory
            Instruction::Load { result, addr } => self.emit_load(result, addr),
            Instruction::Loadi { result, base, offset } => self.emit_loadi(result, base, *offset),
            Instruction::LoadSext { result, addr, from_type } => self.emit_load_sext(result, addr, from_type),
            Instruction::LoadZext { result, addr, from_type } => self.emit_load_zext(result, addr, from_type),
            Instruction::Store { value, addr } => self.emit_store(value, addr),
            Instruction::Storei { value, base, offset } => self.emit_storei(value, base, *offset),
            Instruction::Lea { result, addr } => self.emit_lea(result, addr),

            // Composite memory (hybrid decision)
            Instruction::MemAdd { addr, value } => self.emit_mem_composite(inst, addr, value),
            Instruction::MemSub { addr, value } => self.emit_mem_composite(inst, addr, value),
            Instruction::MemAnd { addr, value } => self.emit_mem_composite(inst, addr, value),
            Instruction::MemOr { addr, value } => self.emit_mem_composite(inst, addr, value),
            Instruction::MemXor { addr, value } => self.emit_mem_composite(inst, addr, value),
            Instruction::MemXchg { result, addr, value } => self.emit_mem_xchg_hybrid(result, addr, value),

            // Atomic
            Instruction::AtomicMemAdd { addr, value } => self.emit_atomic_add(addr, value),
            Instruction::AtomicMemXchg { result, addr, value } => self.emit_atomic_xchg(result, addr, value),
            Instruction::AtomicCas { result, addr, expected, desired } => self.emit_atomic_cas(result, addr, expected, desired),

            // Control flow
            Instruction::Br { target_bb } => self.emit_br(target_bb),
            Instruction::BrCond { cond, true_bb, false_bb } => self.emit_br_cond(cond, true_bb, false_bb),
            Instruction::Switch { value, default_bb, cases } => self.emit_switch(value, default_bb, cases),
            Instruction::Call { result, callee_name, args } => self.emit_call(result, callee_name, args),
            Instruction::CallIndirect { result, fnptr, args } => self.emit_call_indirect(result, fnptr, args),
            Instruction::Ret { value } => self.emit_ret(value),
            Instruction::TailCall { callee_name, args } => self.emit_tail_call(callee_name, args),

            // Float
            Instruction::Fadd { .. }
            | Instruction::Fsub { .. }
            | Instruction::Fmul { .. }
            | Instruction::Fdiv { .. }
            | Instruction::Fmin { .. }
            | Instruction::Fmax { .. } => self.emit_float_binary(inst),
            Instruction::Fneg { .. }
            | Instruction::Fabs { .. }
            | Instruction::Fsqrt { .. } => self.emit_float_unary(inst),
            Instruction::Ffma { result, a, b, c } => self.emit_ffma(result, a, b, c),
            Instruction::FcmpEq { .. }
            | Instruction::FcmpNe { .. }
            | Instruction::FcmpLt { .. }
            | Instruction::FcmpLe { .. }
            | Instruction::FcmpGt { .. }
            | Instruction::FcmpGe { .. }
            | Instruction::FcmpOrd { .. }
            | Instruction::FcmpUno { .. } => self.emit_fcmp(inst),

            // Conversion
            Instruction::Sext { .. }
            | Instruction::Zext { .. }
            | Instruction::Trunc { .. }
            | Instruction::Sitofp { .. }
            | Instruction::Uitofp { .. }
            | Instruction::Fptosi { .. }
            | Instruction::Fptoui { .. }
            | Instruction::Fpext { .. }
            | Instruction::Fptrunc { .. }
            | Instruction::Bitcast { .. } => self.emit_conv(inst),

            // System
            Instruction::Syscall => self.asm_lines.push("    syscall 0".to_string()),
            Instruction::Int { vector } => self.asm_lines.push(format!("    int {}", vector)),
            Instruction::Fence => self.asm_lines.push("    fence".to_string()),
            Instruction::Bkpt => self.asm_lines.push("    bkpt 0".to_string()),
            Instruction::Hlt => self.asm_lines.push("    hlt".to_string()),
            Instruction::Cli => self.asm_lines.push("    cli".to_string()),
            Instruction::Sti => self.asm_lines.push("    sti".to_string()),
            Instruction::Cpuid { result } => {
                self.asm_lines.push("    cpuid".to_string());
                let r_dest = self.reg_allocator.get_reg_name(result);
                if r_dest != "r0" {
                    self.asm_lines.push(format!("    mov {}, r0", r_dest));
                }
            }

            // Auxiliary
            Instruction::Select { result, cond, true_val, false_val } => {
                self.emit_select(result, cond, true_val, false_val);
            }
            Instruction::Nop => self.asm_lines.push("    nop".to_string()),
            Instruction::Phi { .. } => {} // Already eliminated

            // Vector
            Instruction::Vadd { .. }
            | Instruction::Vsub { .. }
            | Instruction::Vmul { .. }
            | Instruction::Vdiv { .. } => self.emit_vector_binary(inst),
            _ => {
                self.asm_lines.push(format!("    ; unhandled: {}", inst.opcode()));
            }
        }
    }

    // =====================================================================
    //  Arithmetic
    // =====================================================================

    fn emit_arith(&mut self, inst: &Instruction) {
        let opcode = inst.opcode();
        let (result, lhs, rhs) = get_arith_operands(inst);
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_lhs = self.reg_allocator.get_reg_name(lhs);
        let r_rhs = self.reg_allocator.get_reg_name(rhs);

        let asm_op = match opcode {
            "add" => "add", "sub" => "sub", "mul" => "mul", "mulh" => "mulh",
            "div" => "div", "divu" => "divu", "rem" => "rem", "remu" => "remu",
            "and" => "and", "or" => "or", "xor" => "xor",
            "shl" => "shl", "shr" => "shr", "sar" => "sar",
            "rotl" => "rol", "rotr" => "ror",
            _ => opcode,
        };

        if opcode == "mulh" {
            self.asm_lines.push(format!("    mulh {}, {}, {}", r_dest, r_lhs, r_rhs));
            return;
        }

        if opcode == "rem" {
            self.asm_lines.push(format!("    ; rem pseudo: {} = {} % {}", r_dest, r_lhs, r_rhs));
            let r_temp = self.get_temp_reg(&[&r_dest, &r_lhs, &r_rhs]);
            self.asm_lines.push(format!("    div {}, {}, {}", r_temp, r_lhs, r_rhs));
            self.asm_lines.push(format!("    mul {}, {}, {}", r_temp, r_temp, r_rhs));
            self.asm_lines.push(format!("    sub {}, {}, {}", r_dest, r_lhs, r_temp));
            return;
        }

        if opcode == "remu" {
            self.asm_lines.push(format!("    ; remu pseudo: {} = {} % {}", r_dest, r_lhs, r_rhs));
            let r_temp = self.get_temp_reg(&[&r_dest, &r_lhs, &r_rhs]);
            self.asm_lines.push(format!("    divu {}, {}, {}", r_temp, r_lhs, r_rhs));
            self.asm_lines.push(format!("    mul {}, {}, {}", r_temp, r_temp, r_rhs));
            self.asm_lines.push(format!("    sub {}, {}, {}", r_dest, r_lhs, r_temp));
            return;
        }

        self.asm_lines.push(format!("    {} {}, {}, {}", asm_op, r_dest, r_lhs, r_rhs));
    }

    fn emit_neg(&mut self, result: &Value, operand: &Value) {
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_src = self.reg_allocator.get_reg_name(operand);
        self.asm_lines.push(format!("    sub {}, r0, {}", r_dest, r_src));
    }

    fn emit_not(&mut self, result: &Value, operand: &Value) {
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_src = self.reg_allocator.get_reg_name(operand);
        self.asm_lines.push(format!("    xori {}, {}, -1", r_dest, r_src));
    }

    // =====================================================================
    //  Immediate
    // =====================================================================

    fn emit_immediate(&mut self, inst: &Instruction) {
        let (result, lhs, imm) = get_imm_operands(inst);
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_lhs = self.reg_allocator.get_reg_name(lhs);
        self.asm_lines.push(format!("    {} {}, {}, {}", inst.opcode(), r_dest, r_lhs, imm));
    }

    fn emit_movi(&mut self, result: &Value, imm: i64) {
        let r_dest = self.reg_allocator.get_reg_name(result);
        if (-8192..=8191).contains(&imm) {
            self.asm_lines.push(format!("    mov {}, {}", r_dest, imm));
        } else {
            self.asm_lines.push(format!("    movi {}, {}", r_dest, imm));
        }
    }

    fn emit_mov(&mut self, result: &Value, src: &Value) {
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_src = self.reg_allocator.get_reg_name(src);
        if r_dest != r_src {
            self.asm_lines.push(format!("    mov {}, {}", r_dest, r_src));
        }
    }

    // =====================================================================
    //  Flag consumers (CISC pattern matching)
    // =====================================================================

    fn emit_test(&mut self, inst: &Instruction) {
        let opcode = inst.opcode();
        let (result_name, flags) = match get_test_result_and_flags(inst) {
            Some((name, flags)) => (name, flags),
            None => return,
        };

        if self.br_cond_optimized.contains(&result_name) {
            self.asm_lines.push(format!("    ; [CISC] {} optimised away (flag pattern match)", opcode));
            return;
        }

        if let Value::VReg { name: ref flags_name, .. } = flags {
            if let Some((_, ref lhs, ref rhs)) = self.flags_to_arith.get(flags_name).cloned() {
                let r_result = self.reg_allocator.get_reg_name(&Value::VReg { name: result_name.clone(), ty: IrType::I1 });
                let r_lhs = self.reg_allocator.get_reg_name(&lhs);
                let r_rhs = self.reg_allocator.get_reg_name(&rhs);

                let cmp_op = match opcode {
                    "test_eq" => "eq", "test_ne" => "eq",
                    "test_lt" => "lt", "test_le" => "le",
                    "test_ltu" => "ltu", "test_leu" => "leu",
                    "test_ge" => "lt", "test_gt" => "le",
                    "test_geu" => "ltu", "test_gtu" => "leu",
                    _ => "eq",
                };

                let needs_invert = matches!(opcode, "test_ne" | "test_ge" | "test_gt" | "test_geu" | "test_gtu");

                if needs_invert {
                    let r_temp = self.get_temp_reg(&[&r_result, &r_lhs, &r_rhs]);
                    self.asm_lines.push(format!("    ; [CISC] {} → {} + xori", opcode, cmp_op));
                    self.asm_lines.push(format!("    {} {}, {}, {}", cmp_op, r_temp, r_lhs, r_rhs));
                    self.asm_lines.push(format!("    xori {}, {}, 1", r_result, r_temp));
                } else {
                    self.asm_lines.push(format!("    ; [CISC] {} → {}", opcode, cmp_op));
                    self.asm_lines.push(format!("    {} {}, {}, {}", cmp_op, r_result, r_lhs, r_rhs));
                }
                return;
            }
        }
        self.asm_lines.push(format!("    ; {} - no producer, fallback", opcode));
    }

    // =====================================================================
    //  Memory
    // =====================================================================

    fn emit_load(&mut self, result: &Value, addr: &AddrExpr) {
        let r_dest = self.reg_allocator.get_reg_name(result);
        self.emit_mem_op("ld", &r_dest, addr, false, "");
    }

    fn emit_loadi(&mut self, result: &Value, base: &Value, offset: i64) {
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_base = self.resolve_addr_base_simple(base);
        if offset >= 0 {
            self.asm_lines.push(format!("    ld {}, [{} + {}]", r_dest, r_base, offset));
        } else {
            self.asm_lines.push(format!("    ld {}, [{} - {}]", r_dest, r_base, -offset));
        }
    }

    fn emit_load_sext(&mut self, result: &Value, addr: &AddrExpr, from_type: &IrType) {
        let r_dest = self.reg_allocator.get_reg_name(result);
        let sz = type_to_size_suffix(from_type);
        self.emit_mem_op("lds", &r_dest, addr, false, sz);
    }

    fn emit_load_zext(&mut self, result: &Value, addr: &AddrExpr, from_type: &IrType) {
        let r_dest = self.reg_allocator.get_reg_name(result);
        let sz = type_to_size_suffix(from_type);
        self.emit_mem_op("ldu", &r_dest, addr, false, sz);
    }

    fn emit_store(&mut self, value: &Value, addr: &AddrExpr) {
        let r_val = self.reg_allocator.get_reg_name(value);
        self.emit_mem_op("st", &r_val, addr, true, "");
    }

    fn emit_storei(&mut self, value: &Value, base: &Value, offset: i64) {
        let r_val = self.reg_allocator.get_reg_name(value);
        let r_base = self.resolve_addr_base_simple(base);
        if offset >= 0 {
            self.asm_lines.push(format!("    st {}, [{} + {}]", r_val, r_base, offset));
        } else {
            self.asm_lines.push(format!("    st {}, [{} - {}]", r_val, r_base, -offset));
        }
    }

    fn emit_lea(&mut self, result: &Value, addr: &AddrExpr) {
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_base = self.resolve_addr_base(addr);
        if let Some(ref index) = addr.index {
            let r_index = self.reg_allocator.get_reg_name(index);
            self.asm_lines.push(format!("    lda {}, {}, {}, {}", r_dest, r_base, r_index, addr.scale));
            if addr.offset != 0 {
                if addr.offset > 0 {
                    self.asm_lines.push(format!("    addi {}, {}, {}", r_dest, r_dest, addr.offset));
                } else {
                    self.asm_lines.push(format!("    subi {}, {}, {}", r_dest, r_dest, -addr.offset));
                }
            }
        } else {
            if addr.offset >= 0 {
                self.asm_lines.push(format!("    addi {}, {}, {}", r_dest, r_base, addr.offset));
            } else {
                self.asm_lines.push(format!("    subi {}, {}, {}", r_dest, r_base, -addr.offset));
            }
        }
    }

    // =====================================================================
    //  Composite memory (hybrid decision)
    // =====================================================================

    fn emit_mem_composite(&mut self, inst: &Instruction, addr: &AddrExpr, value: &Value) {
        let r_val = self.reg_allocator.get_reg_name(value);
        let live_count = self.pressure_tracker.get_live_count(self.inst_position);
        let opcode = inst.opcode();

        match opcode {
            "mem_add" => {
                if self.pressure_tracker.is_pressure_low(self.inst_position) {
                    self.asm_lines.push(format!("    ; [CISC] mem_add (live={}, < 10)", live_count));
                    self.emit_cisc_mem_op("addm", &r_val, addr, None);
                } else {
                    self.asm_lines.push(format!("    ; [RISC] mem_add expanded (live={}, >= 10)", live_count));
                    self.emit_risc_mem_alu("add", &r_val, addr);
                }
            }
            "mem_sub" => {
                if self.pressure_tracker.is_pressure_low(self.inst_position) {
                    self.asm_lines.push(format!("    ; [CISC] mem_sub (live={}, < 10)", live_count));
                    self.emit_cisc_mem_op("subm", &r_val, addr, None);
                } else {
                    self.asm_lines.push(format!("    ; [RISC] mem_sub expanded (live={}, >= 10)", live_count));
                    self.emit_risc_mem_alu("sub", &r_val, addr);
                }
            }
            "mem_and" => {
                self.asm_lines.push(format!("    ; [RISC] mem_and always expanded (no CISC instruction)"));
                self.emit_risc_mem_alu("and", &r_val, addr);
            }
            "mem_or" => {
                self.asm_lines.push(format!("    ; [RISC] mem_or always expanded (no CISC instruction)"));
                self.emit_risc_mem_alu("or", &r_val, addr);
            }
            "mem_xor" => {
                self.asm_lines.push(format!("    ; [RISC] mem_xor always expanded (no CISC instruction)"));
                self.emit_risc_mem_alu("xor", &r_val, addr);
            }
            _ => {}
        }
    }

    fn emit_mem_xchg_hybrid(&mut self, result: &Value, addr: &AddrExpr, value: &Value) {
        let r_val = self.reg_allocator.get_reg_name(value);
        let r_result = self.reg_allocator.get_reg_name(result);
        let live_count = self.pressure_tracker.get_live_count(self.inst_position);

        if self.pressure_tracker.is_pressure_low(self.inst_position) {
            self.asm_lines.push(format!("    ; [CISC] mem_xchg (live={}, < 10)", live_count));
            self.emit_cisc_mem_op("xchg", &r_val, addr, Some(&r_result));
        } else {
            self.asm_lines.push(format!("    ; [RISC] mem_xchg expanded (live={}, >= 10)", live_count));
            self.emit_risc_mem_xchg(&r_result, &r_val, addr);
        }
    }

    fn emit_cisc_mem_op(&mut self, op: &str, reg: &str, addr: &AddrExpr, result: Option<&str>) {
        let r_base = self.resolve_addr_base(addr);
        let off_str = self.format_offset(addr.offset);

        if addr.index.is_some() {
            let r_temp = self.get_temp_reg(&[reg, &r_base]);
            self.asm_lines.push(format!("    ; {} with indexed addressing, expanding", op));
            self.emit_mem_op("ld", &r_temp, addr, false, "");
            if op == "addm" {
                self.asm_lines.push(format!("    add {}, {}, {}", r_temp, r_temp, reg));
            } else if op == "subm" {
                self.asm_lines.push(format!("    sub {}, {}, {}", r_temp, r_temp, reg));
            } else if op == "xchg" {
                self.asm_lines.push(format!("    mov r9, {}", r_temp));
                self.asm_lines.push(format!("    mov {}, {}", r_temp, reg));
                self.asm_lines.push(format!("    mov {}, r9", reg));
            }
            self.emit_mem_op("st", &r_temp, addr, true, "");
        } else {
            self.asm_lines.push(format!("    {} {}, [{}{}]", op, reg, r_base, off_str));
            if let Some(r_result) = result {
                if r_result != reg {
                    self.asm_lines.push(format!("    mov {}, {}", r_result, reg));
                }
            }
        }
    }

    fn emit_risc_mem_alu(&mut self, mnemonic: &str, r_val: &str, addr: &AddrExpr) {
        let r_temp = self.get_temp_reg(&[r_val]);
        let r_base = self.resolve_addr_base(addr);
        let off_str = self.format_offset(addr.offset);

        if let Some(ref index) = addr.index {
            let r_index = self.reg_allocator.get_reg_name(index);
            self.asm_lines.push(format!("    ldr {}, [{} + {}*{}{}]", r_temp, r_base, r_index, addr.scale, off_str));
            self.asm_lines.push(format!("    {} {}, {}, {}", mnemonic, r_temp, r_temp, r_val));
            self.asm_lines.push(format!("    str {}, [{} + {}*{}{}]", r_temp, r_base, r_index, addr.scale, off_str));
        } else {
            self.asm_lines.push(format!("    ld {}, [{}{}]", r_temp, r_base, off_str));
            self.asm_lines.push(format!("    {} {}, {}, {}", mnemonic, r_temp, r_temp, r_val));
            self.asm_lines.push(format!("    st {}, [{}{}]", r_temp, r_base, off_str));
        }
    }

    fn emit_risc_mem_xchg(&mut self, r_result: &str, r_val: &str, addr: &AddrExpr) {
        let r_temp = self.get_temp_reg(&[r_val, r_result]);
        let r_base = self.resolve_addr_base(addr);
        let off_str = self.format_offset(addr.offset);

        if let Some(ref index) = addr.index {
            let r_index = self.reg_allocator.get_reg_name(index);
            self.asm_lines.push(format!("    ldr {}, [{} + {}*{}{}]", r_temp, r_base, r_index, addr.scale, off_str));
            self.asm_lines.push(format!("    str {}, [{} + {}*{}{}]", r_val, r_base, r_index, addr.scale, off_str));
        } else {
            self.asm_lines.push(format!("    ld {}, [{}{}]", r_temp, r_base, off_str));
            self.asm_lines.push(format!("    st {}, [{}{}]", r_val, r_base, off_str));
        }
        self.asm_lines.push(format!("    mov {}, {}", r_result, r_temp));
    }

    // =====================================================================
    //  Atomic
    // =====================================================================

    fn emit_atomic_add(&mut self, addr: &AddrExpr, value: &Value) {
        let r_val = self.reg_allocator.get_reg_name(value);
        let r_base = self.resolve_addr_base(addr);
        let off_str = self.format_offset(addr.offset);
        let r_old = self.get_temp_reg(&[&r_val, &r_base]);
        let r_new = self.get_temp_reg(&[&r_val, &r_base, &r_old]);

        let loop_label = self.new_label("atomic_add");
        self.asm_lines.push(format!(".L_{}:", loop_label));
        self.asm_lines.push(format!("    ld {}, [{}{}]", r_old, r_base, off_str));
        self.asm_lines.push(format!("    add {}, {}, {}", r_new, r_old, r_val));
        self.asm_lines.push(format!("    cmpxchg {}, {}, [{}{}]", r_old, r_new, r_base, off_str));
        self.asm_lines.push(format!("    bne {}, {}, .L_{}", r_old, r_old, loop_label));
    }

    fn emit_atomic_xchg(&mut self, result: &Value, addr: &AddrExpr, value: &Value) {
        let r_result = self.reg_allocator.get_reg_name(result);
        let r_val = self.reg_allocator.get_reg_name(value);
        let r_base = self.resolve_addr_base(addr);
        let off_str = self.format_offset(addr.offset);
        self.asm_lines.push(format!("    xchg {}, [{}{}]", r_val, r_base, off_str));
        if r_result != r_val {
            self.asm_lines.push(format!("    mov {}, {}", r_result, r_val));
        }
    }

    fn emit_atomic_cas(&mut self, result: &Value, addr: &AddrExpr, expected: &Value, desired: &Value) {
        let r_expected = self.reg_allocator.get_reg_name(expected);
        let r_desired = self.reg_allocator.get_reg_name(desired);
        let r_result = self.reg_allocator.get_reg_name(result);
        let r_base = self.resolve_addr_base(addr);
        let off_str = self.format_offset(addr.offset);
        let r_temp = self.get_temp_reg(&[&r_expected, &r_desired, &r_result, &r_base]);
        self.asm_lines.push(format!("    mov {}, {}", r_temp, r_expected));
        self.asm_lines.push(format!("    cmpxchg {}, {}, [{}{}]", r_temp, r_desired, r_base, off_str));
        if r_result != r_temp {
            self.asm_lines.push(format!("    mov {}, {}", r_result, r_temp));
        }
    }

    // =====================================================================
    //  Control flow
    // =====================================================================

    fn emit_br(&mut self, target_bb: &str) {
        self.asm_lines.push(format!("    j .L_{}_{}", self.current_func_name, target_bb));
    }

    fn emit_br_cond(&mut self, cond: &Value, true_bb: &str, false_bb: &str) {
        let true_label = format!(".L_{}_{}", self.current_func_name, true_bb);
        let false_label = format!(".L_{}_{}", self.current_func_name, false_bb);

        if let Value::VReg { name: ref cond_name, .. } = cond {
            if self.br_cond_optimized.contains(cond_name) {
                let (test_type, flags_name) = self.test_to_flags[cond_name].clone();
                let (_, lhs, rhs) = self.flags_to_arith[&flags_name].clone();
                let branch_op = Self::get_test_to_branch(&test_type).unwrap_or("beq");
                let r_lhs = self.reg_allocator.get_reg_name(&lhs);

                let r_rhs = if let Value::ConstInt { value, .. } = &rhs {
                    if *value == 0 {
                        "r0".to_string()
                    } else {
                        let r = self.get_temp_reg(&[&r_lhs]);
                        self.asm_lines.push(format!("    movi {}, {}", r, value));
                        r
                    }
                } else {
                    self.reg_allocator.get_reg_name(&rhs)
                };

                self.asm_lines.push(format!("    ; [CISC] flag-pattern br_cond ({} → {})", test_type, branch_op));
                self.asm_lines.push(format!("    {} {}, {}, {}", branch_op, r_lhs, r_rhs, true_label));
                self.emit_loop_phi_moves();
                self.asm_lines.push(format!("    j {}", false_label));
                return;
            }
        }

        let r_cond = self.reg_allocator.get_reg_name(cond);
        self.asm_lines.push(format!("    beq {}, r0, {}", r_cond, false_label));
        self.emit_loop_phi_moves();
        self.asm_lines.push(format!("    j {}", true_label));
    }

    fn emit_switch(&mut self, value: &Value, default_bb: &str, cases: &[(Value, String)]) {
        let r_val = self.reg_allocator.get_reg_name(value);
        let default_label = format!(".L_{}_{}", self.current_func_name, default_bb);
        for (cv, case_bb) in cases {
            let case_label = format!(".L_{}_{}", self.current_func_name, case_bb);
            if let Value::ConstInt { value: cv_val, .. } = cv {
                if *cv_val == 0 {
                    self.asm_lines.push(format!("    beq {}, r0, {}", r_val, case_label));
                } else {
                    let r_cmp = self.get_temp_reg(&[&r_val]);
                    self.asm_lines.push(format!("    movi {}, {}", r_cmp, cv_val));
                    self.asm_lines.push(format!("    beq {}, {}, {}", r_val, r_cmp, case_label));
                }
            }
        }
        self.asm_lines.push(format!("    j {}", default_label));
    }

    fn emit_call(&mut self, result: &Option<Value>, callee_name: &str, args: &[Value]) {
        for (i, arg) in args.iter().enumerate().take(7) {
            let preg = regalloc::param_reg(i);
            let r_arg = self.reg_allocator.get_reg_name(arg);
            if r_arg != format!("r{}", preg) {
                self.asm_lines.push(format!("    mov r{}, {}", preg, r_arg));
            }
        }
        self.asm_lines.push(format!("    call .L_{}", callee_name));
        if let Some(ref result) = result {
            let r_result = self.reg_allocator.get_reg_name(result);
            if r_result != "r1" {
                self.asm_lines.push(format!("    mov {}, r1", r_result));
            }
        }
    }

    fn emit_call_indirect(&mut self, result: &Option<Value>, fnptr: &Value, args: &[Value]) {
        for (i, arg) in args.iter().enumerate().take(7) {
            let preg = regalloc::param_reg(i);
            let r_arg = self.reg_allocator.get_reg_name(arg);
            if r_arg != format!("r{}", preg) {
                self.asm_lines.push(format!("    mov r{}, {}", preg, r_arg));
            }
        }
        let r_fnptr = self.reg_allocator.get_reg_name(fnptr);
        self.asm_lines.push(format!("    callreg {}", r_fnptr));
        if let Some(ref result) = result {
            let r_result = self.reg_allocator.get_reg_name(result);
            if r_result != "r1" {
                self.asm_lines.push(format!("    mov {}, r1", r_result));
            }
        }
    }

    fn emit_ret(&mut self, value: &Option<Value>) {
        if let Some(ref v) = value {
            let r_val = self.reg_allocator.get_reg_name(v);
            if r_val != "r1" {
                self.asm_lines.push(format!("    mov r1, {}", r_val));
            }
        }
        self.asm_lines.push("    ret".to_string());
    }

    fn emit_tail_call(&mut self, callee_name: &str, args: &[Value]) {
        for (i, arg) in args.iter().enumerate().take(7) {
            let preg = regalloc::param_reg(i);
            let r_arg = self.reg_allocator.get_reg_name(arg);
            if r_arg != format!("r{}", preg) {
                self.asm_lines.push(format!("    mov r{}, {}", preg, r_arg));
            }
        }
        self.asm_lines.push(format!("    j .L_{}", callee_name));
    }

    // =====================================================================
    //  Float, Conversion, Vector, System, Auxiliary
    // =====================================================================

    fn emit_float_binary(&mut self, inst: &Instruction) {
        let (result, lhs, rhs) = get_float_binary_operands(inst);
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_lhs = self.reg_allocator.get_reg_name(lhs);
        let r_rhs = self.reg_allocator.get_reg_name(rhs);
        self.asm_lines.push(format!("    {} {}, {}, {}", inst.opcode(), r_dest, r_lhs, r_rhs));
    }

    fn emit_float_unary(&mut self, inst: &Instruction) {
        let (result, operand) = get_float_unary_operands(inst);
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_src = self.reg_allocator.get_reg_name(operand);
        self.asm_lines.push(format!("    {} {}, {}", inst.opcode(), r_dest, r_src));
    }

    fn emit_ffma(&mut self, result: &Value, a: &Value, b: &Value, c: &Value) {
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_a = self.reg_allocator.get_reg_name(a);
        let r_b = self.reg_allocator.get_reg_name(b);
        let r_c = self.reg_allocator.get_reg_name(c);
        self.asm_lines.push(format!("    fmul {}, {}, {}", r_dest, r_a, r_b));
        self.asm_lines.push(format!("    fadd {}, {}, {}", r_dest, r_dest, r_c));
    }

    fn emit_fcmp(&mut self, inst: &Instruction) {
        let (result, lhs, rhs) = get_fcmp_operands(inst);
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_lhs = self.reg_allocator.get_reg_name(lhs);
        let r_rhs = self.reg_allocator.get_reg_name(rhs);
        let cond = fcmp_cond(inst.opcode());

        self.asm_lines.push(format!("    fcmp {}, {}", r_lhs, r_rhs));
        self.asm_lines.push(format!("    ; fcmp {} → {}", cond, r_dest));
        self.asm_lines.push(format!("    mov {}, 0", r_dest));

        let skip_label = self.new_label("fcmp");
        match cond {
            "eq" => self.asm_lines.push(format!("    bne {}, {}, .L_{}", r_lhs, r_rhs, skip_label)),
            "ne" => self.asm_lines.push(format!("    beq {}, {}, .L_{}", r_lhs, r_rhs, skip_label)),
            "lt" => self.asm_lines.push(format!("    bge {}, {}, .L_{}", r_lhs, r_rhs, skip_label)),
            "le" => self.asm_lines.push(format!("    bgt {}, {}, .L_{}", r_lhs, r_rhs, skip_label)),
            "gt" => self.asm_lines.push(format!("    ble {}, {}, .L_{}", r_lhs, r_rhs, skip_label)),
            "ge" => self.asm_lines.push(format!("    blt {}, {}, .L_{}", r_lhs, r_rhs, skip_label)),
            _ => self.asm_lines.push(format!("    j .L_{}", skip_label)),
        }
        self.asm_lines.push(format!("    movi {}, 1", r_dest));
        self.asm_lines.push(format!(".L_{}:", skip_label));
    }

    fn emit_conv(&mut self, inst: &Instruction) {
        match inst {
            Instruction::Sext { result, value, .. }
            | Instruction::Zext { result, value, .. }
            | Instruction::Trunc { result, value, .. } => {
                let r_dest = self.reg_allocator.get_reg_name(result);
                let r_src = self.reg_allocator.get_reg_name(value);
                if let Instruction::Trunc { from_type, .. } = inst {
                    if let IrType::I8 = from_type {
                        self.asm_lines.push(format!("    andi {}, {}, 0xFF", r_dest, r_src));
                        return;
                    } else if let IrType::I16 = from_type {
                        self.asm_lines.push(format!("    andi {}, {}, 0xFFFF", r_dest, r_src));
                        return;
                    }
                }
                if r_dest != r_src {
                    self.asm_lines.push(format!("    mov {}, {}", r_dest, r_src));
                }
            }
            Instruction::Sitofp { result, value, .. }
            | Instruction::Uitofp { result, value, .. } => {
                let r_dest = self.reg_allocator.get_reg_name(result);
                let r_src = self.reg_allocator.get_reg_name(value);
                self.asm_lines.push(format!("    fcvt.s.w {}, {}", r_dest, r_src));
            }
            Instruction::Fptosi { result, value, .. }
            | Instruction::Fptoui { result, value, .. } => {
                let r_dest = self.reg_allocator.get_reg_name(result);
                let r_src = self.reg_allocator.get_reg_name(value);
                self.asm_lines.push(format!("    fcvt.w.s {}, {}", r_dest, r_src));
            }
            Instruction::Fpext { result, value, .. }
            | Instruction::Fptrunc { result, value, .. }
            | Instruction::Bitcast { result, value, .. } => {
                let r_dest = self.reg_allocator.get_reg_name(result);
                let r_src = self.reg_allocator.get_reg_name(value);
                if r_dest != r_src {
                    self.asm_lines.push(format!("    mov {}, {}", r_dest, r_src));
                }
            }
            _ => {}
        }
    }

    fn emit_select(&mut self, result: &Value, cond: &Value, true_val: &Value, false_val: &Value) {
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_cond = self.reg_allocator.get_reg_name(cond);
        let r_true = self.reg_allocator.get_reg_name(true_val);
        let r_false = self.reg_allocator.get_reg_name(false_val);
        if r_dest != r_false {
            self.asm_lines.push(format!("    mov {}, {}", r_dest, r_false));
        }
        let skip_label = self.new_label("select");
        self.asm_lines.push(format!("    beq {}, r0, .L_{}", r_cond, skip_label));
        if r_dest != r_true {
            self.asm_lines.push(format!("    mov {}, {}", r_dest, r_true));
        }
        self.asm_lines.push(format!(".L_{}:", skip_label));
    }

    fn emit_vector_binary(&mut self, inst: &Instruction) {
        let (result, lhs, rhs) = get_vector_binary_operands(inst);
        let r_dest = self.reg_allocator.get_reg_name(result);
        let r_lhs = self.reg_allocator.get_reg_name(lhs);
        let r_rhs = self.reg_allocator.get_reg_name(rhs);
        self.asm_lines.push(format!("    {} {}, {}, {}", inst.opcode(), r_dest, r_lhs, r_rhs));
    }

    // =====================================================================
    //  Helpers
    // =====================================================================

    fn new_label(&mut self, prefix: &str) -> String {
        let label = format!("{}_{}", prefix, self.label_counter);
        self.label_counter += 1;
        label
    }

    fn get_temp_reg(&self, avoid: &[&str]) -> String {
        let scratch_pool = ["r14", "r15", "r13", "r12", "r8", "r9"];
        for reg in &scratch_pool {
            if !avoid.contains(reg) {
                return reg.to_string();
            }
        }
        "r14".to_string()
    }

    fn format_offset(&self, offset: i64) -> String {
        if offset == 0 {
            " + 0".to_string()
        } else if offset > 0 {
            format!(" + {}", offset)
        } else {
            format!(" - {}", -offset)
        }
    }

    fn resolve_addr_base(&mut self, addr: &AddrExpr) -> String {
        if let Value::GlobalVar { ref name, .. } = addr.base {
            let r_temp = self.get_temp_reg(&[]);
            self.asm_lines.push(format!("    la {}, @{}", r_temp, name));
            return r_temp;
        }
        self.reg_allocator.get_reg_name(&addr.base)
    }

    fn resolve_addr_base_simple(&mut self, base: &Value) -> String {
        if let Value::GlobalVar { ref name, .. } = base {
            let r_temp = self.get_temp_reg(&[]);
            self.asm_lines.push(format!("    la {}, @{}", r_temp, name));
            return r_temp;
        }
        self.reg_allocator.get_reg_name(base)
    }

    fn emit_mem_op(&mut self, _op: &str, reg: &str, addr: &AddrExpr, is_store: bool, sz: &str) {
        let r_base = self.resolve_addr_base(addr);
        let off_str = self.format_offset(addr.offset);

        if let Some(ref index) = addr.index {
            let r_index = self.reg_allocator.get_reg_name(index);
            if is_store {
                self.asm_lines.push(format!("    str {}, [{} + {}*{}{}]", reg, r_base, r_index, addr.scale, off_str));
            } else {
                self.asm_lines.push(format!("    ldr {}, [{} + {}*{}{}]", reg, r_base, r_index, addr.scale, off_str));
            }
        } else {
            if is_store {
                match sz {
                    "b" => self.asm_lines.push(format!("    stb {}, [{}{}]", reg, r_base, off_str)),
                    "w" => self.asm_lines.push(format!("    stw {}, [{}{}]", reg, r_base, off_str)),
                    _ => self.asm_lines.push(format!("    st {}, [{}{}]", reg, r_base, off_str)),
                }
            } else {
                match sz {
                    "b" | "h" | "w" => self.asm_lines.push(format!("    ldu {}, [{}{}]", reg, r_base, off_str)),
                    _ => self.asm_lines.push(format!("    ld {}, [{}{}]", reg, r_base, off_str)),
                }
            }
        }
    }

    fn emit_loop_phi_moves(&mut self) {
        if let Some(insert_lines) = self.phi_loop_inserts.get(&self.current_block_name) {
            let lines = insert_lines.clone();
            for line in &lines {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 && parts[0] == "mov" {
                    let dest_name = parts[1].trim_end_matches(',');
                    let src_name = parts[2];
                    let r_dest = self.reg_allocator.ensure_allocated(dest_name);
                    let r_src = self.reg_allocator.ensure_allocated(src_name);
                    if r_dest != r_src {
                        self.asm_lines.push(format!("    mov {}, {}", r_dest, r_src));
                    }
                }
            }
        }
    }
}

// =========================================================================
//  Shared helper functions
// =========================================================================

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

fn get_arith_operands(inst: &Instruction) -> (&Value, &Value, &Value) {
    match inst {
        Instruction::Add { result, lhs, rhs, .. }
        | Instruction::Sub { result, lhs, rhs, .. }
        | Instruction::Mul { result, lhs, rhs, .. }
        | Instruction::Mulh { result, lhs, rhs, .. }
        | Instruction::Div { result, lhs, rhs, .. }
        | Instruction::Divu { result, lhs, rhs, .. }
        | Instruction::Rem { result, lhs, rhs, .. }
        | Instruction::Remu { result, lhs, rhs, .. }
        | Instruction::And { result, lhs, rhs, .. }
        | Instruction::Or { result, lhs, rhs, .. }
        | Instruction::Xor { result, lhs, rhs, .. }
        | Instruction::Shl { result, lhs, rhs, .. }
        | Instruction::Shr { result, lhs, rhs, .. }
        | Instruction::Sar { result, lhs, rhs, .. }
        | Instruction::Rotl { result, lhs, rhs, .. }
        | Instruction::Rotr { result, lhs, rhs, .. } => (result, lhs, rhs),
        _ => unreachable!(),
    }
}

fn get_imm_operands(inst: &Instruction) -> (&Value, &Value, i64) {
    match inst {
        Instruction::Addi { result, lhs, imm, .. }
        | Instruction::Subi { result, lhs, imm, .. }
        | Instruction::Muli { result, lhs, imm, .. }
        | Instruction::Andi { result, lhs, imm, .. }
        | Instruction::Ori { result, lhs, imm, .. }
        | Instruction::Xori { result, lhs, imm, .. }
        | Instruction::Shli { result, lhs, imm, .. }
        | Instruction::Shri { result, lhs, imm, .. }
        | Instruction::Sari { result, lhs, imm, .. }
        | Instruction::Rotli { result, lhs, imm, .. }
        | Instruction::Rotri { result, lhs, imm, .. } => (result, lhs, *imm),
        _ => unreachable!(),
    }
}

fn get_test_result_and_flags(inst: &Instruction) -> Option<(String, Value)> {
    match inst {
        Instruction::TestEq { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestNe { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestLt { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestLe { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestLtu { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestLeu { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestOf { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestCf { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestSf { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestGe { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestGt { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestGeu { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        Instruction::TestGtu { result, ref flags, .. } => vreg_name(result).map(|n| (n, flags.clone())),
        _ => None,
    }
}

fn vreg_name(val: &Value) -> Option<String> {
    if let Value::VReg { ref name, .. } = val {
        Some(name.clone())
    } else {
        None
    }
}

fn get_result_vreg_name(inst: &Instruction) -> Option<String> {
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
        | Instruction::Pop { result, .. } => vreg_name(result),
        Instruction::Call { result: Some(ref result), .. }
        | Instruction::CallIndirect { result: Some(ref result), .. } => vreg_name(result),
        Instruction::Fadd { result, .. }
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
        | Instruction::Select { result, .. }
        | Instruction::Phi { result, .. } => vreg_name(result),
        _ => None,
    }
}

fn get_flags_vreg_name(inst: &Instruction) -> Option<String> {
    match inst {
        Instruction::Add { ref flags_result, .. }
        | Instruction::Sub { ref flags_result, .. }
        | Instruction::Mul { ref flags_result, .. }
        | Instruction::Mulh { ref flags_result, .. }
        | Instruction::Div { ref flags_result, .. }
        | Instruction::Divu { ref flags_result, .. }
        | Instruction::Rem { ref flags_result, .. }
        | Instruction::Remu { ref flags_result, .. }
        | Instruction::And { ref flags_result, .. }
        | Instruction::Or { ref flags_result, .. }
        | Instruction::Xor { ref flags_result, .. }
        | Instruction::Shl { ref flags_result, .. }
        | Instruction::Shr { ref flags_result, .. }
        | Instruction::Sar { ref flags_result, .. }
        | Instruction::Rotl { ref flags_result, .. }
        | Instruction::Rotr { ref flags_result, .. }
        | Instruction::Neg { ref flags_result, .. }
        | Instruction::Not { ref flags_result, .. } => vreg_name(flags_result),
        Instruction::Addi { flags_result: Some(ref f), .. }
        | Instruction::Subi { flags_result: Some(ref f), .. }
        | Instruction::Muli { flags_result: Some(ref f), .. }
        | Instruction::Andi { flags_result: Some(ref f), .. }
        | Instruction::Ori { flags_result: Some(ref f), .. }
        | Instruction::Xori { flags_result: Some(ref f), .. }
        | Instruction::Shli { flags_result: Some(ref f), .. }
        | Instruction::Shri { flags_result: Some(ref f), .. }
        | Instruction::Sari { flags_result: Some(ref f), .. }
        | Instruction::Rotli { flags_result: Some(ref f), .. }
        | Instruction::Rotri { flags_result: Some(ref f), .. } => vreg_name(f),
        _ => None,
    }
}

fn get_inst_operands(inst: &Instruction) -> Vec<&Value> {
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
        | Instruction::Rotr { lhs, rhs, .. } => { ops.push(lhs); ops.push(rhs); }
        Instruction::Neg { operand, .. } | Instruction::Not { operand, .. } => { ops.push(operand); }
        Instruction::Addi { lhs, .. } | Instruction::Subi { lhs, .. } | Instruction::Muli { lhs, .. }
        | Instruction::Andi { lhs, .. } | Instruction::Ori { lhs, .. } | Instruction::Xori { lhs, .. }
        | Instruction::Shli { lhs, .. } | Instruction::Shri { lhs, .. } | Instruction::Sari { lhs, .. }
        | Instruction::Rotli { lhs, .. } | Instruction::Rotri { lhs, .. } => { ops.push(lhs); }
        Instruction::Mov { src, .. } => { ops.push(src); }
        Instruction::TestEq { flags, .. } | Instruction::TestNe { flags, .. } | Instruction::TestLt { flags, .. }
        | Instruction::TestLe { flags, .. } | Instruction::TestLtu { flags, .. } | Instruction::TestLeu { flags, .. }
        | Instruction::TestOf { flags, .. } | Instruction::TestCf { flags, .. } | Instruction::TestSf { flags, .. }
        | Instruction::TestGe { flags, .. } | Instruction::TestGt { flags, .. } | Instruction::TestGeu { flags, .. }
        | Instruction::TestGtu { flags, .. } => { ops.push(flags); }
        Instruction::Push { value } => { ops.push(value); }
        Instruction::BrCond { cond, .. } => { ops.push(cond); }
        Instruction::Switch { value, cases, .. } => { ops.push(value); for (cv, _) in cases { ops.push(cv); } }
        Instruction::Call { args, .. } | Instruction::CallIndirect { args, .. } | Instruction::TailCall { args, .. } => { for a in args { ops.push(a); } }
        Instruction::Ret { value: Some(ref v) } => { ops.push(v); }
        Instruction::Phi { incoming, .. } => { for (v, _) in incoming { ops.push(v); } }
        Instruction::Select { cond, true_val, false_val, .. } => { ops.push(cond); ops.push(true_val); ops.push(false_val); }
        _ => {}
    }
    ops
}

fn get_float_binary_operands(inst: &Instruction) -> (&Value, &Value, &Value) {
    match inst {
        Instruction::Fadd { result, lhs, rhs }
        | Instruction::Fsub { result, lhs, rhs }
        | Instruction::Fmul { result, lhs, rhs }
        | Instruction::Fdiv { result, lhs, rhs }
        | Instruction::Fmin { result, lhs, rhs }
        | Instruction::Fmax { result, lhs, rhs } => (result, lhs, rhs),
        _ => unreachable!(),
    }
}

fn get_float_unary_operands(inst: &Instruction) -> (&Value, &Value) {
    match inst {
        Instruction::Fneg { result, operand }
        | Instruction::Fabs { result, operand }
        | Instruction::Fsqrt { result, operand } => (result, operand),
        _ => unreachable!(),
    }
}

fn get_fcmp_operands(inst: &Instruction) -> (&Value, &Value, &Value) {
    match inst {
        Instruction::FcmpEq { result, lhs, rhs }
        | Instruction::FcmpNe { result, lhs, rhs }
        | Instruction::FcmpLt { result, lhs, rhs }
        | Instruction::FcmpLe { result, lhs, rhs }
        | Instruction::FcmpGt { result, lhs, rhs }
        | Instruction::FcmpGe { result, lhs, rhs }
        | Instruction::FcmpOrd { result, lhs, rhs }
        | Instruction::FcmpUno { result, lhs, rhs } => (result, lhs, rhs),
        _ => unreachable!(),
    }
}

fn get_vector_binary_operands(inst: &Instruction) -> (&Value, &Value, &Value) {
    match inst {
        Instruction::Vadd { result, lhs, rhs }
        | Instruction::Vsub { result, lhs, rhs }
        | Instruction::Vmul { result, lhs, rhs }
        | Instruction::Vdiv { result, lhs, rhs } => (result, lhs, rhs),
        _ => unreachable!(),
    }
}

fn type_to_size_suffix(ty: &IrType) -> &str {
    match ty {
        IrType::I8 => "b",
        IrType::I16 => "h",
        IrType::I32 => "w",
        _ => "",
    }
}

fn fcmp_cond(opcode: &str) -> &str {
    match opcode {
        "fcmp_eq" => "eq", "fcmp_ne" => "ne",
        "fcmp_lt" => "lt", "fcmp_le" => "le",
        "fcmp_gt" => "gt", "fcmp_ge" => "ge",
        "fcmp_ord" => "ord", "fcmp_uno" => "uno",
        _ => "eq",
    }
}