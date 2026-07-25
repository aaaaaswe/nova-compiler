/// HIR → MIR lowering.
///
/// Converts HIR control flow (if/while/for/loop) to basic blocks + gotos,
/// converts HIR expressions to MIR statements with temporary variables,
/// and computes the basic block CFG (predecessors/successors).
use std::collections::HashMap;
use std::mem;

use nova_hir::hir::*;
use crate::mir::{MirConstant, *};

/// Context for HIR → MIR lowering.
pub struct MirLowerer {
    /// Current function being lowered.
    current_func: Option<MirFunction>,
    /// Temp counter for generating unique temporary variables.
    temp_counter: usize,
    /// Block counter for generating unique block names.
    block_counter: usize,
    /// Variable name → MIR place mapping.
    var_map: HashMap<String, MirPlace>,
    /// Struct definitions for field access lowering.
    structs: HashMap<String, Vec<(String, HirType)>>,
}

impl MirLowerer {
    pub fn new() -> Self {
        MirLowerer {
            current_func: None,
            temp_counter: 0,
            block_counter: 0,
            var_map: HashMap::new(),
            structs: HashMap::new(),
        }
    }

    /// Lower a complete HIR program to MIR.
    pub fn lower(&mut self, program: &HirProgram) -> MirProgram {
        let mut functions = Vec::new();

        // Collect struct info
        for item in &program.items {
            if let HirItem::Struct(s) = item {
                self.structs.insert(s.name.clone(), s.fields.clone());
            }
        }

        for item in &program.items {
            if let HirItem::Function(func) = item {
                functions.push(self.lower_function(func));
            }
        }

        MirProgram { functions }
    }

    fn lower_function(&mut self, func: &HirFunction) -> MirFunction {
        self.temp_counter = 0;
        self.block_counter = 0;
        self.var_map.clear();

        let mut mir_func = MirFunction {
            name: func.name.clone(),
            params: func.params.iter().map(|p| MirParam {
                name: p.name.clone(),
                ty: convert_hir_type(&p.ty),
            }).collect(),
            return_type: convert_hir_type(&func.return_type),
            basic_blocks: Vec::new(),
            call_conv: func.call_conv.to_string(),
        };

        // Register parameters
        for (_i, param) in func.params.iter().enumerate() {
            self.var_map.insert(param.name.clone(), MirPlace::Var(param.name.clone()));
        }

        self.current_func = Some(mir_func.clone());

        // Create entry block
        let entry_block = self.lower_block(&func.body, "entry".to_string());
        let _blocks = vec![entry_block];

        // Continue lowering any blocks that were created
        // (In practice, the block lowering is recursive)

        self.current_func = Some(mir_func.clone());

        // Re-lower to get all blocks
        let mut final_blocks = Vec::new();
        let _entry = self.lower_block_cfg(&func.body, "entry".to_string(), &mut final_blocks);

        // Compute predecessors and successors
        self.compute_cfg(&mut final_blocks);

        // Mark entry block
        if let Some(first) = final_blocks.first_mut() {
            first.is_entry = true;
        }

        mir_func.basic_blocks = final_blocks;
        mir_func
    }

