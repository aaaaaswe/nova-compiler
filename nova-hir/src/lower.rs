/// AST → HIR lowering.
///
/// Converts the parsed AST into a typed, resolved HIR.
/// Handles name resolution, type conversion, and scope management.
use std::collections::HashMap;

use nova_frontend::ast::*;
use crate::error::TypeError;
use crate::hir::*;

/// Symbol table for name resolution.
#[derive(Debug, Clone)]
pub struct SymbolTable {
    /// Variables in the current scope: name → (mutable, HirType)
    variables: HashMap<String, (bool, HirType)>,
    /// Functions in scope: name → (param_types, return_type, CallConv, is_pub)
    functions: HashMap<String, (Vec<HirType>, HirType, CallConv, bool)>,
    /// Structs in scope: name → (fields: Vec<(name, type)>, is_pub)
    structs: HashMap<String, (Vec<(String, HirType)>, bool)>,
    /// Enums in scope: name → (variants: Vec<(name, Option<HirType>)>, is_pub)
    enums: HashMap<String, (Vec<(String, Option<HirType>)>, bool)>,
    /// Whether we are inside a loop (for break/continue validation)
    loop_depth: usize,
    /// Parent scope
    parent: Option<Box<SymbolTable>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        SymbolTable {
            variables: HashMap::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            loop_depth: 0,
            parent: None,
        }
    }

    /// Create a child scope (for blocks).
    pub fn child(&self) -> Self {
        SymbolTable {
            variables: HashMap::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            loop_depth: self.loop_depth,
            parent: Some(Box::new(self.clone_without_vars())),
        }
    }

    fn clone_without_vars(&self) -> Self {
        SymbolTable {
            variables: self.variables.clone(),
            functions: self.functions.clone(),
            structs: self.structs.clone(),
            enums: self.enums.clone(),
            loop_depth: self.loop_depth,
            parent: self.parent.clone(),
        }
    }

    pub fn add_var(&mut self, name: String, mutable: bool, ty: HirType) {
        self.variables.insert(name, (mutable, ty));
    }

    pub fn lookup_var(&self, name: &str) -> Option<(bool, HirType)> {
        if let Some(v) = self.variables.get(name) {
            return Some(v.clone());
        }
        if let Some(ref parent) = self.parent {
            return parent.lookup_var(name);
        }
        None
    }

    pub fn add_function(&mut self, name: String, params: Vec<HirType>, ret: HirType, cc: CallConv, is_pub: bool) {
        self.functions.insert(name, (params, ret, cc, is_pub));
    }

    pub fn lookup_function(&self, name: &str) -> Option<(Vec<HirType>, HirType, CallConv, bool)> {
        if let Some(f) = self.functions.get(name) {
            return Some(f.clone());
        }
        if let Some(ref parent) = self.parent {
            return parent.lookup_function(name);
        }
        None
    }

    pub fn add_struct(&mut self, name: String, fields: Vec<(String, HirType)>, is_pub: bool) {
        self.structs.insert(name, (fields, is_pub));
    }

    pub fn lookup_struct(&self, name: &str) -> Option<(Vec<(String, HirType)>, bool)> {
        if let Some(s) = self.structs.get(name) {
            return Some(s.clone());
        }
        if let Some(ref parent) = self.parent {
            return parent.lookup_struct(name);
        }
        None
    }

    pub fn add_enum(&mut self, name: String, variants: Vec<(String, Option<HirType>)>, is_pub: bool) {
        self.enums.insert(name, (variants, is_pub));
    }

    pub fn enter_loop(&mut self) {
        self.loop_depth += 1;
    }

    pub fn exit_loop(&mut self) {
        self.loop_depth = self.loop_depth.saturating_sub(1);
    }

    pub fn in_loop(&self) -> bool {
        self.loop_depth > 0
    }
}

/// Context for AST → HIR lowering.
pub struct Lowerer {
    /// Global symbol table.
    pub symbols: SymbolTable,
    /// Collected errors.
    errors: Vec<TypeError>,
}

impl Lowerer {
    pub fn new() -> Self {
        Lowerer {
            symbols: SymbolTable::new(),
            errors: Vec::new(),
        }
    }

