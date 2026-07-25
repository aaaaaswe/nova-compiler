/// MIR → NIR lowering.
///
/// Converts Nova MIR (Mid-level IR) to NIR (Nova IR),
/// which is then used by the RISC/CISC/Hybrid backends.
use std::collections::HashMap;

use nova_mir::mir::*;
use nova_mir::mir::MirConstant;
use nova_nir::ir::{AddrExpr, BasicBlock, Function, Instruction, Module};
use nova_nir::types::{IrType, Value};

/// MIR → NIR lowering context.
pub struct NovaLowerer {
    /// Current NIR module being built.
    module: Module,
    /// MIR temp → NIR VReg mapping.
    temp_map: HashMap<String, Value>,
    /// MIR variable → NIR VReg mapping.
    var_map: HashMap<String, Value>,
    /// Per-function vreg counter.
    func_vreg_counter: usize,
}

impl NovaLowerer {
    pub fn new() -> Self {
        NovaLowerer {
            module: Module::new("nova".to_string()),
            temp_map: HashMap::new(),
            var_map: HashMap::new(),
            func_vreg_counter: 0,
        }
    }

    /// Lower a complete MIR program to a NIR module.
    pub fn lower(&mut self, program: &MirProgram) -> Module {
        self.module = Module::new("nova".to_string());

        for func in &program.functions {
            let nir_func = self.lower_function(func);
            self.module.functions.push(nir_func);
        }

        self.module.clone()
    }

    /// Lower a single MIR function to a NIR function.
    fn lower_function(&mut self, func: &MirFunction) -> Function {
        self.func_vreg_counter = 0;
        self.temp_map.clear();
        self.var_map.clear();

        let mut nir_func = Function::new(func.name.clone(), func.call_conv.clone());
        nir_func.return_type = convert_mir_type(&func.return_type);

        // Create parameters
        for (i, param) in func.params.iter().enumerate() {
            let param_val = Value::FuncParam {
                name: format!("%{}", param.name),
                ty: convert_mir_type(&param.ty),
                index: i,
            };
            self.var_map.insert(param.name.clone(), param_val.clone());
            nir_func.parameters.push(param_val);
        }

        // Lower basic blocks
        for bb in &func.basic_blocks {
            let nir_bb = self.lower_basic_block(bb, &nir_func);
            nir_func.basic_blocks.push(nir_bb);
        }

        // Mark entry block
        if let Some(first) = nir_func.basic_blocks.first_mut() {
            first.is_entry = true;
        }

        // Compute predecessors
        self.compute_predecessors(&mut nir_func);

        nir_func
    }

    /// Lower a single MIR basic block to a NIR basic block.
    fn lower_basic_block(&mut self, bb: &MirBasicBlock, _func: &Function) -> BasicBlock {
        let mut nir_bb = BasicBlock::new(bb.name.clone());
        nir_bb.is_entry = bb.is_entry;

        // Lower statements
        for stmt in &bb.stmts {
            let nir_insts = self.lower_stmt(stmt);
            for inst in nir_insts {
                nir_bb.add_instruction(inst);
            }
        }

        // Lower terminator
        let term_insts = self.lower_terminator(&bb.terminator);
        for inst in term_insts {
            nir_bb.add_instruction(inst);
        }

        nir_bb
    }

    /// Lower a MIR statement to NIR instructions.
    fn lower_stmt(&mut self, stmt: &MirStmt) -> Vec<Instruction> {
        match stmt {
            MirStmt::Assign { dst, src } => {
                let dst_val = self.place_to_value(dst);
                self.lower_rvalue(src, &dst_val)
            }
            MirStmt::StorageLive(_name) => {
                // StorageLive is a hint for the optimizer
                vec![Instruction::Nop]
            }
            MirStmt::StorageDead(_name) => {
                // StorageDead is a hint for the optimizer
                vec![Instruction::Nop]
            }
        }
    }