    /// Lower a block to a flat list of MIR basic blocks (CFG).
    fn lower_block_cfg(
        &mut self,
        block: &HirBlock,
        bb_name: String,
        all_blocks: &mut Vec<MirBasicBlock>,
    ) -> String {
        let mut stmts = Vec::new();
        let mut current_bb = bb_name;

        for stmt in &block.stmts {
            match stmt {
                HirStmt::Let { name, mutable: _, ty: _, init } => {
                    let place = MirPlace::Var(name.clone());
                    self.var_map.insert(name.clone(), place.clone());
                    if let Some(ref init_expr) = init {
                        let (init_stmts, init_operand) = self.lower_expr_to_operand(init_expr);
                        stmts.extend(init_stmts);
                        stmts.push(MirStmt::Assign {
                            dst: place,
                            src: MirRvalue::Use(init_operand),
                        });
                    }
                }
                HirStmt::Expr(expr) => {
                    let (expr_stmts, _operand) = self.lower_expr_to_operand(expr);
                    stmts.extend(expr_stmts);
                }
                HirStmt::Return(expr) => {
                    let operand = expr.as_ref().map(|e| {
                        let (e_stmts, op) = self.lower_expr_to_operand(e);
                        stmts.extend(e_stmts);
                        op
                    });
                    let bb = MirBasicBlock {
                        name: current_bb.clone(),
                        stmts,
                        terminator: MirTerminator::Return(operand),
                        predecessors: Vec::new(),
                        successors: Vec::new(),
                        is_entry: false,
                    };
                    all_blocks.push(bb);
                    return current_bb;
                }
                HirStmt::If { cond, then_block, else_block } => {
                    let (cond_stmts, cond_op) = self.lower_expr_to_operand(cond);
                    stmts.extend(cond_stmts);

                    let then_bb_name = self.fresh_bb_name("then");
                    let else_bb_name = self.fresh_bb_name("else");
                    let merge_bb_name = self.fresh_bb_name("merge");

                    let if_bb = MirBasicBlock {
                        name: current_bb.clone(),
                        stmts: mem::take(&mut stmts),
                        terminator: MirTerminator::If {
                            cond: cond_op,
                            then_bb: then_bb_name.clone(),
                            else_bb: else_bb_name.clone(),
                        },
                        predecessors: Vec::new(),
                        successors: vec![then_bb_name.clone(), else_bb_name.clone()],
                        is_entry: false,
                    };
                    all_blocks.push(if_bb);

                    // Then block
                    let _then_bb = self.lower_block_cfg(then_block, then_bb_name, all_blocks);
                    // Add goto to merge from then
                    if let Some(last) = all_blocks.last_mut() {
                        if matches!(last.terminator, MirTerminator::Goto(_) | MirTerminator::Return(_) | MirTerminator::If { .. }) {
                            // Already has a terminator
                        } else {
                            last.terminator = MirTerminator::Goto(merge_bb_name.clone());
                            last.successors.push(merge_bb_name.clone());
                        }
                    }

                    // Else block
                    if let Some(ref else_b) = else_block {
                        let _else_bb = self.lower_block_cfg(else_b, else_bb_name, all_blocks);
                        if let Some(last) = all_blocks.last_mut() {
                            if matches!(last.terminator, MirTerminator::Goto(_) | MirTerminator::Return(_) | MirTerminator::If { .. }) {
                                // Already has a terminator
                            } else {
                                last.terminator = MirTerminator::Goto(merge_bb_name.clone());
                                last.successors.push(merge_bb_name.clone());
                            }
                        }
                    } else {
                        // Empty else: just goto merge
                        let else_bb = MirBasicBlock {
                            name: else_bb_name,
                            stmts: Vec::new(),
                            terminator: MirTerminator::Goto(merge_bb_name.clone()),
                            predecessors: Vec::new(),
                            successors: vec![merge_bb_name.clone()],
                            is_entry: false,
                        };
                        all_blocks.push(else_bb);
                    }

                    // Continue processing remaining statements in the merge block
                    current_bb = merge_bb_name;
                    // stmts is already empty (taken by if_bb above)
                }
                HirStmt::While { cond, body } => {
                    let header_bb_name = self.fresh_bb_name("while_header");
                    let body_bb_name = self.fresh_bb_name("while_body");
                    let exit_bb_name = self.fresh_bb_name("while_exit");
                    let header_clone = header_bb_name.clone();

                    // Current block falls through to header
                    let header_bb = MirBasicBlock {
                        name: current_bb.clone(),
                        stmts: mem::take(&mut stmts),
                        terminator: MirTerminator::Goto(header_bb_name.clone()),
                        predecessors: Vec::new(),
                        successors: vec![header_bb_name.clone()],
                        is_entry: false,
                    };
                    all_blocks.push(header_bb);

                    // Header block: evaluate condition
                    let (cond_stmts, cond_op) = self.lower_expr_to_operand(cond);
                    let cond_bb = MirBasicBlock {
                        name: header_bb_name,
                        stmts: cond_stmts,
                        terminator: MirTerminator::If {
                            cond: cond_op,
                            then_bb: body_bb_name.clone(),
                            else_bb: exit_bb_name.clone(),
                        },
                        predecessors: Vec::new(),
                        successors: vec![body_bb_name.clone(), exit_bb_name.clone()],
                        is_entry: false,
                    };
                    all_blocks.push(cond_bb);

                    // Body block
                    let _body_bb = self.lower_block_cfg(body, body_bb_name, all_blocks);
                    // Add goto back to header
                    if let Some(last) = all_blocks.last_mut() {
                        if matches!(last.terminator, MirTerminator::Goto(_) | MirTerminator::If { .. }) {
                            // Already has a proper terminator
                        } else if matches!(last.terminator, MirTerminator::Return(Some(_))) {
                            // Explicit return from body: keep it
                        } else {
                            // Implicit fall-through (Return(None) or Unreachable): loop back to header
                            last.terminator = MirTerminator::Goto(header_clone.clone());
                            last.successors = vec![header_clone.clone()];
                        }
                    }

                    // Continue processing remaining statements in the exit block
                    current_bb = exit_bb_name;
                    // stmts is already empty (taken by header_bb above)
                }
                HirStmt::For { var, iter, body } => {
                    // Simple for loop: lower to while-like structure
                    let (init_stmts, _start_op) = match iter {
                        HirForIter::Range { start, end: _, inclusive: _ } => {
                            let (s, op) = self.lower_expr_to_operand(start);
                            let place = MirPlace::Var(var.clone());
                            self.var_map.insert(var.clone(), place.clone());
                            (s, op)
                        }
                        HirForIter::Array(_) => {
                            // Simplified: just create a loop counter
                            (Vec::new(), MirOperand {
                                place: MirPlace::Temp(0),
                                ty: MirType::I64,
                            })
                        }
                    };

                    let header_bb_name = self.fresh_bb_name("for_header");
                    let body_bb_name = self.fresh_bb_name("for_body");
                    let exit_bb_name = self.fresh_bb_name("for_exit");
                    let for_header_clone = header_bb_name.clone();

                    // Current block
                    let mut entry_stmts = mem::take(&mut stmts);
                    entry_stmts.extend(init_stmts);
                    // Initialize loop variable
                    let var_place = MirPlace::Var(var.clone());
                    let (start_stmts, start_op) = match iter {
                        HirForIter::Range { start, .. } => self.lower_expr_to_operand(start),
                        HirForIter::Array(_) => (Vec::new(), MirOperand {
                            place: MirPlace::Temp(0),
                            ty: MirType::I64,
                        }),
                    };
                    entry_stmts.extend(start_stmts);
                    entry_stmts.push(MirStmt::Assign {
                        dst: var_place,
                        src: MirRvalue::Use(start_op),
                    });

                    let entry_bb = MirBasicBlock {
                        name: current_bb.clone(),
                        stmts: entry_stmts,
                        terminator: MirTerminator::Goto(header_bb_name.clone()),
                        predecessors: Vec::new(),
                        successors: vec![header_bb_name.clone()],
                        is_entry: false,
                    };
                    all_blocks.push(entry_bb);

                    // Header: check condition var < end
                    let end_operand = match iter {
                        HirForIter::Range { end, .. } => {
                            let (e_stmts, op) = self.lower_expr_to_operand(end);
                            let mut h_stmts = e_stmts;
                            let var_op = MirOperand {
                                place: MirPlace::Var(var.clone()),
                                ty: MirType::I64,
                            };
                            let cmp_temp = self.fresh_temp(MirType::Bool);
                            h_stmts.push(MirStmt::Assign {
                                dst: cmp_temp.clone(),
                                src: MirRvalue::BinaryOp(MirBinOp::Lt, Box::new(var_op), Box::new(op.clone())),
                            });
                            let cond_op = MirOperand {
                                place: cmp_temp,
                                ty: MirType::Bool,
                            };
                            (h_stmts, cond_op)
                        }
                        HirForIter::Array(_) => {
                            // Simplified condition
                            (Vec::new(), MirOperand {
                                place: MirPlace::Temp(0),
                                ty: MirType::Bool,
                            })
                        }
                    };

                    let header_bb = MirBasicBlock {
                        name: header_bb_name,
                        stmts: end_operand.0,
                        terminator: MirTerminator::If {
                            cond: end_operand.1,
                            then_bb: body_bb_name.clone(),
                            else_bb: exit_bb_name.clone(),
                        },
                        predecessors: Vec::new(),
                        successors: vec![body_bb_name.clone(), exit_bb_name.clone()],
                        is_entry: false,
                    };
                    all_blocks.push(header_bb);

                    // Body block
                    let _body_bb = self.lower_block_cfg(body, body_bb_name, all_blocks);
                    // Increment loop variable at end of body
                    if let Some(last) = all_blocks.last_mut() {
                        if matches!(last.terminator, MirTerminator::Goto(_) | MirTerminator::If { .. }) {
                            // Already has a proper terminator
                        } else if matches!(last.terminator, MirTerminator::Return(Some(_))) {
                            // Explicit return from body: keep it
                        } else {
                            // Add increment
                            let var_op = MirOperand {
                                place: MirPlace::Var(var.clone()),
                                ty: MirType::I64,
                            };
                            let one_op = MirOperand {
                                place: MirPlace::Temp(self.temp_counter),
                                ty: MirType::I64,
                            };
                            self.temp_counter += 1;
                            last.stmts.push(MirStmt::Assign {
                                dst: one_op.place.clone(),
                                src: MirRvalue::Use(MirOperand {
                                    place: MirPlace::Temp(0),
                                    ty: MirType::I64,
                                }),
                            });
                            last.stmts.push(MirStmt::Assign {
                                dst: MirPlace::Var(var.clone()),
                                src: MirRvalue::BinaryOp(MirBinOp::Add, Box::new(var_op), Box::new(one_op.clone())),
                            });
                            last.terminator = MirTerminator::Goto(for_header_clone.clone());
                            last.successors = vec![for_header_clone];
                        }
                    }

                    // Continue processing remaining statements in the exit block
                    current_bb = exit_bb_name;
                    // stmts is already empty (taken by entry_bb above)
                }
                HirStmt::Loop { body } => {
                    let body_bb_name = self.fresh_bb_name("loop_body");
                    let header_bb_name = self.fresh_bb_name("loop_header");
                    let loop_header_clone = header_bb_name.clone();

                    // Entry block goes to header
                    let entry_bb = MirBasicBlock {
                        name: current_bb.clone(),
                        stmts: mem::take(&mut stmts),
                        terminator: MirTerminator::Goto(header_bb_name.clone()),
                        predecessors: Vec::new(),
                        successors: vec![header_bb_name.clone()],
                        is_entry: false,
                    };
                    all_blocks.push(entry_bb);

                    // Header block (just a label)
                    let header_bb = MirBasicBlock {
                        name: header_bb_name,
                        stmts: Vec::new(),
                        terminator: MirTerminator::Goto(body_bb_name.clone()),
                        predecessors: Vec::new(),
                        successors: vec![body_bb_name.clone()],
                        is_entry: false,
                    };
                    all_blocks.push(header_bb);

                    // Body block
                    let _body_bb = self.lower_block_cfg(body, body_bb_name.clone(), all_blocks);
                    // Loop back to header
                    if let Some(last) = all_blocks.last_mut() {
                        if matches!(last.terminator, MirTerminator::Goto(_) | MirTerminator::If { .. }) {
                            // Already has a proper terminator
                        } else if matches!(last.terminator, MirTerminator::Return(Some(_))) {
                            // Explicit return from body: keep it
                        } else {
                            last.terminator = MirTerminator::Goto(loop_header_clone.clone());
                            last.successors = vec![loop_header_clone];
                        }
                    }
                    // Continue processing with body_bb_name as current (though loop has no exit)
                    current_bb = body_bb_name;
                    // stmts is already empty (taken by entry_bb above)
                }
                HirStmt::Break => {
                    // Simplified: just push current block with unreachable
                    let bb = MirBasicBlock {
                        name: current_bb.clone(),
                        stmts: mem::take(&mut stmts),
                        terminator: MirTerminator::Unreachable,
                        predecessors: Vec::new(),
                        successors: Vec::new(),
                        is_entry: false,
                    };
                    all_blocks.push(bb);
                    current_bb = self.fresh_bb_name("after_break");
                }
                HirStmt::Continue => {
                    let bb = MirBasicBlock {
                        name: current_bb.clone(),
                        stmts: mem::take(&mut stmts),
                        terminator: MirTerminator::Unreachable,
                        predecessors: Vec::new(),
                        successors: Vec::new(),
                        is_entry: false,
                    };
                    all_blocks.push(bb);
                    current_bb = self.fresh_bb_name("after_continue");
                }
                HirStmt::Assign { target, value } => {
                    let (val_stmts, val_op) = self.lower_expr_to_operand(value);
                    let (tgt_stmts, tgt_place) = self.lower_expr_to_place(target);
                    stmts.extend(val_stmts);
                    stmts.extend(tgt_stmts);
                    stmts.push(MirStmt::Assign {
                        dst: tgt_place,
                        src: MirRvalue::Use(val_op),
                    });
                }
                HirStmt::Unsafe(block) => {
                    let unsafe_bb_name = self.fresh_bb_name("unsafe_body");
                    let inner_bb = self.lower_block_cfg(block, unsafe_bb_name, all_blocks);
                    // Simplified: just continue
                    let bb = MirBasicBlock {
                        name: current_bb.clone(),
                        stmts: mem::take(&mut stmts),
                        terminator: MirTerminator::Goto(inner_bb),
                        predecessors: Vec::new(),
                        successors: Vec::new(),
                        is_entry: false,
                    };
                    all_blocks.push(bb);
                    current_bb = self.fresh_bb_name("after_unsafe");
                }
                HirStmt::Asm(_) => {
                    // Inline assembly is preserved as-is
                    // We'll just skip it for now
                }
            }
        }

        // Handle tail expression
        let terminator = if let Some(ref expr) = block.expr {
            let (expr_stmts, operand) = self.lower_expr_to_operand(expr);
            stmts.extend(expr_stmts);
            MirTerminator::Return(Some(operand))
        } else {
            MirTerminator::Return(None)
        };

        let bb = MirBasicBlock {
            name: current_bb.clone(),
            stmts,
            terminator,
            predecessors: Vec::new(),
            successors: Vec::new(),
            is_entry: false,
        };
        all_blocks.push(bb);
        current_bb
    }