    pub fn lower(&mut self, program: &Program) -> Result<HirProgram, Vec<TypeError>> {
        let mut items = Vec::new();

        // First pass: collect all type and function declarations
        for item in &program.items {
            self.collect_declaration(item);
        }

        // Second pass: lower all items
        for item in &program.items {
            if let Some(hir_item) = self.lower_item(item) {
                items.push(hir_item);
            }
        }

        if !self.errors.is_empty() {
            return Err(self.errors.clone());
        }

        Ok(HirProgram { items })
    }

    /// Collect function and type declarations for forward reference support.
    fn collect_declaration(&mut self, item: &Item) {
        match item {
            Item::Function(func) => {
                let params: Vec<HirType> = func.params.iter().map(|p| convert_type(&p.ty)).collect();
                let ret = func.return_type.as_ref().map(|t| convert_type(t)).unwrap_or(HirType::Void);
                let cc = CallConv::Nova;
                self.symbols.add_function(func.name.clone(), params, ret, cc, func.vis == Visibility::Public);
            }
            Item::Struct(s) => {
                let fields: Vec<(String, HirType)> = s.fields.iter()
                    .map(|f| (f.name.clone(), convert_type(&f.ty)))
                    .collect();
                self.symbols.add_struct(s.name.clone(), fields, s.vis == Visibility::Public);
            }
            Item::Enum(e) => {
                let variants: Vec<(String, Option<HirType>)> = e.variants.iter()
                    .map(|v| (v.name.clone(), v.data.as_ref().map(|t| convert_type(t))))
                    .collect();
                self.symbols.add_enum(e.name.clone(), variants, e.vis == Visibility::Public);
            }
            Item::Union(u) => {
                let fields: Vec<(String, HirType)> = u.fields.iter()
                    .map(|f| (f.name.clone(), convert_type(&f.ty)))
                    .collect();
                // Store unions as structs for simplicity
                self.symbols.add_struct(u.name.clone(), fields, u.vis == Visibility::Public);
            }
            _ => {}
        }
    }

    fn lower_item(&mut self, item: &Item) -> Option<HirItem> {
        match item {
            Item::Function(func) => {
                let mut scope = self.symbols.child();
                let params: Vec<HirParam> = func.params.iter()
                    .enumerate()
                    .map(|(_i, p)| {
                        let ty = convert_type(&p.ty);
                        scope.add_var(p.name.clone(), false, ty.clone());
                        HirParam { name: p.name.clone(), ty }
                    })
                    .collect();
                let ret = func.return_type.as_ref().map(|t| convert_type(t)).unwrap_or(HirType::Void);
                let body = match &func.body {
                    Some(b) => self.lower_block_into(b, &mut scope),
                    None => HirBlock { stmts: vec![], expr: None },
                };
                let cc = CallConv::Nova;
                Some(HirItem::Function(HirFunction {
                    name: func.name.clone(),
                    params,
                    return_type: ret,
                    body,
                    call_conv: cc,
                    is_pub: func.vis == Visibility::Public,
                }))
            }
            Item::Struct(s) => {
                let fields: Vec<(String, HirType)> = s.fields.iter()
                    .map(|f| (f.name.clone(), convert_type(&f.ty)))
                    .collect();
                Some(HirItem::Struct(HirStruct {
                    name: s.name.clone(),
                    fields,
                    is_pub: s.vis == Visibility::Public,
                }))
            }
            Item::Enum(e) => {
                let variants: Vec<(String, Option<HirType>)> = e.variants.iter()
                    .map(|v| (v.name.clone(), v.data.as_ref().map(|t| convert_type(t))))
                    .collect();
                Some(HirItem::Enum(HirEnum {
                    name: e.name.clone(),
                    variants,
                    is_pub: e.vis == Visibility::Public,
                }))
            }
            Item::Union(u) => {
                let fields: Vec<(String, HirType)> = u.fields.iter()
                    .map(|f| (f.name.clone(), convert_type(&f.ty)))
                    .collect();
                Some(HirItem::Union(HirUnion {
                    name: u.name.clone(),
                    fields,
                    is_pub: u.vis == Visibility::Public,
                }))
            }
            Item::ExternBlock(eb) => {
                let functions: Vec<HirFunction> = eb.items.iter().filter_map(|item| {
                    if let Item::Function(func) = item {
                        let params: Vec<HirParam> = func.params.iter()
                            .map(|p| {
                                let ty = convert_type(&p.ty);
                                HirParam { name: p.name.clone(), ty }
                            })
                            .collect();
                        let ret = func.return_type.as_ref().map(|t| convert_type(t)).unwrap_or(HirType::Void);
                        let cc = match eb.abi.as_deref() {
                            Some("C") | Some("c") => CallConv::C,
                            _ => CallConv::Nova,
                        };
                        Some(HirFunction {
                            name: func.name.clone(),
                            params,
                            return_type: ret,
                            body: HirBlock { stmts: vec![], expr: None },
                            call_conv: cc,
                            is_pub: true,
                        })
                    } else {
                        None
                    }
                }).collect();
                Some(HirItem::ExternBlock(HirExternBlock {
                    abi: eb.abi.clone().unwrap_or_else(|| "nova".to_string()),
                    functions,
                }))
            }
            _ => None, // Skip impl, mod, use for now
        }
    }