    /// Lower a MIR rvalue to NIR instructions, storing result in dst.
    fn lower_rvalue(&mut self, src: &MirRvalue, dst: &Value) -> Vec<Instruction> {
        match src {
            MirRvalue::Use(operand) => {
                let src_val = self.operand_to_value(operand);
                vec![Instruction::Mov {
                    result: dst.clone(),
                    src: src_val,
                }]
            }
            MirRvalue::Constant(constant) => {
                match constant {
                    MirConstant::Int(v) => vec![Instruction::Movi {
                        result: dst.clone(),
                        imm: *v,
                    }],
                    MirConstant::Float(v) => {
                        // Float constants: store as raw bits in i64, use movi
                        vec![Instruction::Movi {
                            result: dst.clone(),
                            imm: (*v).to_bits() as i64,
                        }]
                    }
                    MirConstant::Bool(b) => vec![Instruction::Movi {
                        result: dst.clone(),
                        imm: if *b { 1 } else { 0 },
                    }],
                    MirConstant::Null => vec![Instruction::Movi {
                        result: dst.clone(),
                        imm: 0,
                    }],
                }
            }
            MirRvalue::BinaryOp(op, left, right) => {
                let lhs_val = self.operand_to_value(left);
                let rhs_val = self.operand_to_value(right);
                let flags = self.new_flags_vreg();

                let inst = match op {
                    MirBinOp::Add => Instruction::Add {
                        result: dst.clone(),
                        lhs: lhs_val,
                        rhs: rhs_val,
                        flags_result: flags,
                    },
                    MirBinOp::Sub => Instruction::Sub {
                        result: dst.clone(),
                        lhs: lhs_val,
                        rhs: rhs_val,
                        flags_result: flags,
                    },
                    MirBinOp::Mul => Instruction::Mul {
                        result: dst.clone(),
                        lhs: lhs_val,
                        rhs: rhs_val,
                        flags_result: flags,
                    },
                    MirBinOp::Div => Instruction::Div {
                        result: dst.clone(),
                        lhs: lhs_val,
                        rhs: rhs_val,
                        flags_result: flags,
                    },
                    MirBinOp::Rem => Instruction::Rem {
                        result: dst.clone(),
                        lhs: lhs_val,
                        rhs: rhs_val,
                        flags_result: flags,
                    },
                    MirBinOp::And => Instruction::And {
                        result: dst.clone(),
                        lhs: lhs_val,
                        rhs: rhs_val,
                        flags_result: flags,
                    },
                    MirBinOp::Or => Instruction::Or {
                        result: dst.clone(),
                        lhs: lhs_val,
                        rhs: rhs_val,
                        flags_result: flags,
                    },
                    MirBinOp::Xor => Instruction::Xor {
                        result: dst.clone(),
                        lhs: lhs_val,
                        rhs: rhs_val,
                        flags_result: flags,
                    },
                    MirBinOp::Shl => Instruction::Shl {
                        result: dst.clone(),
                        lhs: lhs_val,
                        rhs: rhs_val,
                        flags_result: flags,
                    },
                    MirBinOp::Shr => Instruction::Shr {
                        result: dst.clone(),
                        lhs: lhs_val,
                        rhs: rhs_val,
                        flags_result: flags,
                    },
                    MirBinOp::Eq => {
                        let flags = self.new_flags_vreg();
                        let sub_result = self.new_vreg(IrType::I64);
                        let sub = Instruction::Sub {
                            result: sub_result.clone(),
                            lhs: lhs_val,
                            rhs: rhs_val,
                            flags_result: flags.clone(),
                        };
                        let test = Instruction::TestEq {
                            result: dst.clone(),
                            flags,
                        };
                        return vec![sub, test];
                    }
                    MirBinOp::Ne => {
                        let flags = self.new_flags_vreg();
                        let sub_result = self.new_vreg(IrType::I64);
                        let sub = Instruction::Sub {
                            result: sub_result.clone(),
                            lhs: lhs_val,
                            rhs: rhs_val,
                            flags_result: flags.clone(),
                        };
                        let test = Instruction::TestNe {
                            result: dst.clone(),
                            flags,
                        };
                        return vec![sub, test];
                    }
                    MirBinOp::Lt => {
                        let flags = self.new_flags_vreg();
                        let sub_result = self.new_vreg(IrType::I64);
                        let sub = Instruction::Sub {
                            result: sub_result.clone(),
                            lhs: lhs_val,
                            rhs: rhs_val,
                            flags_result: flags.clone(),
                        };
                        let test = Instruction::TestLt {
                            result: dst.clone(),
                            flags,
                        };
                        return vec![sub, test];
                    }
                    MirBinOp::Le => {
                        let flags = self.new_flags_vreg();
                        let sub_result = self.new_vreg(IrType::I64);
                        let sub = Instruction::Sub {
                            result: sub_result.clone(),
                            lhs: lhs_val,
                            rhs: rhs_val,
                            flags_result: flags.clone(),
                        };
                        let test = Instruction::TestLe {
                            result: dst.clone(),
                            flags,
                        };
                        return vec![sub, test];
                    }
                    MirBinOp::Gt => {
                        let flags = self.new_flags_vreg();
                        let sub_result = self.new_vreg(IrType::I64);
                        let sub = Instruction::Sub {
                            result: sub_result.clone(),
                            lhs: lhs_val,
                            rhs: rhs_val,
                            flags_result: flags.clone(),
                        };
                        let test = Instruction::TestGt {
                            result: dst.clone(),
                            flags,
                        };
                        return vec![sub, test];
                    }
                    MirBinOp::Ge => {
                        let flags = self.new_flags_vreg();
                        let sub_result = self.new_vreg(IrType::I64);
                        let sub = Instruction::Sub {
                            result: sub_result.clone(),
                            lhs: lhs_val,
                            rhs: rhs_val,
                            flags_result: flags.clone(),
                        };
                        let test = Instruction::TestGe {
                            result: dst.clone(),
                            flags,
                        };
                        return vec![sub, test];
                    }
                };
                vec![inst]
            }
            MirRvalue::UnaryOp(op, operand) => {
                let op_val = self.operand_to_value(operand);
                let flags = self.new_flags_vreg();
                let inst = match op {
                    MirUnaryOp::Neg => Instruction::Neg {
                        result: dst.clone(),
                        operand: op_val,
                        flags_result: flags,
                    },
                    MirUnaryOp::Not => Instruction::Not {
                        result: dst.clone(),
                        operand: op_val,
                        flags_result: flags,
                    },
                };
                vec![inst]
            }
            MirRvalue::Call { func, args } => {
                let mut nir_args = Vec::new();
                for arg in args {
                    nir_args.push(self.operand_to_value(arg));
                }
                vec![Instruction::Call {
                    result: Some(dst.clone()),
                    callee_name: func.clone(),
                    args: nir_args,
                }]
            }
            MirRvalue::Ref(place) => {
                let addr_val = self.place_to_addr(place);
                vec![Instruction::Lea {
                    result: dst.clone(),
                    addr: addr_val,
                }]
            }
        }
    }

