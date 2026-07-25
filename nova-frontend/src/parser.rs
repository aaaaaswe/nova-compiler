use std::ops::Range;

use crate::ast::*;
use crate::error::{ParseError, Span};
use crate::lexer::{tokenize, Token};

/// A recursive descent parser for the Nova language.
pub struct Parser<'a> {
    tokens: Vec<(Token, Range<usize>)>,
    source: &'a str,
    pos: usize,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    /// Create a new parser from source code.
    pub fn new(source: &'a str) -> Self {
        let tokens = tokenize(source);
        Parser {
            tokens,
            source,
            pos: 0,
            errors: Vec::new(),
        }
    }

    /// Parse the entire program.
    pub fn parse_program(&mut self) -> Program {
        let mut items = Vec::new();
        while !self.is_eof() {
            match self.parse_item() {
                Ok(item) => items.push(item),
                Err(err) => {
                    self.errors.push(err);
                    self.recover();
                }
            }
        }
        Program { items }
    }

    /// Return all collected errors.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// Return whether there were any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    // ── Helper methods ──

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn peek_n(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n).map(|(t, _)| t)
    }

    fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|(_, s)| Span {
                start: s.start,
                end: s.end,
            })
            .unwrap_or(Span {
                start: self.source.len(),
                end: self.source.len(),
            })
    }

    fn advance(&mut self) -> Option<(Token, Range<usize>)> {
        if self.is_eof() {
            return None;
        }
        let token = self.tokens[self.pos].clone();
        self.pos += 1;
        Some(token)
    }

    fn expect(&mut self, expected: Token) -> Result<(Token, Range<usize>), ParseError> {
        if self.is_eof() {
            return Err(ParseError::UnexpectedEof);
        }
        let (ref token, ref span) = self.tokens[self.pos];
        if std::mem::discriminant(token) == std::mem::discriminant(&expected) {
            Ok(self.advance().unwrap())
        } else {
            Err(ParseError::expected_token(
                &expected.name(),
                &token.name(),
                Span {
                    start: span.start,
                    end: span.end,
                },
                self.source,
            ))
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        if self.is_eof() {
            return Err(ParseError::UnexpectedEof);
        }
        match self.peek() {
            Some(Token::Ident(name)) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            Some(tok) => {
                let span = self.current_span();
                Err(ParseError::expected_token(
                    "identifier",
                    &tok.name(),
                    span,
                    self.source,
                ))
            }
            None => Err(ParseError::UnexpectedEof),
        }
    }

    /// Error recovery: skip tokens until we reach a semicolon or closing brace.
    fn recover(&mut self) {
        while !self.is_eof() {
            match self.peek() {
                Some(Token::Semicolon) | Some(Token::RBrace) => {
                    // Skip the semicolon but not the closing brace
                    if matches!(self.peek(), Some(Token::Semicolon)) {
                        self.advance();
                    }
                    return;
                }
                Some(_) => {
                    self.advance();
                }
                None => return,
            }
        }
    }

    /// Skip to the next semicolon or closing brace (for error recovery in statements).
    fn skip_to_stmt_end(&mut self) {
        while !self.is_eof() {
            match self.peek() {
                Some(Token::Semicolon) | Some(Token::RBrace) => {
                    return;
                }
                Some(_) => {
                    self.advance();
                }
                None => return,
            }
        }
    }

    // ── Item parsing ──

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        // Parse attributes
        let attrs = self.parse_attributes();

        // Parse visibility
        let vis = self.parse_visibility();

        match self.peek() {
            Some(Token::Fn) => {
                let func = self.parse_function(vis, attrs)?;
                Ok(Item::Function(func))
            }
            Some(Token::Struct) => {
                let sd = self.parse_struct_def(vis, attrs)?;
                Ok(Item::Struct(sd))
            }
            Some(Token::Enum) => {
                let ed = self.parse_enum_def(vis, attrs)?;
                Ok(Item::Enum(ed))
            }
            Some(Token::Union) => {
                let ud = self.parse_union_def(vis, attrs)?;
                Ok(Item::Union(ud))
            }
            Some(Token::Impl) => {
                let ib = self.parse_impl_block(vis, attrs)?;
                Ok(Item::Impl(ib))
            }
            Some(Token::Mod) => {
                let md = self.parse_mod_decl(vis, attrs)?;
                Ok(Item::Mod(md))
            }
            Some(Token::Use) => {
                let ud = self.parse_use_decl(vis, attrs)?;
                Ok(Item::Use(ud))
            }
            Some(Token::Extern) => {
                let eb = self.parse_extern_block(vis, attrs)?;
                Ok(Item::ExternBlock(eb))
            }
            Some(tok) => {
                let span = self.current_span();
                Err(ParseError::unexpected_token(
                    &tok.name(),
                    span,
                    self.source,
                ))
            }
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_visibility(&mut self) -> Visibility {
        if matches!(self.peek(), Some(Token::Pub)) {
            self.advance();
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    fn parse_attributes(&mut self) -> Vec<Attribute> {
        let mut attrs = Vec::new();
        while matches!(self.peek(), Some(Token::Hash)) {
            if let Ok(attr) = self.parse_attribute() {
                attrs.push(attr);
            } else {
                break;
            }
        }
        attrs
    }

    fn parse_attribute(&mut self) -> Result<Attribute, ParseError> {
        self.expect(Token::Hash)?;
        self.expect(Token::LBracket)?;
        let name = self.expect_ident()?;
        let mut args = Vec::new();
        // Parse optional arguments
        if matches!(self.peek(), Some(Token::LParen)) {
            self.advance(); // (
            while !matches!(self.peek(), Some(Token::RParen)) && !self.is_eof() {
                if let Some(Token::Ident(s)) = self.peek() {
                    args.push(s.clone());
                    self.advance();
                } else {
                    break;
                }
                if matches!(self.peek(), Some(Token::Comma)) {
                    self.advance();
                }
            }
            self.expect(Token::RParen)?;
        }
        self.expect(Token::RBracket)?;
        Ok(Attribute { name, args })
    }

    fn parse_generic_params(&mut self) -> Result<Vec<GenericParam>, ParseError> {
        let mut params = Vec::new();
        if matches!(self.peek(), Some(Token::Lt)) {
            self.advance();
            loop {
                let name = self.expect_ident()?;
                params.push(GenericParam { name });
                if matches!(self.peek(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::Gt)?;
        }
        Ok(params)
    }

    // ── Function ──

    fn parse_function(
        &mut self,
        vis: Visibility,
        attrs: Vec<Attribute>,
    ) -> Result<Function, ParseError> {
        self.expect(Token::Fn)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        self.expect(Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(Token::RParen)?;

        let return_type = if matches!(self.peek(), Some(Token::Arrow)) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = if matches!(self.peek(), Some(Token::LBrace)) {
            Some(self.parse_block()?)
        } else if matches!(self.peek(), Some(Token::Semicolon)) {
            self.advance();
            None
        } else {
            // For extern function declarations (no body, no semicolon in some cases)
            None
        };

        Ok(Function {
            vis,
            name,
            params,
            return_type,
            body,
            attrs,
            generics,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        if matches!(self.peek(), Some(Token::RParen)) {
            return Ok(params);
        }

        loop {
            // Check for variadic (...)
            if matches!(self.peek(), Some(Token::Ellipsis)) {
                self.advance();
                break;
            }

            // Check for self parameter
            if matches!(self.peek(), Some(Token::Self_)) {
                self.advance();
                let ty = Type::Named("Self".into());
                params.push(Param {
                    name: "self".into(),
                    ty,
                });
                if matches!(self.peek(), Some(Token::Comma)) {
                    self.advance();
                    continue;
                } else {
                    break;
                }
            }

            // Check for &self parameter
            if matches!(self.peek(), Some(Token::Amp)) && matches!(self.peek_n(1), Some(Token::Self_))
            {
                self.advance(); // &
                self.advance(); // self
                let ty = Type::Ptr(Box::new(Type::Named("Self".into())));
                params.push(Param {
                    name: "self".into(),
                    ty,
                });
                if matches!(self.peek(), Some(Token::Comma)) {
                    self.advance();
                    continue;
                } else {
                    break;
                }
            }

            let name = self.expect_ident()?;
            self.expect(Token::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param { name, ty });

            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(params)
    }

    // ── Struct ──

    fn parse_struct_def(
        &mut self,
        vis: Visibility,
        attrs: Vec<Attribute>,
    ) -> Result<StructDef, ParseError> {
        self.expect(Token::Struct)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        let fields = self.parse_field_list()?;
        Ok(StructDef {
            vis,
            name,
            fields,
            generics,
            attrs,
        })
    }

    fn parse_field_list(&mut self) -> Result<Vec<Field>, ParseError> {
        self.expect(Token::LBrace)?;
        let mut fields = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace)) && !self.is_eof() {
            let name = self.expect_ident()?;
            self.expect(Token::Colon)?;
            let ty = self.parse_type()?;
            fields.push(Field { name, ty });
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
                // Allow trailing comma
                if matches!(self.peek(), Some(Token::RBrace)) {
                    break;
                }
            } else if !matches!(self.peek(), Some(Token::RBrace)) {
                // Allow newline-separated fields without comma
                // Check if we're at an identifier (next field)
                if !matches!(self.peek(), Some(Token::Ident(_))) {
                    break;
                }
            }
        }
        self.expect(Token::RBrace)?;
        Ok(fields)
    }

    // ── Enum ──

    fn parse_enum_def(
        &mut self,
        vis: Visibility,
        attrs: Vec<Attribute>,
    ) -> Result<EnumDef, ParseError> {
        self.expect(Token::Enum)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        self.expect(Token::LBrace)?;
        let mut variants = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace)) && !self.is_eof() {
            let vname = self.expect_ident()?;
            let data = if matches!(self.peek(), Some(Token::LParen)) {
                self.advance();
                let ty = self.parse_type()?;
                self.expect(Token::RParen)?;
                Some(ty)
            } else {
                None
            };
            variants.push(EnumVariant { name: vname, data });
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
                if matches!(self.peek(), Some(Token::RBrace)) {
                    break;
                }
            }
        }
        self.expect(Token::RBrace)?;
        Ok(EnumDef {
            vis,
            name,
            variants,
            generics,
            attrs,
        })
    }

    // ── Union ──

    fn parse_union_def(
        &mut self,
        vis: Visibility,
        attrs: Vec<Attribute>,
    ) -> Result<UnionDef, ParseError> {
        self.expect(Token::Union)?;
        let name = self.expect_ident()?;
        let generics = self.parse_generic_params()?;
        let fields = self.parse_field_list()?;
        Ok(UnionDef {
            vis,
            name,
            fields,
            generics,
            attrs,
        })
    }

    // ── Impl ──

    fn parse_impl_block(
        &mut self,
        _vis: Visibility,
        _attrs: Vec<Attribute>,
    ) -> Result<ImplBlock, ParseError> {
        self.expect(Token::Impl)?;
        let generics = self.parse_generic_params()?;
        let target = self.parse_type()?;
        self.expect(Token::LBrace)?;
        let mut items = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace)) && !self.is_eof() {
            match self.parse_item() {
                Ok(item) => items.push(item),
                Err(err) => {
                    self.errors.push(err);
                    self.recover();
                }
            }
        }
        self.expect(Token::RBrace)?;
        Ok(ImplBlock {
            generics,
            target,
            items,
        })
    }

    // ── Mod ──

    fn parse_mod_decl(
        &mut self,
        vis: Visibility,
        attrs: Vec<Attribute>,
    ) -> Result<ModDecl, ParseError> {
        self.expect(Token::Mod)?;
        let name = self.expect_ident()?;
        self.expect(Token::Semicolon)?;
        Ok(ModDecl { vis, name, attrs })
    }

    // ── Use ──

    fn parse_use_decl(
        &mut self,
        vis: Visibility,
        attrs: Vec<Attribute>,
    ) -> Result<UseDecl, ParseError> {
        self.expect(Token::Use)?;
        let mut path = vec![self.expect_ident()?];
        while matches!(self.peek(), Some(Token::PathSep)) {
            self.advance();
            path.push(self.expect_ident()?);
        }
        self.expect(Token::Semicolon)?;
        Ok(UseDecl { vis, path, attrs })
    }

    // ── Extern ──

    fn parse_extern_block(
        &mut self,
        vis: Visibility,
        attrs: Vec<Attribute>,
    ) -> Result<ExternBlock, ParseError> {
        self.expect(Token::Extern)?;
        let abi = if let Some(Token::StringLiteral(s)) = self.peek() {
            let s = s.clone();
            self.advance();
            Some(s)
        } else {
            None
        };
        self.expect(Token::LBrace)?;
        let mut items = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace)) && !self.is_eof() {
            let item_attrs = self.parse_attributes();
            let item_vis = self.parse_visibility();
            match self.peek() {
                Some(Token::Fn) => {
                    let func = self.parse_function(item_vis, item_attrs)?;
                    items.push(Item::Function(func));
                }
                Some(tok) => {
                    let span = self.current_span();
                    return Err(ParseError::unexpected_token(
                        &tok.name(),
                        span,
                        self.source,
                    ));
                }
                None => break,
            }
        }
        self.expect(Token::RBrace)?;
        Ok(ExternBlock {
            vis,
            abi,
            items,
            attrs,
        })
    }

    // ── Statement parsing ──

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek() {
            Some(Token::Let) => self.parse_let_stmt(),
            Some(Token::Return) => self.parse_return_stmt(),
            Some(Token::If) => self.parse_if_stmt(),
            Some(Token::While) => self.parse_while_stmt(),
            Some(Token::For) => self.parse_for_stmt(),
            Some(Token::Loop) => self.parse_loop_stmt(),
            Some(Token::Break) => {
                self.advance();
                self.expect(Token::Semicolon)?;
                Ok(Statement::Break)
            }
            Some(Token::Continue) => {
                self.advance();
                self.expect(Token::Semicolon)?;
                Ok(Statement::Continue)
            }
            Some(Token::Unsafe) => self.parse_unsafe_stmt(),
            Some(Token::Asm) => self.parse_asm_stmt(),
            Some(Token::Defer) => self.parse_defer_stmt(),
            Some(_) => {
                let expr = self.parse_expression(0)?;
                self.expect(Token::Semicolon)?;
                Ok(Statement::Expr(expr))
            }
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        self.expect(Token::LBrace)?;
        let mut statements = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace)) && !self.is_eof() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(err) => {
                    self.errors.push(err);
                    self.skip_to_stmt_end();
                }
            }
        }
        self.expect(Token::RBrace)?;
        Ok(Block { statements })
    }

    fn parse_let_stmt(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::Let)?;
        let mutable = if matches!(self.peek(), Some(Token::Mut)) {
            self.advance();
            true
        } else {
            false
        };
        let name = self.expect_ident()?;
        let ty = if matches!(self.peek(), Some(Token::Colon)) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        let init = if matches!(self.peek(), Some(Token::Eq)) {
            self.advance();
            Some(self.parse_expression(0)?)
        } else {
            None
        };
        self.expect(Token::Semicolon)?;
        Ok(Statement::Let {
            mutable,
            name,
            ty,
            init,
        })
    }

    fn parse_return_stmt(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::Return)?;
        let expr = if matches!(self.peek(), Some(Token::Semicolon)) {
            None
        } else {
            Some(self.parse_expression(0)?)
        };
        self.expect(Token::Semicolon)?;
        Ok(Statement::Return(expr))
    }

    fn parse_if_stmt(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::If)?;
        let cond = self.parse_expression(0)?;
        let then_branch = self.parse_block()?;
        let else_branch = if matches!(self.peek(), Some(Token::Else)) {
            self.advance();
            if matches!(self.peek(), Some(Token::If)) {
                Some(Box::new(self.parse_if_stmt()?))
            } else {
                let block = self.parse_block()?;
                Some(Box::new(Statement::Expr(Expr::Block(block))))
            }
        } else {
            None
        };
        Ok(Statement::If {
            cond,
            then_branch,
            else_branch,
        })
    }

    fn parse_while_stmt(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::While)?;
        let cond = self.parse_expression(0)?;
        let body = self.parse_block()?;
        Ok(Statement::While { cond, body })
    }

    fn parse_for_stmt(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::For)?;
        let var = self.expect_ident()?;
        self.expect(Token::In)?;
        let iter = self.parse_expression(0)?;
        let body = self.parse_block()?;
        Ok(Statement::For { var, iter, body })
    }

    fn parse_loop_stmt(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::Loop)?;
        let body = self.parse_block()?;
        Ok(Statement::Loop { body })
    }

    fn parse_unsafe_stmt(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::Unsafe)?;
        let body = self.parse_block()?;
        Ok(Statement::Unsafe(body))
    }

    fn parse_asm_stmt(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::Asm)?;
        self.expect(Token::Bang)?;
        self.expect(Token::LParen)?;
        let asm_str = if let Some(Token::StringLiteral(s)) = self.peek() {
            let s = s.clone();
            self.advance();
            s
        } else {
            let span = self.current_span();
            return Err(ParseError::unexpected_token(
                "expected string literal",
                span,
                self.source,
            ));
        };
        self.expect(Token::RParen)?;
        self.expect(Token::Semicolon)?;
        Ok(Statement::Asm(asm_str))
    }

    fn parse_defer_stmt(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::Defer)?;
        let stmt = self.parse_statement()?;
        Ok(Statement::Defer(Box::new(stmt)))
    }

    // ── Expression parsing (Pratt parser) ──

    fn parse_expression(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            let next = self.peek().cloned();
            match next {
                Some(Token::LParen) => {
                    // Function call
                    self.advance();
                    let args = self.parse_call_args()?;
                    self.expect(Token::RParen)?;
                    lhs = Expr::Call {
                        func: Box::new(lhs),
                        args,
                    };
                    continue;
                }
                Some(Token::LBracket) => {
                    // Index
                    self.advance();
                    let index = self.parse_expression(0)?;
                    self.expect(Token::RBracket)?;
                    lhs = Expr::Index {
                        expr: Box::new(lhs),
                        index: Box::new(index),
                    };
                    continue;
                }
                Some(Token::Dot) => {
                    // Field access
                    self.advance();
                    let field = self.expect_ident()?;
                    lhs = Expr::FieldAccess {
                        expr: Box::new(lhs),
                        field,
                    };
                    continue;
                }
                Some(ref tok) => {
                    // Check if it's an infix operator
                    match infix_binding_power(tok) {
                        Some((l_bp, r_bp)) if l_bp >= min_bp => {
                            self.advance();
                            let rhs = self.parse_expression(r_bp)?;
                            lhs = self.make_binary_expr(lhs, tok, rhs)?;
                            continue;
                        }
                        _ => break,
                    }
                }
                None => break,
            };
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Some(Token::Minus) => {
                self.advance();
                let expr = self.parse_expression(13)?;
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                })
            }
            Some(Token::Bang) => {
                self.advance();
                let expr = self.parse_expression(13)?;
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                })
            }
            Some(Token::Star) => {
                self.advance();
                let expr = self.parse_expression(13)?;
                Ok(Expr::Deref(Box::new(expr)))
            }
            Some(Token::Amp) => {
                self.advance();
                if matches!(self.peek(), Some(Token::Mut)) {
                    self.advance();
                    let expr = self.parse_expression(13)?;
                    Ok(Expr::Ref {
                        mutable: true,
                        expr: Box::new(expr),
                    })
                } else {
                    let expr = self.parse_expression(13)?;
                    Ok(Expr::Ref {
                        mutable: false,
                        expr: Box::new(expr),
                    })
                }
            }
            Some(Token::AmpMut) => {
                self.advance();
                let expr = self.parse_expression(13)?;
                Ok(Expr::Ref {
                    mutable: true,
                    expr: Box::new(expr),
                })
            }
            Some(Token::Sizeof) => self.parse_sizeof(),
            Some(Token::Alignof) => self.parse_alignof(),
            Some(Token::If) => self.parse_if_expr(),
            Some(Token::LBrace) => {
                let block = self.parse_block()?;
                Ok(Expr::Block(block))
            }
            Some(Token::LBracket) => self.parse_array_or_index(),
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expression(0)?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            Some(Token::IntLiteral(n)) => {
                let n = *n;
                self.advance();
                Ok(Expr::IntLiteral(n))
            }
            Some(Token::FloatLiteral(f)) => {
                let f = *f;
                self.advance();
                Ok(Expr::FloatLiteral(f))
            }
            Some(Token::True) => {
                self.advance();
                Ok(Expr::BoolLiteral(true))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Expr::BoolLiteral(false))
            }
            Some(Token::StringLiteral(s)) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::StringLiteral(s))
            }
            Some(Token::Self_) => {
                self.advance();
                Ok(Expr::Self_)
            }
            Some(Token::Ident(name)) => {
                let name = name.clone();
                self.advance();
                // Check for struct literal: Name { ... }
                if matches!(self.peek(), Some(Token::LBrace)) && name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    self.advance(); // {
                    let mut fields = Vec::new();
                    while !matches!(self.peek(), Some(Token::RBrace)) && !self.is_eof() {
                        let field_name = self.expect_ident()?;
                        self.expect(Token::Colon)?;
                        let field_val = self.parse_expression(0)?;
                        fields.push((field_name, field_val));
                        if matches!(self.peek(), Some(Token::Comma)) {
                            self.advance();
                            if matches!(self.peek(), Some(Token::RBrace)) {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RBrace)?;
                    Ok(Expr::StructLit { name, fields })
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            Some(tok) => {
                let span = self.current_span();
                Err(ParseError::unexpected_token(
                    &tok.name(),
                    span,
                    self.source,
                ))
            }
            None => Err(ParseError::UnexpectedEof),
        }
    }

    fn parse_call_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if matches!(self.peek(), Some(Token::RParen)) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expression(0)?);
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(args)
    }

    fn parse_sizeof(&mut self) -> Result<Expr, ParseError> {
        self.expect(Token::Sizeof)?;
        self.expect(Token::LParen)?;
        let ty = self.parse_type()?;
        self.expect(Token::RParen)?;
        Ok(Expr::Sizeof(ty))
    }

    fn parse_alignof(&mut self) -> Result<Expr, ParseError> {
        self.expect(Token::Alignof)?;
        self.expect(Token::LParen)?;
        let ty = self.parse_type()?;
        self.expect(Token::RParen)?;
        Ok(Expr::Alignof(ty))
    }

    fn parse_if_expr(&mut self) -> Result<Expr, ParseError> {
        self.expect(Token::If)?;
        let cond = self.parse_expression(0)?;
        let then_branch = self.parse_block()?;
        let else_branch = if matches!(self.peek(), Some(Token::Else)) {
            self.advance();
            if matches!(self.peek(), Some(Token::If)) {
                Some(Box::new(self.parse_if_expr()?))
            } else if matches!(self.peek(), Some(Token::LBrace)) {
                let block = self.parse_block()?;
                Some(Box::new(Expr::Block(block)))
            } else {
                let expr = self.parse_expression(0)?;
                Some(Box::new(expr))
            }
        } else {
            None
        };
        Ok(Expr::IfExpr {
            cond: Box::new(cond),
            then_branch,
            else_branch,
        })
    }

    fn parse_array_or_index(&mut self) -> Result<Expr, ParseError> {
        self.expect(Token::LBracket)?;
        if matches!(self.peek(), Some(Token::RBracket)) {
            self.advance();
            return Ok(Expr::ArrayLit(vec![]));
        }
        let first = self.parse_expression(0)?;
        // Check if this is an array literal [a, b, c] or index
        if matches!(self.peek(), Some(Token::Comma)) {
            let mut elements = vec![first];
            while matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
                if matches!(self.peek(), Some(Token::RBracket)) {
                    break;
                }
                elements.push(self.parse_expression(0)?);
            }
            self.expect(Token::RBracket)?;
            Ok(Expr::ArrayLit(elements))
        } else {
            self.expect(Token::RBracket)?;
            // It's not really an index here, it's just an expression in brackets
            Ok(first)
        }
    }

    fn make_binary_expr(&self, lhs: Expr, op: &Token, rhs: Expr) -> Result<Expr, ParseError> {
        let binop = match op {
            Token::Plus => BinOp::Add,
            Token::Minus => BinOp::Sub,
            Token::Star => BinOp::Mul,
            Token::Slash => BinOp::Div,
            Token::Percent => BinOp::Rem,
            Token::Amp => BinOp::And,
            Token::Pipe => BinOp::Or,
            Token::Caret => BinOp::Xor,
            Token::Shl => BinOp::Shl,
            Token::Shr => BinOp::Shr,
            Token::EqEq => BinOp::Eq,
            Token::Ne => BinOp::Ne,
            Token::Lt => BinOp::Lt,
            Token::Le => BinOp::Le,
            Token::Gt => BinOp::Gt,
            Token::Ge => BinOp::Ge,
            Token::Eq => BinOp::Assign,
            Token::PlusEq => BinOp::Add,
            Token::MinusEq => BinOp::Sub,
            Token::StarEq => BinOp::Mul,
            Token::SlashEq => BinOp::Div,
            Token::AmpEq => BinOp::And,
            Token::PipeEq => BinOp::Or,
            Token::DotDot | Token::DotDotEq => {
                // Range expression
                let inclusive = matches!(op, Token::DotDotEq);
                return Ok(Expr::Range {
                    start: Some(Box::new(lhs)),
                    end: Some(Box::new(rhs)),
                    inclusive,
                });
            }
            Token::As => {
                // Cast: extract the type from rhs
                let ty = match rhs {
                    Expr::Ident(name) => Type::Named(name),
                    _ => Type::Named("unknown".into()),
                };
                return Ok(Expr::Cast {
                    expr: Box::new(lhs),
                    ty,
                });
            }
            _ => return Ok(Expr::Binary {
                left: Box::new(lhs),
                op: BinOp::Add,
                right: Box::new(rhs),
            }),
        };

        // Handle compound assignment: +=, -=, etc.
        if matches!(
            op,
            Token::PlusEq
                | Token::MinusEq
                | Token::StarEq
                | Token::SlashEq
                | Token::AmpEq
                | Token::PipeEq
        ) {
            return Ok(Expr::Assign {
                target: Box::new(lhs),
                op: Some(binop),
                value: Box::new(rhs),
            });
        }

        // Handle regular assignment
        if matches!(op, Token::Eq) {
            return Ok(Expr::Assign {
                target: Box::new(lhs),
                op: None,
                value: Box::new(rhs),
            });
        }

        Ok(Expr::Binary {
            left: Box::new(lhs),
            op: binop,
            right: Box::new(rhs),
        })
    }

    // ── Type parsing ──

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        match self.peek() {
            Some(Token::Star) => {
                self.advance();
                if matches!(self.peek(), Some(Token::Mut)) {
                    self.advance();
                    let inner = self.parse_type()?;
                    Ok(Type::MutPtr(Box::new(inner)))
                } else if matches!(self.peek(), Some(Token::Const)) {
                    // We don't have a `const` keyword token, but handle it anyway
                    self.advance();
                    let inner = self.parse_type()?;
                    Ok(Type::ConstPtr(Box::new(inner)))
                } else {
                    let inner = self.parse_type()?;
                    Ok(Type::Ptr(Box::new(inner)))
                }
            }
            Some(Token::Amp) => {
                self.advance();
                if matches!(self.peek(), Some(Token::Mut)) {
                    self.advance();
                    let inner = self.parse_type()?;
                    Ok(Type::MutPtr(Box::new(inner)))
                } else {
                    let inner = self.parse_type()?;
                    Ok(Type::Ptr(Box::new(inner)))
                }
            }
            Some(Token::LBracket) => {
                self.advance();
                let ty = self.parse_type()?;
                self.expect(Token::Semicolon)?;
                let size = if let Some(Token::IntLiteral(n)) = self.peek() {
                    let n = *n as usize;
                    self.advance();
                    n
                } else {
                    let span = self.current_span();
                    return Err(ParseError::unexpected_token(
                        "expected integer literal for array size",
                        span,
                        self.source,
                    ));
                };
                self.expect(Token::RBracket)?;
                Ok(Type::Array(Box::new(ty), size))
            }
            Some(Token::Fn) => {
                self.advance();
                self.expect(Token::LParen)?;
                let mut params = Vec::new();
                if !matches!(self.peek(), Some(Token::RParen)) {
                    loop {
                        params.push(self.parse_type()?);
                        if matches!(self.peek(), Some(Token::Comma)) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(Token::RParen)?;
                let ret = if matches!(self.peek(), Some(Token::Arrow)) {
                    self.advance();
                    Some(Box::new(self.parse_type()?))
                } else {
                    None
                };
                Ok(Type::Fn(params, ret))
            }
            Some(Token::Ident(name)) => {
                let name = name.clone();
                self.advance();
                // Check for ptr<T> syntax
                if name == "ptr" && matches!(self.peek(), Some(Token::Lt)) {
                    self.advance();
                    let inner = self.parse_type()?;
                    self.expect(Token::Gt)?;
                    Ok(Type::Ptr(Box::new(inner)))
                } else if matches!(self.peek(), Some(Token::Lt)) {
                    // Generic type like Option<T>
                    self.advance();
                    let _inner = self.parse_type()?;
                    self.expect(Token::Gt)?;
                    // For simplicity, treat generics as named types with the generic parameter
                    // stored as part of the name
                    // Actually, we should return a Named type here
                    // The HIR layer can handle generics
                    Ok(Type::Named(name))
                } else {
                    Ok(Type::Named(name))
                }
            }
            Some(tok) => {
                let span = self.current_span();
                Err(ParseError::unexpected_token(
                    &tok.name(),
                    span,
                    self.source,
                ))
            }
            None => Err(ParseError::UnexpectedEof),
        }
    }
}