    fn lower_block_into(&mut self, block: &Block, scope: &mut SymbolTable) -> HirBlock {
        let mut stmts = Vec::new();
        let mut expr = None;

        let len = block.statements.len();
        for (i, stmt) in block.statements.iter().enumerate() {
            if i == len - 1 {
                // Last statement - if it's an expression (but not an assignment),
                // it becomes the tail expression
                if let Statement::Expr(e) = stmt {
                    // Don't treat assignments as tail expressions
                    let expr_ref: &Expr = e;
                    let is_assign = matches!(expr_ref, Expr::Assign { .. });
                    if !is_assign {
                        let hir_expr = self.lower_expr(e, scope);
                        expr = Some(hir_expr);
                        continue;
                    }
                }
            }
            let lowered = self.lower_stmt(stmt, scope);
            if let Some(s) = lowered {
                stmts.push(s);
            }
        }

        HirBlock { stmts, expr }
    }

    fn lower_stmt(&mut self, stmt: &Statement, scope: &mut SymbolTable) -> Option<HirStmt> {
        match stmt {
            Statement::Let { mutable, name, ty, init } => {
                let ann_ty = ty.as_ref().map(|t| convert_type(t));
                let init_expr = init.as_ref().map(|e| self.lower_expr(e, scope));
                let inferred_ty = if let Some(ref ann) = ann_ty {
                    ann.clone()
                } else if let Some(ref init_expr) = init_expr {
                    init_expr.ty().clone()
                } else {
                    self.errors.push(TypeError::Other(format!(
                        "cannot infer type for `{}` without type annotation or initializer", name
                    )));
                    HirType::I64
                };
                if let Some(ref ann) = ann_ty {
                    if let Some(ref init_expr) = init_expr {
                        if *init_expr.ty() != *ann {
                            if !self.is_coercible(init_expr.ty(), ann) {
                                self.errors.push(TypeError::TypeMismatch {
                                    expected: ann.to_string(),
                                    found: init_expr.ty().to_string(),
                                    span: None,
                                });
                            }
                        }
                    }
                }
                scope.add_var(name.clone(), *mutable, inferred_ty.clone());
                Some(HirStmt::Let {
                    name: name.clone(),
                    mutable: *mutable,
                    ty: inferred_ty,
                    init: init_expr,
                })
            }
            Statement::Expr(e) => {
                // Handle assignment expressions specially
                if let Expr::Assign { target, op, value } = e {
                    let hir_value = self.lower_expr(value, scope);
                    let hir_target = self.lower_expr(target, scope);
                    if let Some(bin_op) = op {
                        // Compound assignment: x += y → x = x + y
                        let hir_op = convert_binop(bin_op);
                        let result_ty = self.binary_result_type(&hir_target, &hir_op, &hir_value);
                        let combined = HirExpr::Binary {
                            left: Box::new(hir_target.clone()),
                            op: hir_op,
                            right: Box::new(hir_value),
                            ty: result_ty,
                        };
                        Some(HirStmt::Assign {
                            target: hir_target,
                            value: combined,
                        })
                    } else {
                        // Simple assignment: x = y
                        Some(HirStmt::Assign {
                            target: hir_target,
                            value: hir_value,
                        })
                    }
                } else {
                    let hir_expr = self.lower_expr(e, scope);
                    Some(HirStmt::Expr(hir_expr))
                }
            }
            Statement::Return(e) => {
                let hir_expr = e.as_ref().map(|e| self.lower_expr(e, scope));
                Some(HirStmt::Return(hir_expr))
            }
            Statement::If { cond, then_branch, else_branch } => {
                let hir_cond = self.lower_expr(cond, scope);
                if *hir_cond.ty() != HirType::Bool {
                    self.errors.push(TypeError::TypeMismatch {
                        expected: "bool".to_string(),
                        found: hir_cond.ty().to_string(),
                        span: None,
                    });
                }
                let mut then_scope = scope.child();
                let then_block = self.lower_block_into(then_branch, &mut then_scope);
                let else_block = else_branch.as_ref().map(|else_stmt| {
                    let mut else_scope = scope.child();
                    match else_stmt.as_ref() {
                        Statement::If { .. } => {
                            // else-if chain: wrap in a block
                            let lowered = self.lower_stmt(else_stmt, &mut else_scope);
                            HirBlock {
                                stmts: lowered.into_iter().collect(),
                                expr: None,
                            }
                        }
                        _ => {
                            let block = Block { statements: vec![(**else_stmt).clone()] };
                            self.lower_block_into(&block, &mut else_scope)
                        }
                    }
                });
                Some(HirStmt::If {
                    cond: hir_cond,
                    then_block,
                    else_block,
                })
            }
            Statement::While { cond, body } => {
                let hir_cond = self.lower_expr(cond, scope);
                if *hir_cond.ty() != HirType::Bool {
                    self.errors.push(TypeError::TypeMismatch {
                        expected: "bool".to_string(),
                        found: hir_cond.ty().to_string(),
                        span: None,
                    });
                }
                scope.enter_loop();
                let mut body_scope = scope.child();
                let hir_body = self.lower_block_into(body, &mut body_scope);
                scope.exit_loop();
                Some(HirStmt::While {
                    cond: hir_cond,
                    body: hir_body,
                })
            }
            Statement::For { var, iter, body } => {
                let hir_iter = self.lower_for_iter(iter, scope);
                scope.enter_loop();
                let mut body_scope = scope.child();
                let iter_ty = self.for_iter_type(&hir_iter);
                body_scope.add_var(var.clone(), false, iter_ty);
                let hir_body = self.lower_block_into(body, &mut body_scope);
                scope.exit_loop();
                Some(HirStmt::For {
                    var: var.clone(),
                    iter: hir_iter,
                    body: hir_body,
                })
            }
            Statement::Loop { body } => {
                scope.enter_loop();
                let mut body_scope = scope.child();
                let hir_body = self.lower_block_into(body, &mut body_scope);
                scope.exit_loop();
                Some(HirStmt::Loop { body: hir_body })
            }
            Statement::Break => {
                if !scope.in_loop() {
                    self.errors.push(TypeError::BreakOutsideLoop { span: None });
                }
                Some(HirStmt::Break)
            }
            Statement::Continue => {
                if !scope.in_loop() {
                    self.errors.push(TypeError::ContinueOutsideLoop { span: None });
                }
                Some(HirStmt::Continue)
            }
            Statement::Unsafe(block) => {
                let mut unsafe_scope = scope.child();
                let hir_block = self.lower_block_into(block, &mut unsafe_scope);
                Some(HirStmt::Unsafe(hir_block))
            }
            Statement::Asm(s) => {
                Some(HirStmt::Asm(s.clone()))
            }
            Statement::Defer(_) => {
                // Defer is not implemented in HIR yet
                None
            }
        }
    }