    /// Lower an HIR block to a single MIR basic block (flat, no CFG).
    fn lower_block(&mut self, block: &HirBlock, name: String) -> MirBasicBlock {
        let mut stmts = Vec::new();

        for stmt in &block.stmts {
            match stmt {
                HirStmt::Let { name, mutable: _, ty: _, init } => {
                    let place = MirPlace::Var(name.clone());
                    self.var_map.insert(name.clone(), place.clone());
                    if let Some(ref init_expr) = init {
                        let (init_stmts, init_operand) = self.lower_expr_to_operand(init_expr);
                        stmts.extend(init_stmts);
                        stmts.push(MirStmt::Assign {
                            dst: place,
                            src: MirRvalue::Use(init_operand),
                        });
                    }
                }
                HirStmt::Expr(expr) => {
                    let (expr_stmts, _) = self.lower_expr_to_operand(expr);
                    stmts.extend(expr_stmts);
                }
                HirStmt::Return(expr) => {
                    let operand = expr.as_ref().map(|e| {
                        let (e_stmts, op) = self.lower_expr_to_operand(e);
                        stmts.extend(e_stmts);
                        op
                    });
                    return MirBasicBlock {
                        name,
                        stmts,
                        terminator: MirTerminator::Return(operand),
                        predecessors: Vec::new(),
                        successors: Vec::new(),
                        is_entry: false,
                    };
                }
                HirStmt::If { cond, then_block: _, else_block: _ } => {
                    let (cond_stmts, cond_op) = self.lower_expr_to_operand(cond);
                    stmts.extend(cond_stmts);

                    let then_bb_name = self.fresh_bb_name("then");
                    let else_bb_name = self.fresh_bb_name("else");
                    let _merge_bb_name = self.fresh_bb_name("merge");
                    let then_clone = then_bb_name.clone();
                    let else_clone = else_bb_name.clone();

                    return MirBasicBlock {
                        name,
                        stmts,
                        terminator: MirTerminator::If {
                            cond: cond_op,
                            then_bb: then_bb_name,
                            else_bb: else_bb_name,
                        },
                        predecessors: Vec::new(),
                        successors: vec![then_clone, else_clone],
                        is_entry: false,
                    };
                }
                HirStmt::While { cond, body: _ } => {
                    let (cond_stmts, cond_op) = self.lower_expr_to_operand(cond);
                    stmts.extend(cond_stmts);

                    let body_bb = self.fresh_bb_name("while_body");
                    let exit_bb = self.fresh_bb_name("while_exit");
                    let body_clone = body_bb.clone();
                    let exit_clone = exit_bb.clone();

                    return MirBasicBlock {
                        name,
                        stmts,
                        terminator: MirTerminator::If {
                            cond: cond_op,
                            then_bb: body_bb,
                            else_bb: exit_bb,
                        },
                        predecessors: Vec::new(),
                        successors: vec![body_clone, exit_clone],
                        is_entry: false,
                    };
                }
                HirStmt::For { var: _, iter: _, body: _ } => {
                    let (iter_stmts, _) = self.lower_expr_to_operand(
                        &HirExpr::IntLiteral { value: 0, ty: HirType::I64 }
                    );
                    stmts.extend(iter_stmts);

                    let body_bb = self.fresh_bb_name("for_body");
                    let _exit_bb = self.fresh_bb_name("for_exit");
                    let body_clone = body_bb.clone();

                    return MirBasicBlock {
                        name,
                        stmts,
                        terminator: MirTerminator::Goto(body_bb),
                        predecessors: Vec::new(),
                        successors: vec![body_clone],
                        is_entry: false,
                    };
                }
                HirStmt::Loop { body: _ } => {
                    let body_bb = self.fresh_bb_name("loop_body");
                    let body_clone = body_bb.clone();
                    return MirBasicBlock {
                        name,
                        stmts,
                        terminator: MirTerminator::Goto(body_bb),
                        predecessors: Vec::new(),
                        successors: vec![body_clone],
                        is_entry: false,
                    };
                }
                HirStmt::Break | HirStmt::Continue => {
                    // These will be handled at the CFG level
                    stmts.push(MirStmt::Assign {
                        dst: MirPlace::Temp(self.temp_counter),
                        src: MirRvalue::Use(MirOperand {
                            place: MirPlace::Temp(0),
                            ty: MirType::I64,
                        }),
                    });
                    self.temp_counter += 1;
                }
                HirStmt::Assign { target, value } => {
                    let (val_stmts, val_op) = self.lower_expr_to_operand(value);
                    let (tgt_stmts, tgt_place) = self.lower_expr_to_place(target);
                    stmts.extend(val_stmts);
                    stmts.extend(tgt_stmts);
                    stmts.push(MirStmt::Assign {
                        dst: tgt_place,
                        src: MirRvalue::Use(val_op),
                    });
                }
                HirStmt::Unsafe(_) => {}
                HirStmt::Asm(_) => {}
            }
        }

        let terminator = if let Some(ref expr) = block.expr {
            let (expr_stmts, operand) = self.lower_expr_to_operand(expr);
            stmts.extend(expr_stmts);
            MirTerminator::Return(Some(operand))
        } else {
            MirTerminator::Return(None)
        };

        MirBasicBlock {
            name,
            stmts,
            terminator,
            predecessors: Vec::new(),
            successors: Vec::new(),
            is_entry: true,
        }
    }