    /// Lower a MIR terminator to NIR instructions.
    fn lower_terminator(&mut self, term: &MirTerminator) -> Vec<Instruction> {
        match term {
            MirTerminator::Return(operand) => {
                let val = operand.as_ref().map(|op| self.operand_to_value(op));
                vec![Instruction::Ret { value: val }]
            }
            MirTerminator::Goto(target) => {
                vec![Instruction::Br {
                    target_bb: target.clone(),
                }]
            }
            MirTerminator::If { cond, then_bb, else_bb } => {
                let cond_val = self.operand_to_value(cond);
                vec![Instruction::BrCond {
                    cond: cond_val,
                    true_bb: then_bb.clone(),
                    false_bb: else_bb.clone(),
                }]
            }
            MirTerminator::Call { func, args, dest, next_bb } => {
                let mut nir_args = Vec::new();
                for arg in args {
                    nir_args.push(self.operand_to_value(arg));
                }
                let result = dest.as_ref().map(|d| self.place_to_value(d));
                vec![
                    Instruction::Call {
                        result,
                        callee_name: func.clone(),
                        args: nir_args,
                    },
                    Instruction::Br {
                        target_bb: next_bb.clone(),
                    },
                ]
            }
            MirTerminator::Unreachable => {
                vec![Instruction::Nop]
            }
        }
    }

    /// Convert a MIR operand to a NIR Value.
    fn operand_to_value(&mut self, operand: &MirOperand) -> Value {
        match &operand.place {
            MirPlace::Var(name) => {
                if let Some(v) = self.var_map.get(name) {
                    return v.clone();
                }
                let v = Value::VReg {
                    name: format!("%{}", name),
                    ty: convert_mir_type(&operand.ty),
                };
                self.var_map.insert(name.clone(), v.clone());
                v
            }
            MirPlace::Temp(n) => {
                let key = format!("t{}", n);
                if let Some(v) = self.temp_map.get(&key) {
                    return v.clone();
                }
                let v = Value::VReg {
                    name: format!("%t{}", n),
                    ty: convert_mir_type(&operand.ty),
                };
                self.temp_map.insert(key, v.clone());
                v
            }
            MirPlace::Deref(inner) => {
                let _inner_val = self.place_to_value(inner);
                let loaded = self.new_vreg(convert_mir_type(&operand.ty));
                loaded
            }
            MirPlace::Field(inner, _field) => {
                let _inner_val = self.place_to_value(inner);
                let _addr = self.new_vreg(IrType::Ptr);
                let loaded = self.new_vreg(convert_mir_type(&operand.ty));
                loaded
            }
        }
    }