    fn lower_for_iter(&mut self, iter: &Expr, scope: &mut SymbolTable) -> HirForIter {
        match iter {
            Expr::Range { start, end, inclusive } => {
                let start_expr = start.as_ref()
                    .map(|e| self.lower_expr(e, scope))
                    .unwrap_or_else(|| HirExpr::IntLiteral { value: 0, ty: HirType::I64 });
                let end_expr = end.as_ref()
                    .map(|e| self.lower_expr(e, scope))
                    .unwrap_or_else(|| {
                        self.errors.push(TypeError::Other("range must have an end bound".to_string()));
                        HirExpr::IntLiteral { value: 0, ty: HirType::I64 }
                    });
                HirForIter::Range {
                    start: start_expr,
                    end: end_expr,
                    inclusive: *inclusive,
                }
            }
            other => {
                let expr = self.lower_expr(other, scope);
                HirForIter::Array(expr)
            }
        }
    }

    fn for_iter_type(&self, iter: &HirForIter) -> HirType {
        match iter {
            HirForIter::Range { start, .. } => start.ty().clone(),
            HirForIter::Array(expr) => {
                match expr.ty() {
                    HirType::Array(inner, _) => *inner.clone(),
                    HirType::Ptr(inner) => *inner.clone(),
                    _ => HirType::I64,
                }
            }
        }
    }

