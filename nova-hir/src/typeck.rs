/// HIR type checker.
///
/// Validates that all expressions have valid types, operations are compatible,
/// and control flow is well-typed.
use crate::error::TypeError;
use crate::hir::*;

/// Type check a complete HIR program.
pub fn type_check(program: &HirProgram) -> Result<(), Vec<TypeError>> {
    let mut checker = TypeChecker::new();
    checker.check_program(program)
}

struct TypeChecker {
    errors: Vec<TypeError>,
    /// Current function return type being checked
    current_return_type: Option<HirType>,
    /// Loop depth for break/continue validation
    loop_depth: usize,
}

impl TypeChecker {
    fn new() -> Self {
        TypeChecker {
            errors: Vec::new(),
            current_return_type: None,
            loop_depth: 0,
        }
    }

    fn check_program(&mut self, program: &HirProgram) -> Result<(), Vec<TypeError>> {
        for item in &program.items {
            self.check_item(item);
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    fn check_item(&mut self, item: &HirItem) {
        match item {
            HirItem::Function(func) => self.check_function(func),
            HirItem::Struct(_) | HirItem::Enum(_) | HirItem::Union(_) => {
                // Struct/enum/union definitions are always valid
            }
            HirItem::ExternBlock(eb) => {
                for _func in &eb.functions {
                    // Extern functions have no body, so they're trivially valid
                }
            }
        }
    }

    fn check_function(&mut self, func: &HirFunction) {
        self.current_return_type = Some(func.return_type.clone());
        self.check_block(&func.body);
        self.current_return_type = None;
    }

    fn check_block(&mut self, block: &HirBlock) {
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
        if let Some(ref expr) = block.expr {
            self.check_expr(expr);
        }
    }

    fn check_stmt(&mut self, stmt: &HirStmt) {
        match stmt {
            HirStmt::Let { init, ty, .. } => {
                if let Some(ref init_expr) = init {
                    self.check_expr(init_expr);
                    if init_expr.ty() != ty {
                        if !self.is_numeric_coercible(init_expr.ty(), ty) {
                            self.errors.push(TypeError::TypeMismatch {
                                expected: ty.to_string(),
                                found: init_expr.ty().to_string(),
                                span: None,
                            });
                        }
                    }
                }
            }
            HirStmt::Expr(expr) => {
                self.check_expr(expr);
            }
            HirStmt::Return(expr) => {
                if let Some(ref e) = expr {
                    self.check_expr(e);
                    if let Some(ref ret_ty) = self.current_return_type {
                        if *ret_ty != HirType::Void && e.ty() != ret_ty {
                            self.errors.push(TypeError::ReturnTypeMismatch {
                                expected: ret_ty.to_string(),
                                found: e.ty().to_string(),
                                span: None,
                            });
                        }
                    }
                } else if let Some(ref ret_ty) = self.current_return_type {
                    if *ret_ty != HirType::Void {
                        self.errors.push(TypeError::ReturnTypeMismatch {
                            expected: ret_ty.to_string(),
                            found: "void".to_string(),
                            span: None,
                        });
                    }
                }
            }
            HirStmt::If { cond, then_block, else_block } => {
                self.check_expr(cond);
                if *cond.ty() != HirType::Bool {
                    self.errors.push(TypeError::TypeMismatch {
                        expected: "bool".to_string(),
                        found: cond.ty().to_string(),
                        span: None,
                    });
                }
                self.check_block(then_block);
                if let Some(ref else_b) = else_block {
                    self.check_block(else_b);
                }
            }
            HirStmt::While { cond, body } => {
                self.check_expr(cond);
                if *cond.ty() != HirType::Bool {
                    self.errors.push(TypeError::TypeMismatch {
                        expected: "bool".to_string(),
                        found: cond.ty().to_string(),
                        span: None,
                    });
                }
                self.loop_depth += 1;
                self.check_block(body);
                self.loop_depth -= 1;
            }
            HirStmt::For { body, .. } => {
                self.loop_depth += 1;
                self.check_block(body);
                self.loop_depth -= 1;
            }
            HirStmt::Loop { body } => {
                self.loop_depth += 1;
                self.check_block(body);
                self.loop_depth -= 1;
            }
            HirStmt::Break => {
                if self.loop_depth == 0 {
                    self.errors.push(TypeError::BreakOutsideLoop { span: None });
                }
            }
            HirStmt::Continue => {
                if self.loop_depth == 0 {
                    self.errors.push(TypeError::ContinueOutsideLoop { span: None });
                }
            }
            HirStmt::Assign { target, value } => {
                self.check_expr(target);
                self.check_expr(value);
            }
            HirStmt::Unsafe(block) => {
                self.check_block(block);
            }
            HirStmt::Asm(_) => {
                // Inline assembly is always valid
            }
        }
    }

    fn check_expr(&mut self, expr: &HirExpr) {
        match expr {
            HirExpr::Ident { .. } => {}
            HirExpr::IntLiteral { .. } => {}
            HirExpr::FloatLiteral { .. } => {}
            HirExpr::BoolLiteral { .. } => {}
            HirExpr::Binary { left, op, right, .. } => {
                self.check_expr(left);
                self.check_expr(right);
                let lt = left.ty();
                let rt = right.ty();
                match op {
                    HirBinOp::Eq | HirBinOp::Ne | HirBinOp::Lt | HirBinOp::Le
                    | HirBinOp::Gt | HirBinOp::Ge => {
                        // Comparison operators need compatible types
                        if lt != rt {
                            self.errors.push(TypeError::InvalidBinaryOp {
                                op: op.to_string(),
                                left: lt.to_string(),
                                right: rt.to_string(),
                                span: None,
                            });
                        }
                    }
                    _ => {
                        if !self.is_numeric(lt) || !self.is_numeric(rt) {
                            self.errors.push(TypeError::InvalidBinaryOp {
                                op: op.to_string(),
                                left: lt.to_string(),
                                right: rt.to_string(),
                                span: None,
                            });
                        }
                    }
                }
            }
            HirExpr::Unary { op, expr, .. } => {
                self.check_expr(expr);
                let ty = expr.ty();
                match op {
                    HirUnaryOp::Neg => {
                        if !self.is_numeric(ty) {
                            self.errors.push(TypeError::InvalidUnaryOp {
                                op: op.to_string(),
                                ty: ty.to_string(),
                                span: None,
                            });
                        }
                    }
                    HirUnaryOp::Not => {
                        if *ty != HirType::Bool {
                            self.errors.push(TypeError::InvalidUnaryOp {
                                op: op.to_string(),
                                ty: ty.to_string(),
                                span: None,
                            });
                        }
                    }
                    HirUnaryOp::Deref => {
                        match ty {
                            HirType::Ptr(_) => {}
                            _ => {
                                self.errors.push(TypeError::InvalidDeref {
                                    ty: ty.to_string(),
                                    span: None,
                                });
                            }
                        }
                    }
                    HirUnaryOp::Ref | HirUnaryOp::RefMut => {}
                }
            }
            HirExpr::Call { func, args, ty: _ } => {
                self.check_expr(func);
                for arg in args {
                    self.check_expr(arg);
                }
            }
            HirExpr::FieldAccess { expr, .. } => {
                self.check_expr(expr);
            }
            HirExpr::Index { expr, index, .. } => {
                self.check_expr(expr);
                self.check_expr(index);
                if let HirType::Ptr(_) = expr.ty() {
                    // Valid
                } else if let HirType::Array(_, _) = expr.ty() {
                    // Valid
                } else {
                    self.errors.push(TypeError::InvalidIndex {
                        ty: expr.ty().to_string(),
                        span: None,
                    });
                }
            }
            HirExpr::Cast { expr, .. } => {
                self.check_expr(expr);
            }
            HirExpr::StructLit { fields, .. } => {
                for (_, field_expr) in fields {
                    self.check_expr(field_expr);
                }
            }
            HirExpr::Deref { expr, .. } => {
                self.check_expr(expr);
            }
            HirExpr::Ref { expr, .. } => {
                self.check_expr(expr);
            }
            HirExpr::Param { .. } => {}
        }
    }

    fn is_numeric(&self, ty: &HirType) -> bool {
        matches!(ty, HirType::I8 | HirType::I16 | HirType::I32 | HirType::I64
            | HirType::U8 | HirType::U16 | HirType::U32 | HirType::U64
            | HirType::F32 | HirType::F64)
    }

    fn is_numeric_coercible(&self, from: &HirType, to: &HirType) -> bool {
        if from == to {
            return true;
        }
        if self.is_numeric(from) && self.is_numeric(to) {
            return true;
        }
        false
    }
}