    /// Convert a MIR place to a NIR Value.
    fn place_to_value(&mut self, place: &MirPlace) -> Value {
        match place {
            MirPlace::Var(name) => {
                if let Some(v) = self.var_map.get(name) {
                    return v.clone();
                }
                let v = Value::VReg {
                    name: format!("%{}", name),
                    ty: IrType::I64,
                };
                self.var_map.insert(name.clone(), v.clone());
                v
            }
            MirPlace::Temp(n) => {
                let key = format!("t{}", n);
                if let Some(v) = self.temp_map.get(&key) {
                    return v.clone();
                }
                let v = Value::VReg {
                    name: format!("%t{}", n),
                    ty: IrType::I64,
                };
                self.temp_map.insert(key, v.clone());
                v
            }
            MirPlace::Deref(inner) => {
                let _inner_val = self.place_to_value(inner);
                let result = self.new_vreg(IrType::I64);
                result
            }
            MirPlace::Field(inner, _field) => {
                let _inner_val = self.place_to_value(inner);
                let result = self.new_vreg(IrType::I64);
                result
            }
        }
    }

    /// Convert a MIR place to an AddrExpr for lea/load/store.
    fn place_to_addr(&mut self, place: &MirPlace) -> AddrExpr {
        match place {
            MirPlace::Var(name) => {
                let v = self.var_map.get(name).cloned().unwrap_or_else(|| {
                    Value::VReg {
                        name: format!("%{}", name),
                        ty: IrType::Ptr,
                    }
                });
                AddrExpr {
                    base: v,
                    index: None,
                    scale: 1,
                    offset: 0,
                }
            }
            MirPlace::Temp(n) => {
                let key = format!("t{}", n);
                let v = self.temp_map.get(&key).cloned().unwrap_or_else(|| {
                    Value::VReg {
                        name: format!("%t{}", n),
                        ty: IrType::Ptr,
                    }
                });
                AddrExpr {
                    base: v,
                    index: None,
                    scale: 1,
                    offset: 0,
                }
            }
            MirPlace::Deref(_inner) => {
                // Dereference: load the address first
                AddrExpr {
                    base: Value::VReg {
                        name: "%sp".to_string(),
                        ty: IrType::Ptr,
                    },
                    index: None,
                    scale: 1,
                    offset: 0,
                }
            }
            MirPlace::Field(_inner, _field) => {
                // Field access: offset from base
                AddrExpr {
                    base: Value::VReg {
                        name: "%sp".to_string(),
                        ty: IrType::Ptr,
                    },
                    index: None,
                    scale: 1,
                    offset: 0,
                }
            }
        }
    }

    /// Generate a new virtual register.
    fn new_vreg(&mut self, ty: IrType) -> Value {
        let name = format!("%v{}", self.func_vreg_counter);
        self.func_vreg_counter += 1;
        Value::VReg { name, ty }
    }

    /// Generate a new flags virtual register.
    fn new_flags_vreg(&mut self) -> Value {
        let name = format!("%f{}", self.func_vreg_counter);
        self.func_vreg_counter += 1;
        Value::VReg { name, ty: IrType::Flags }
    }

    /// Compute predecessor relationships for all basic blocks.
    fn compute_predecessors(&self, func: &mut Function) {
        let mut preds: HashMap<String, Vec<String>> = HashMap::new();

        for bb in &func.basic_blocks {
            for succ in &bb.successors {
                preds.entry(succ.clone()).or_default().push(bb.name.clone());
            }
        }

        for bb in &mut func.basic_blocks {
            bb.predecessors = preds.get(&bb.name).cloned().unwrap_or_default();
        }
    }
}

/// Convert MIR type to NIR IrType.
fn convert_mir_type(ty: &MirType) -> IrType {
    match ty {
        MirType::I8 => IrType::I8,
        MirType::I16 => IrType::I16,
        MirType::I32 => IrType::I32,
        MirType::I64 => IrType::I64,
        MirType::U8 => IrType::I8,
        MirType::U16 => IrType::I16,
        MirType::U32 => IrType::I32,
        MirType::U64 => IrType::I64,
        MirType::F32 => IrType::F32,
        MirType::F64 => IrType::F64,
        MirType::Bool => IrType::I1,
        MirType::Void => IrType::Void,
        MirType::Ptr(_) => IrType::Ptr,
        MirType::Array(inner, _) => convert_mir_type(inner),
        MirType::Struct(_) => IrType::Ptr,
    }
}

/// Lower a complete MIR program to NIR module.
pub fn lower_mir_to_nir(program: &MirProgram) -> Module {
    let mut lowerer = NovaLowerer::new();
    lowerer.lower(program)
}