    fn lower_expr(&mut self, expr: &Expr, scope: &mut SymbolTable) -> HirExpr {
        match expr {
            Expr::Ident(name) => {
                if let Some((_, ty)) = scope.lookup_var(name) {
                    HirExpr::Ident { name: name.clone(), ty }
                } else if let Some((params, ret, _, _)) = scope.lookup_function(name) {
                    HirExpr::Ident {
                        name: name.clone(),
                        ty: HirType::Fn(params, Box::new(ret)),
                    }
                } else {
                    self.errors.push(TypeError::UndefinedVariable {
                        name: name.clone(),
                        span: None,
                    });
                    HirExpr::Ident { name: name.clone(), ty: HirType::I64 }
                }
            }
            Expr::IntLiteral(v) => {
                HirExpr::IntLiteral { value: *v, ty: HirType::I64 }
            }
            Expr::FloatLiteral(v) => {
                HirExpr::FloatLiteral { value: *v, ty: HirType::F64 }
            }
            Expr::BoolLiteral(v) => {
                HirExpr::BoolLiteral { value: *v, ty: HirType::Bool }
            }
            Expr::StringLiteral(s) => {
                let len = s.len();
                HirExpr::Ref {
                    expr: Box::new(HirExpr::IntLiteral { value: 0, ty: HirType::U8 }),
                    mutable: false,
                    ty: HirType::Ptr(Box::new(HirType::Array(Box::new(HirType::U8), len))),
                }
            }
            Expr::Binary { left, op, right } => {
                let hir_left = self.lower_expr(left, scope);
                let hir_right = self.lower_expr(right, scope);
                let hir_op = convert_binop(op);
                let result_ty = self.binary_result_type(&hir_left, &hir_op, &hir_right);
                HirExpr::Binary {
                    left: Box::new(hir_left),
                    op: hir_op,
                    right: Box::new(hir_right),
                    ty: result_ty,
                }
            }
            Expr::Unary { op, expr } => {
                let hir_expr = self.lower_expr(expr, scope);
                let hir_op = convert_unary_op(op);
                let result_ty = self.unary_result_type(&hir_op, &hir_expr);
                HirExpr::Unary {
                    op: hir_op,
                    expr: Box::new(hir_expr),
                    ty: result_ty,
                }
            }
            Expr::Call { func, args } => {
                let hir_func = self.lower_expr(func, scope);
                let hir_args: Vec<HirExpr> = args.iter()
                    .map(|a| self.lower_expr(a, scope))
                    .collect();
                let return_ty = match &hir_func.ty() {
                    HirType::Fn(params, ret) => {
                        if params.len() != hir_args.len() {
                            self.errors.push(TypeError::WrongArgCount {
                                expected: params.len(),
                                found: hir_args.len(),
                                span: None,
                            });
                        } else {
                            for (_i, (param_ty, arg)) in params.iter().zip(&hir_args).enumerate() {
                                if *arg.ty() != *param_ty {
                                    self.errors.push(TypeError::TypeMismatch {
                                        expected: param_ty.to_string(),
                                        found: arg.ty().to_string(),
                                        span: None,
                                    });
                                }
                            }
                        }
                        *ret.clone()
                    }
                    _ => {
                        self.errors.push(TypeError::TypeMismatch {
                            expected: "function".to_string(),
                            found: hir_func.ty().to_string(),
                            span: None,
                        });
                        HirType::I64
                    }
                };
                HirExpr::Call {
                    func: Box::new(hir_func),
                    args: hir_args,
                    ty: return_ty,
                }
            }
            Expr::FieldAccess { expr, field } => {
                let hir_expr = self.lower_expr(expr, scope);
                let field_ty = self.resolve_field_access(hir_expr.ty(), field);
                HirExpr::FieldAccess {
                    expr: Box::new(hir_expr),
                    field: field.clone(),
                    ty: field_ty,
                }
            }
            Expr::Index { expr, index } => {
                let hir_expr = self.lower_expr(expr, scope);
                let hir_index = self.lower_expr(index, scope);
                let elem_ty = match hir_expr.ty() {
                    HirType::Array(inner, _) => *inner.clone(),
                    HirType::Ptr(inner) => *inner.clone(),
                    other => {
                        self.errors.push(TypeError::InvalidIndex {
                            ty: other.to_string(),
                            span: None,
                        });
                        HirType::I64
                    }
                };
                HirExpr::Index {
                    expr: Box::new(hir_expr),
                    index: Box::new(hir_index),
                    ty: elem_ty,
                }
            }
            Expr::Cast { expr, ty } => {
                let hir_expr = self.lower_expr(expr, scope);
                let target_ty = convert_type(ty);
                HirExpr::Cast {
                    expr: Box::new(hir_expr),
                    ty: target_ty,
                }
            }
            Expr::Block(block) => {
                let mut block_scope = scope.child();
                let hir_block = self.lower_block_into(block, &mut block_scope);
                if let Some(ref expr) = hir_block.expr {
                    HirExpr::Ident { name: "_block_expr".to_string(), ty: expr.ty().clone() }
                } else {
                    HirExpr::IntLiteral { value: 0, ty: HirType::Void }
                }
            }
            Expr::IfExpr { cond, then_branch, else_branch } => {
                let _hir_cond = self.lower_expr(cond, scope);
                let mut then_scope = scope.child();
                let then_block = self.lower_block_into(then_branch, &mut then_scope);
                let then_ty = then_block.expr.as_ref().map(|e| e.ty().clone()).unwrap_or(HirType::Void);
                let else_ty = else_branch.as_ref().and_then(|else_expr| {
                    let mut else_scope = scope.child();
                    let else_e = self.lower_expr(else_expr, &mut else_scope);
                    Some(else_e.ty().clone())
                });
                // Result type is the common type (or Void)
                let result_ty = if let Some(ref et) = else_ty {
                    if then_ty == *et { then_ty } else { HirType::Void }
                } else {
                    HirType::Void
                };
                HirExpr::IntLiteral { value: 0, ty: result_ty }
            }
            Expr::StructLit { name, fields } => {
                let hir_fields: Vec<(String, HirExpr)> = fields.iter()
                    .map(|(fname, fexpr)| {
                        (fname.clone(), self.lower_expr(fexpr, scope))
                    })
                    .collect();
                let ty = HirType::Struct(name.clone());
                HirExpr::StructLit {
                    name: name.clone(),
                    fields: hir_fields,
                    ty,
                }
            }
            Expr::ArrayLit(elements) => {
                let hir_elems: Vec<HirExpr> = elements.iter()
                    .map(|e| self.lower_expr(e, scope))
                    .collect();
                let elem_ty = if let Some(first) = hir_elems.first() {
                    first.ty().clone()
                } else {
                    HirType::I64
                };
                let len = hir_elems.len();
                // TODO: properly lower array literals
                HirExpr::IntLiteral { value: 0, ty: HirType::Array(Box::new(elem_ty), len) }
            }
            Expr::Range { start, end: _, inclusive: _ } => {
                let start_expr = start.as_ref()
                    .map(|e| self.lower_expr(e, scope))
                    .unwrap_or_else(|| HirExpr::IntLiteral { value: 0, ty: HirType::I64 });
                HirExpr::IntLiteral { value: 0, ty: start_expr.ty().clone() }
            }
            Expr::Ref { mutable, expr } => {
                let hir_expr = self.lower_expr(expr, scope);
                let inner_ty = hir_expr.ty().clone();
                HirExpr::Ref {
                    expr: Box::new(hir_expr),
                    mutable: *mutable,
                    ty: HirType::Ptr(Box::new(inner_ty)),
                }
            }
            Expr::Deref(expr) => {
                let hir_expr = self.lower_expr(expr, scope);
                let inner_ty = match hir_expr.ty() {
                    HirType::Ptr(inner) => *inner.clone(),
                    other => {
                        self.errors.push(TypeError::InvalidDeref {
                            ty: other.to_string(),
                            span: None,
                        });
                        HirType::I64
                    }
                };
                HirExpr::Deref {
                    expr: Box::new(hir_expr),
                    ty: inner_ty,
                }
            }
            Expr::Self_ => {
                HirExpr::Ident { name: "self".to_string(), ty: HirType::I64 }
            }
            Expr::Assign { target, op, value } => {
                let hir_value = self.lower_expr(value, scope);
                let hir_target = self.lower_expr(target, scope);
                if let Some(bin_op) = op {
                    let hir_op = convert_binop(bin_op);
                    let result_ty = self.binary_result_type(&hir_target, &hir_op, &hir_value);
                    HirExpr::Binary {
                        left: Box::new(hir_target),
                        op: hir_op,
                        right: Box::new(hir_value),
                        ty: result_ty,
                    }
                } else {
                    HirExpr::Ident { name: "_assign".to_string(), ty: HirType::Void }
                }
            }
            Expr::Sizeof(_) => {
                HirExpr::IntLiteral { value: 8, ty: HirType::I64 }
            }
            Expr::Alignof(_) => {
                HirExpr::IntLiteral { value: 8, ty: HirType::I64 }
            }
        }
    }