    /// Lower an HIR expression to MIR statements and a single operand.
    fn lower_expr_to_operand(&mut self, expr: &HirExpr) -> (Vec<MirStmt>, MirOperand) {
        match expr {
            HirExpr::Ident { name, ty } => {
                if let Some(place) = self.var_map.get(name) {
                    (Vec::new(), MirOperand {
                        place: place.clone(),
                        ty: convert_hir_type(ty),
                    })
                } else {
                    // Function reference or parameter
                    let place = MirPlace::Var(name.clone());
                    (Vec::new(), MirOperand {
                        place,
                        ty: convert_hir_type(ty),
                    })
                }
            }
            HirExpr::IntLiteral { value, ty } => {
                let temp = self.fresh_temp(convert_hir_type(ty));
                let mut stmts = Vec::new();
                stmts.push(MirStmt::Assign {
                    dst: temp.clone(),
                    src: MirRvalue::Constant(MirConstant::Int(*value)),
                });
                (stmts, MirOperand {
                    place: temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::FloatLiteral { value, ty } => {
                let temp = self.fresh_temp(convert_hir_type(ty));
                let mut stmts = Vec::new();
                stmts.push(MirStmt::Assign {
                    dst: temp.clone(),
                    src: MirRvalue::Constant(MirConstant::Float(*value)),
                });
                (stmts, MirOperand {
                    place: temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::BoolLiteral { value, ty } => {
                let temp = self.fresh_temp(convert_hir_type(ty));
                let mut stmts = Vec::new();
                stmts.push(MirStmt::Assign {
                    dst: temp.clone(),
                    src: MirRvalue::Constant(MirConstant::Bool(*value)),
                });
                (stmts, MirOperand {
                    place: temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::Binary { left, op, right, ty } => {
                let (mut left_stmts, left_op) = self.lower_expr_to_operand(left);
                let (right_stmts, right_op) = self.lower_expr_to_operand(right);
                left_stmts.extend(right_stmts);
                let mir_op = convert_binop(op);
                let result_temp = self.fresh_temp(convert_hir_type(ty));
                left_stmts.push(MirStmt::Assign {
                    dst: result_temp.clone(),
                    src: MirRvalue::BinaryOp(mir_op, Box::new(left_op), Box::new(right_op)),
                });
                (left_stmts, MirOperand {
                    place: result_temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::Unary { op, expr, ty } => {
                let (mut stmts, operand) = self.lower_expr_to_operand(expr);
                let mir_op = match op {
                    HirUnaryOp::Neg => MirUnaryOp::Neg,
                    HirUnaryOp::Not => MirUnaryOp::Not,
                    HirUnaryOp::Deref => {
                        let temp = self.fresh_temp(convert_hir_type(ty));
                        return (stmts, MirOperand {
                            place: temp,
                            ty: convert_hir_type(ty),
                        });
                    }
                    HirUnaryOp::Ref | HirUnaryOp::RefMut => {
                        let temp = self.fresh_temp(convert_hir_type(ty));
                        return (stmts, MirOperand {
                            place: temp,
                            ty: convert_hir_type(ty),
                        });
                    }
                };
                let result_temp = self.fresh_temp(convert_hir_type(ty));
                stmts.push(MirStmt::Assign {
                    dst: result_temp.clone(),
                    src: MirRvalue::UnaryOp(mir_op, Box::new(operand)),
                });
                (stmts, MirOperand {
                    place: result_temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::Call { func, args, ty } => {
                let mut stmts = Vec::new();
                let func_name = match func.as_ref() {
                    HirExpr::Ident { name, .. } => name.clone(),
                    _ => "unknown".to_string(),
                };

                let mut mir_args = Vec::new();
                for arg in args {
                    let (arg_stmts, arg_op) = self.lower_expr_to_operand(arg);
                    stmts.extend(arg_stmts);
                    mir_args.push(arg_op);
                }

                let result_temp = self.fresh_temp(convert_hir_type(ty));
                stmts.push(MirStmt::Assign {
                    dst: result_temp.clone(),
                    src: MirRvalue::Call {
                        func: func_name,
                        args: mir_args,
                    },
                });
                (stmts, MirOperand {
                    place: result_temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::FieldAccess { expr, field, ty } => {
                let (mut stmts, operand) = self.lower_expr_to_operand(expr);
                let result_temp = self.fresh_temp(convert_hir_type(ty));
                stmts.push(MirStmt::Assign {
                    dst: result_temp.clone(),
                    src: MirRvalue::Use(MirOperand {
                        place: MirPlace::Field(Box::new(operand.place), field.clone()),
                        ty: convert_hir_type(ty),
                    }),
                });
                (stmts, MirOperand {
                    place: result_temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::Index { expr, index, ty } => {
                let (mut stmts, base_op) = self.lower_expr_to_operand(expr);
                let (idx_stmts, idx_op) = self.lower_expr_to_operand(index);
                stmts.extend(idx_stmts);
                let result_temp = self.fresh_temp(convert_hir_type(ty));
                // Array indexing: base + index * element_size
                stmts.push(MirStmt::Assign {
                    dst: result_temp.clone(),
                    src: MirRvalue::BinaryOp(MirBinOp::Add, Box::new(base_op), Box::new(idx_op)),
                });
                (stmts, MirOperand {
                    place: result_temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::Cast { expr, ty } => {
                let (mut stmts, operand) = self.lower_expr_to_operand(expr);
                let result_temp = self.fresh_temp(convert_hir_type(ty));
                stmts.push(MirStmt::Assign {
                    dst: result_temp.clone(),
                    src: MirRvalue::Use(operand),
                });
                (stmts, MirOperand {
                    place: result_temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::StructLit { name: _, fields, ty } => {
                let mut stmts = Vec::new();
                let result_temp = self.fresh_temp(convert_hir_type(ty));
                for (field_name, field_expr) in fields {
                    let (f_stmts, f_op) = self.lower_expr_to_operand(field_expr);
                    stmts.extend(f_stmts);
                    stmts.push(MirStmt::Assign {
                        dst: MirPlace::Field(Box::new(result_temp.clone()), field_name.clone()),
                        src: MirRvalue::Use(f_op),
                    });
                }
                (stmts, MirOperand {
                    place: result_temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::Deref { expr, ty } => {
                let (stmts, _operand) = self.lower_expr_to_operand(expr);
                let result_temp = self.fresh_temp(convert_hir_type(ty));
                (stmts, MirOperand {
                    place: result_temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::Ref { expr, mutable: _, ty } => {
                let (stmts, _operand) = self.lower_expr_to_operand(expr);
                let result_temp = self.fresh_temp(convert_hir_type(ty));
                (stmts, MirOperand {
                    place: result_temp,
                    ty: convert_hir_type(ty),
                })
            }
            HirExpr::Param { name, index: _, ty } => {
                let place = MirPlace::Var(name.clone());
                (Vec::new(), MirOperand {
                    place,
                    ty: convert_hir_type(ty),
                })
            }
        }
    }

    /// Lower an HIR expression to a place (lvalue).
    fn lower_expr_to_place(&mut self, expr: &HirExpr) -> (Vec<MirStmt>, MirPlace) {
        match expr {
            HirExpr::Ident { name, .. } => {
                if let Some(place) = self.var_map.get(name) {
                    (Vec::new(), place.clone())
                } else {
                    (Vec::new(), MirPlace::Var(name.clone()))
                }
            }
            HirExpr::FieldAccess { expr, field, .. } => {
                let (stmts, base_place) = self.lower_expr_to_place(expr);
                let field_place = MirPlace::Field(Box::new(base_place), field.clone());
                (stmts, field_place)
            }
            HirExpr::Index { expr, index, .. } => {
                let (mut stmts, base_place) = self.lower_expr_to_place(expr);
                let (idx_stmts, idx_op) = self.lower_expr_to_operand(index);
                stmts.extend(idx_stmts);
                let temp = self.fresh_temp(MirType::I64);
                stmts.push(MirStmt::Assign {
                    dst: temp.clone(),
                    src: MirRvalue::BinaryOp(MirBinOp::Add, Box::new(MirOperand {
                        place: base_place,
                        ty: MirType::I64,
                    }), Box::new(idx_op)),
                });
                (stmts, temp)
            }
            HirExpr::Deref { expr, .. } => {
                let (stmts, operand) = self.lower_expr_to_operand(expr);
                (stmts, MirPlace::Deref(Box::new(operand.place)))
            }
            _ => {
                let (stmts, operand) = self.lower_expr_to_operand(expr);
                (stmts, operand.place)
            }
        }
    }

    fn fresh_temp(&mut self, _ty: MirType) -> MirPlace {
        let temp = MirPlace::Temp(self.temp_counter);
        self.temp_counter += 1;
        temp
    }

    fn fresh_bb_name(&mut self, prefix: &str) -> String {
        let name = format!("{}_{}", prefix, self.block_counter);
        self.block_counter += 1;
        name
    }

    /// Compute predecessor and successor relationships for all basic blocks.
    fn compute_cfg(&self, blocks: &mut Vec<MirBasicBlock>) {
        let mut preds: HashMap<String, Vec<String>> = HashMap::new();

        for bb in blocks.iter() {
            for succ in &bb.successors {
                preds.entry(succ.clone()).or_default().push(bb.name.clone());
            }
        }

        for bb in blocks.iter_mut() {
            bb.predecessors = preds.get(&bb.name).cloned().unwrap_or_default();
        }
    }
}

/// Convert HIR type to MIR type.
pub fn convert_hir_type(ty: &HirType) -> MirType {
    match ty {
        HirType::I8 => MirType::I8,
        HirType::I16 => MirType::I16,
        HirType::I32 => MirType::I32,
        HirType::I64 => MirType::I64,
        HirType::U8 => MirType::U8,
        HirType::U16 => MirType::U16,
        HirType::U32 => MirType::U32,
        HirType::U64 => MirType::U64,
        HirType::F32 => MirType::F32,
        HirType::F64 => MirType::F64,
        HirType::Bool => MirType::Bool,
        HirType::Void => MirType::Void,
        HirType::Ptr(inner) => MirType::Ptr(Box::new(convert_hir_type(inner))),
        HirType::Array(inner, len) => MirType::Array(Box::new(convert_hir_type(inner)), *len),
        HirType::Struct(name) => MirType::Struct(name.clone()),
        HirType::Fn(_, _) => MirType::I64, // Function pointers as I64
    }
}

/// Convert HIR binary operator to MIR binary operator.
pub fn convert_binop(op: &HirBinOp) -> MirBinOp {
    match op {
        HirBinOp::Add => MirBinOp::Add,
        HirBinOp::Sub => MirBinOp::Sub,
        HirBinOp::Mul => MirBinOp::Mul,
        HirBinOp::Div => MirBinOp::Div,
        HirBinOp::Rem => MirBinOp::Rem,
        HirBinOp::And => MirBinOp::And,
        HirBinOp::Or => MirBinOp::Or,
        HirBinOp::Xor => MirBinOp::Xor,
        HirBinOp::Shl => MirBinOp::Shl,
        HirBinOp::Shr => MirBinOp::Shr,
        HirBinOp::Eq => MirBinOp::Eq,
        HirBinOp::Ne => MirBinOp::Ne,
        HirBinOp::Lt => MirBinOp::Lt,
        HirBinOp::Le => MirBinOp::Le,
        HirBinOp::Gt => MirBinOp::Gt,
        HirBinOp::Ge => MirBinOp::Ge,
    }
}