/// Get the binding power for a prefix operator.
fn _prefix_binding_power(op: &Token) -> Option<((), u8)> {
    match op {
        Token::Minus | Token::Bang | Token::Star | Token::Amp => Some(((), 13)),
        _ => None,
    }
}

/// Get the (left, right) binding power for an infix operator.
fn infix_binding_power(op: &Token) -> Option<(u8, u8)> {
    match op {
        Token::Eq
        | Token::PlusEq
        | Token::MinusEq
        | Token::StarEq
        | Token::SlashEq
        | Token::AmpEq
        | Token::PipeEq => Some((1, 2)),
        Token::DotDot | Token::DotDotEq => Some((3, 2)),
        Token::EqEq | Token::Ne | Token::Lt | Token::Le | Token::Gt | Token::Ge => Some((5, 6)),
        Token::Pipe => Some((7, 8)),
        Token::Caret => Some((9, 10)),
        Token::Amp => Some((11, 12)),
        Token::Shl | Token::Shr => Some((13, 14)),
        Token::Plus | Token::Minus => Some((15, 16)),
        Token::Star | Token::Slash | Token::Percent => Some((17, 18)),
        Token::As => Some((19, 20)),
        _ => None,
    }
}

/// Parse Nova source code and return the AST.
pub fn parse_source(source: &str) -> Result<Program, Vec<ParseError>> {
    let mut parser = Parser::new(source);
    let program = parser.parse_program();
    if parser.has_errors() {
        Err(parser.errors)
    } else {
        Ok(program)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_function() {
        let source = "fn main() -> i64 { let x = 42; return x; }";
        let result = parse_source(source);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        let program = result.unwrap();
        assert_eq!(program.items.len(), 1);
        match &program.items[0] {
            Item::Function(func) => {
                assert_eq!(func.name, "main");
                assert_eq!(func.params.len(), 0);
                assert!(func.return_type.is_some());
                assert!(func.body.is_some());
            }
            _ => panic!("Expected Function item"),
        }
    }

    #[test]
    fn test_parse_function_with_params() {
        let source = "fn add(a: i64, b: i64) -> i64 { return a + b; }";
        let result = parse_source(source);
        assert!(result.is_ok());
        let program = result.unwrap();
        match &program.items[0] {
            Item::Function(func) => {
                assert_eq!(func.name, "add");
                assert_eq!(func.params.len(), 2);
                assert_eq!(func.params[0].name, "a");
                assert_eq!(func.params[1].name, "b");
            }
            _ => panic!("Expected Function item"),
        }
    }

    #[test]
    fn test_parse_struct() {
        let source = "struct Point { x: i64, y: i64 }";
        let result = parse_source(source);
        assert!(result.is_ok());
        let program = result.unwrap();
        match &program.items[0] {
            Item::Struct(sd) => {
                assert_eq!(sd.name, "Point");
                assert_eq!(sd.fields.len(), 2);
            }
            _ => panic!("Expected Struct item"),
        }
    }

    #[test]
    fn test_parse_enum() {
        let source = "enum Option { Some(i64), None }";
        let result = parse_source(source);
        assert!(result.is_ok());
        let program = result.unwrap();
        match &program.items[0] {
            Item::Enum(ed) => {
                assert_eq!(ed.name, "Option");
                assert_eq!(ed.variants.len(), 2);
            }
            _ => panic!("Expected Enum item"),
        }
    }

    #[test]
    fn test_parse_impl() {
        let source = "impl Point { fn new(x: i64, y: i64) -> Point { return Point { x: x, y: y }; } }";
        let result = parse_source(source);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        let program = result.unwrap();
        match &program.items[0] {
            Item::Impl(ib) => {
                assert_eq!(ib.items.len(), 1);
            }
            _ => panic!("Expected Impl item"),
        }
    }

    #[test]
    fn test_parse_if_statement() {
        let source = "fn main() { if x > 0 { return x; } else { return 0; } }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_while_loop() {
        let source = "fn main() { while x > 0 { x = x - 1; } }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_for_loop() {
        let source = "fn main() { for i in 0..10 { let x = i; } }";
        let result = parse_source(source);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    }

    #[test]
    fn test_parse_attributes() {
        let source = "#[inline] fn fast_add(a: i64, b: i64) -> i64 { return a + b; }";
        let result = parse_source(source);
        assert!(result.is_ok());
        let program = result.unwrap();
        match &program.items[0] {
            Item::Function(func) => {
                assert_eq!(func.attrs.len(), 1);
                assert_eq!(func.attrs[0].name, "inline");
            }
            _ => panic!("Expected Function item"),
        }
    }

    #[test]
    fn test_parse_unsafe_block() {
        let source = "fn main() { unsafe { let ptr: *mut i64 = 0; } }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_extern_block() {
        let source = "extern \"C\" { fn printf(fmt: *const u8); }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_generics() {
        let source = "fn identity<T>(x: T) -> T { return x; }";
        let result = parse_source(source);
        assert!(result.is_ok());
        let program = result.unwrap();
        match &program.items[0] {
            Item::Function(func) => {
                assert_eq!(func.name, "identity");
                assert_eq!(func.generics.len(), 1);
                assert_eq!(func.generics[0].name, "T");
            }
            _ => panic!("Expected Function item"),
        }
    }

    #[test]
    fn test_parse_detailed_ast() {
        let source = "fn main() -> i64 {\n    let x = 42;\n    return x;\n}\n";
        let result = parse_source(source);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        let program = result.unwrap();

        // Verify program structure
        assert_eq!(program.items.len(), 1);

        match &program.items[0] {
            Item::Function(func) => {
                assert_eq!(func.name, "main");
                assert_eq!(func.params.len(), 0);
                assert!(func.return_type.is_some());
                match func.return_type.as_ref().unwrap() {
                    Type::Named(name) => assert_eq!(name, "i64"),
                    _ => panic!("Expected Named type"),
                }

                // Verify body
                let body = func.body.as_ref().expect("Expected body");
                assert_eq!(body.statements.len(), 2);

                // Verify let statement
                match &body.statements[0] {
                    Statement::Let { mutable, name, ty, init } => {
                        assert!(!mutable);
                        assert_eq!(name, "x");
                        assert!(ty.is_none());
                        match init {
                            Some(Expr::IntLiteral(42)) => {}
                            _ => panic!("Expected IntLiteral(42), got {:?}", init),
                        }
                    }
                    _ => panic!("Expected Let statement"),
                }

                // Verify return statement
                match &body.statements[1] {
                    Statement::Return(Some(expr)) => {
                        match expr {
                            Expr::Ident(name) => assert_eq!(name, "x"),
                            _ => panic!("Expected Ident"),
                        }
                    }
                    _ => panic!("Expected Return statement with expression"),
                }
            }
            _ => panic!("Expected Function item"),
        }
    }
}