    fn binary_result_type(&self, left: &HirExpr, op: &HirBinOp, right: &HirExpr) -> HirType {
        let lt = left.ty();
        let rt = right.ty();

        match op {
            HirBinOp::Eq | HirBinOp::Ne | HirBinOp::Lt | HirBinOp::Le | HirBinOp::Gt | HirBinOp::Ge => {
                HirType::Bool
            }
            _ => {
                if lt == rt {
                    lt.clone()
                } else if self.is_numeric(lt) && self.is_numeric(rt) {
                    // Coerce to wider type
                    if self.type_width(lt) >= self.type_width(rt) {
                        lt.clone()
                    } else {
                        rt.clone()
                    }
                } else {
                    lt.clone()
                }
            }
        }
    }

    fn unary_result_type(&self, op: &HirUnaryOp, expr: &HirExpr) -> HirType {
        match op {
            HirUnaryOp::Neg => expr.ty().clone(),
            HirUnaryOp::Not => HirType::Bool,
            HirUnaryOp::Deref => {
                match expr.ty() {
                    HirType::Ptr(inner) => *inner.clone(),
                    _ => HirType::I64,
                }
            }
            HirUnaryOp::Ref | HirUnaryOp::RefMut => {
                HirType::Ptr(Box::new(expr.ty().clone()))
            }
        }
    }

    fn resolve_field_access(&mut self, ty: &HirType, field: &str) -> HirType {
        match ty {
            HirType::Struct(name) => {
                if let Some((fields, _)) = self.symbols.lookup_struct(name) {
                    for (fname, fty) in &fields {
                        if fname == field {
                            return fty.clone();
                        }
                    }
                }
                self.errors.push(TypeError::InvalidFieldAccess {
                    field: field.to_string(),
                    ty: ty.to_string(),
                    span: None,
                });
                HirType::I64
            }
            HirType::Ptr(inner) => {
                self.resolve_field_access(inner, field)
            }
            _ => {
                HirType::I64
            }
        }
    }

    fn is_numeric(&self, ty: &HirType) -> bool {
        matches!(ty, HirType::I8 | HirType::I16 | HirType::I32 | HirType::I64
            | HirType::U8 | HirType::U16 | HirType::U32 | HirType::U64
            | HirType::F32 | HirType::F64)
    }

    fn type_width(&self, ty: &HirType) -> usize {
        match ty {
            HirType::I8 | HirType::U8 => 1,
            HirType::I16 | HirType::U16 => 2,
            HirType::I32 | HirType::U32 | HirType::F32 => 4,
            HirType::I64 | HirType::U64 | HirType::F64 => 8,
            _ => 8,
        }
    }

    fn is_coercible(&self, from: &HirType, to: &HirType) -> bool {
        if from == to {
            return true;
        }
        if self.is_numeric(from) && self.is_numeric(to) {
            return true;
        }
        false
    }
}

/// Convert AST type to HIR type.
pub fn convert_type(ty: &Type) -> HirType {
    match ty {
        Type::Named(name) => {
            match name.as_str() {
                "i8" => HirType::I8,
                "i16" => HirType::I16,
                "i32" => HirType::I32,
                "i64" => HirType::I64,
                "u8" => HirType::U8,
                "u16" => HirType::U16,
                "u32" => HirType::U32,
                "u64" => HirType::U64,
                "f32" => HirType::F32,
                "f64" => HirType::F64,
                "bool" => HirType::Bool,
                "void" => HirType::Void,
                _ => HirType::Struct(name.clone()),
            }
        }
        Type::Ptr(inner) => HirType::Ptr(Box::new(convert_type(inner))),
        Type::MutPtr(inner) => HirType::Ptr(Box::new(convert_type(inner))),
        Type::ConstPtr(inner) => HirType::Ptr(Box::new(convert_type(inner))),
        Type::Array(inner, len) => HirType::Array(Box::new(convert_type(inner)), *len),
        Type::Fn(params, ret) => {
            let hparams: Vec<HirType> = params.iter().map(|t| convert_type(t)).collect();
            let hret = ret.as_ref().map(|t| convert_type(t)).unwrap_or(HirType::Void);
            HirType::Fn(hparams, Box::new(hret))
        }
        Type::Generic(_) => HirType::I64, // Default generic to I64
    }
}

/// Convert AST binary operator to HIR binary operator.
pub fn convert_binop(op: &BinOp) -> HirBinOp {
    match op {
        BinOp::Add => HirBinOp::Add,
        BinOp::Sub => HirBinOp::Sub,
        BinOp::Mul => HirBinOp::Mul,
        BinOp::Div => HirBinOp::Div,
        BinOp::Rem => HirBinOp::Rem,
        BinOp::And => HirBinOp::And,
        BinOp::Or => HirBinOp::Or,
        BinOp::Xor => HirBinOp::Xor,
        BinOp::Shl => HirBinOp::Shl,
        BinOp::Shr => HirBinOp::Shr,
        BinOp::Eq => HirBinOp::Eq,
        BinOp::Ne => HirBinOp::Ne,
        BinOp::Lt => HirBinOp::Lt,
        BinOp::Le => HirBinOp::Le,
        BinOp::Gt => HirBinOp::Gt,
        BinOp::Ge => HirBinOp::Ge,
        BinOp::Assign => HirBinOp::Add, // fallback
    }
}

/// Convert AST unary operator to HIR unary operator.
pub fn convert_unary_op(op: &UnaryOp) -> HirUnaryOp {
    match op {
        UnaryOp::Neg => HirUnaryOp::Neg,
        UnaryOp::Not => HirUnaryOp::Not,
        UnaryOp::Deref => HirUnaryOp::Deref,
        UnaryOp::Ref => HirUnaryOp::Ref,
        UnaryOp::RefMut => HirUnaryOp::RefMut,
    }
}