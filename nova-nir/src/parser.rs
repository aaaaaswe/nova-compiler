//! NIR text parser – parses `.nir` files into the `Module` IR structure.
//!
//! Uses `logos` for lexing and a recursive-descent parser for the AST.

use logos::{Lexer, Logos};
use std::collections::HashMap;

use crate::ir::{AddrExpr, BasicBlock, Function, Instruction, Module};
use crate::types::{IrType, Value};

// =============================================================================
//  Token (Lexer)
// =============================================================================

#[derive(Logos, Debug, PartialEq, Clone, Copy)]
#[logos(skip r"[ \t\r\n]+")]
pub enum Token {
    #[token("->")]
    Arrow,

    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token("=")]
    Eq,
    #[token("@")]
    At,
    #[token("%")]
    Percent,
    #[token("+")]
    Plus,
    #[token("*")]
    Star,
    #[token("-")]
    Minus,

    #[regex(r"[0-9]+")]
    Number,

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_\.]*")]
    Ident,

    #[token(";", lex_comment)]
    Comment,
}

fn lex_comment(lex: &mut Lexer<Token>) -> logos::Skip {
    let remaining = lex.remainder();
    let len = remaining.find('\n').unwrap_or(remaining.len());
    lex.bump(len);
    logos::Skip
}

// =============================================================================
//  TokenInfo
// =============================================================================

#[derive(Debug, Clone)]
struct TokenInfo {
    kind: Token,
    span: (usize, usize),
    line: usize,
}

// =============================================================================
//  ParseError
// =============================================================================

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
}

impl ParseError {
    pub fn new(message: impl Into<String>, line: usize) -> Self {
        ParseError {
            message: message.into(),
            line,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

// =============================================================================
//  Parser
// =============================================================================

type ParseResult<T> = Result<T, ParseError>;

pub struct Parser<'a> {
    tokens: Vec<TokenInfo>,
    source: &'a str,
    pos: usize,
    source_name: String,
    func_vreg_counter: usize,
    current_params: HashMap<String, Value>,
    vreg_table: HashMap<String, Value>,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str, source_name: String) -> Self {
        let lexer = Token::lexer(source);
        let mut tokens = Vec::new();
        let mut line = 1usize;
        let mut last_end = 0usize;

        for (tok_res, span) in lexer.spanned() {
            if let Ok(tok) = tok_res {
                if tok == Token::Comment {
                    continue;
                }
                let between = &source[last_end..span.start];
                line += between.chars().filter(|&c| c == '\n').count();
                tokens.push(TokenInfo {
                    kind: tok,
                    span: (span.start, span.end),
                    line,
                });
                let token_text = &source[span.start..span.end];
                line += token_text.chars().filter(|&c| c == '\n').count();
                last_end = span.end;
            }
        }

        Parser {
            tokens,
            source,
            pos: 0,
            source_name,
            func_vreg_counter: 0,
            current_params: HashMap::new(),
            vreg_table: HashMap::new(),
        }
    }

    // -- Position-based accessors -------------------------------------------

    fn cur_kind(&self) -> Token {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].kind
        } else {
            Token::Comment
        }
    }

    fn cur_line(&self) -> usize {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].line
        } else {
            1
        }
    }

    fn text_at(&self, pos: usize) -> &str {
        let info = &self.tokens[pos];
        &self.source[info.span.0..info.span.1]
    }

    fn cur_text(&self) -> &str {
        self.text_at(self.pos)
    }

    fn advance(&mut self) -> usize {
        let old = self.pos;
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        old
    }

    fn expect(&mut self, kind: Token) -> ParseResult<usize> {
        let cur_kind = self.tokens[self.pos].kind;
        if std::mem::discriminant(&cur_kind) == std::mem::discriminant(&kind) {
            Ok(self.advance())
        } else {
            Err(self.error(format!("expected {:?}, got {:?}", kind, cur_kind)))
        }
    }

    fn consume(&mut self, kind: Token) -> bool {
        if std::mem::discriminant(&self.cur_kind()) == std::mem::discriminant(&kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn matches_kind(&self, kind: Token) -> bool {
        if self.pos < self.tokens.len() {
            std::mem::discriminant(&self.tokens[self.pos].kind) == std::mem::discriminant(&kind)
        } else {
            false
        }
    }

    fn matches_ident(&self, text: &str) -> bool {
        if self.pos < self.tokens.len() {
            if let Token::Ident = self.tokens[self.pos].kind {
                self.cur_text() == text
            } else {
                false
            }
        } else {
            false
        }
    }

    fn expect_number(&mut self) -> ParseResult<i64> {
        if !matches!(self.cur_kind(), Token::Number) {
            return Err(self.error(format!("expected number, got {:?}", self.cur_kind())));
        }
        let pos = self.advance();
        let text = self.text_at(pos);
        text.parse::<i64>()
            .map_err(|_| self.error(format!("invalid number '{}'", text)))
    }

    fn is_type_or_keyword(&self) -> bool {
        if self.pos < self.tokens.len() {
            matches_type_name(self.cur_text())
        } else {
            false
        }
    }

    fn error(&self, msg: String) -> ParseError {
        ParseError::new(msg, self.cur_line())
    }

    fn new_vreg(&mut self, ty: IrType) -> Value {
        let name = format!("%_tmp_{}", self.func_vreg_counter);
        self.func_vreg_counter += 1;
        Value::VReg { name, ty }
    }

    fn get_or_create_vreg(&mut self, name: &str, ty: IrType) -> Value {
        if let Some(vreg) = self.vreg_table.get(name) {
            let mut v = vreg.clone();
            if v.ty() == &IrType::Void {
                v = Value::VReg {
                    name: name.to_string(),
                    ty,
                };
                self.vreg_table.insert(name.to_string(), v.clone());
            }
            return v;
        }
        let vreg = Value::VReg {
            name: name.to_string(),
            ty,
        };
        self.vreg_table.insert(name.to_string(), vreg.clone());
        vreg
    }

    // =========================================================================
    //  Type parsing
    // =========================================================================

    fn parse_type(&mut self) -> ParseResult<IrType> {
        let pos = self.advance();
        let text = self.text_at(pos);
        parse_type_name(text).ok_or_else(|| self.error(format!("unknown type '{}'", text)))
    }

    // =========================================================================
    //  Operand parsing
    // =========================================================================

    fn parse_operand(&mut self) -> ParseResult<Value> {
        match self.cur_kind() {
            Token::Percent => {
                self.advance(); // consume %
                let pos = self.advance(); // consume name
                let name_text = self.text_at(pos);
                let vreg_name = format!("%{}", name_text);

                if let Some(param) = self.current_params.get(&vreg_name) {
                    return Ok(param.clone());
                }
                if let Some(vreg) = self.vreg_table.get(&vreg_name) {
                    return Ok(vreg.clone());
                }
                let vreg = Value::VReg {
                    name: vreg_name.clone(),
                    ty: IrType::Void,
                };
                self.vreg_table.insert(vreg_name, vreg.clone());
                Ok(vreg)
            }
            Token::Number => {
                let pos = self.advance();
                let text = self.text_at(pos);
                let value: i64 = text
                    .parse()
                    .map_err(|_| self.error(format!("invalid number '{}'", text)))?;
                Ok(Value::ConstInt {
                    value,
                    ty: IrType::I64,
                })
            }
            Token::Minus => {
                self.advance(); // consume -
                if let Token::Number = self.cur_kind() {
                    let pos = self.advance();
                    let text = self.text_at(pos);
                    let value: i64 = format!("-{}", text)
                        .parse()
                        .map_err(|_| self.error(format!("invalid number '-{}'", text)))?;
                    Ok(Value::ConstInt {
                        value,
                        ty: IrType::I64,
                    })
                } else {
                    Err(self.error("expected number after '-'".to_string()))
                }
            }
            Token::At => {
                self.advance(); // consume @
                let pos = self.advance(); // consume name
                let name = self.text_at(pos).to_string();
                Ok(Value::GlobalVar {
                    name,
                    ty: IrType::Ptr,
                })
            }
            _ => Err(self.error(format!(
                "expected operand, got {:?}",
                self.cur_kind()
            ))),
        }
    }

    // =========================================================================
    //  Address parsing
    // =========================================================================

    fn parse_addr(&mut self) -> ParseResult<AddrExpr> {
        self.expect(Token::LBracket)?;
        let base = self.parse_operand()?;
        let mut index: Option<Value> = None;
        let mut scale: i32 = 1;
        let mut offset: i64 = 0;

        if self.consume(Token::Plus) || self.consume(Token::Minus) {
            let last_was_minus = self.pos > 0 && matches!(self.tokens[self.pos - 1].kind, Token::Minus);
            let sign: i32 = if last_was_minus { -1 } else { 1 };

            if matches!(self.cur_kind(), Token::Percent) {
                let idx_op = self.parse_operand()?;
                if self.consume(Token::Star) {
                    let scale_num = self.expect_number()? as i32;
                    scale = scale_num * sign;
                } else {
                    scale = sign;
                }
                index = Some(idx_op);

                if self.matches_kind(Token::Plus) || self.matches_kind(Token::Minus) {
                    let off_sign: i64 = if self.matches_kind(Token::Plus) { 1 } else { -1 };
                    self.advance();
                    let off = self.expect_number()?;
                    offset = off * off_sign;
                }
            } else if matches!(self.cur_kind(), Token::Number) {
                let off = self.expect_number()?;
                offset = off * sign as i64;
            } else {
                return Err(self.error("expected index register or offset".to_string()));
            }
        }

        self.expect(Token::RBracket)?;
        AddrExpr::new(base, index, scale, offset).map_err(|e| self.error(e.to_string()))
    }

    // =========================================================================
    //  Instruction parsing
    // =========================================================================

    pub fn parse_instruction(&mut self) -> ParseResult<Instruction> {
        if matches!(self.cur_kind(), Token::Percent) {
            self.advance(); // consume %
            let pos1 = self.advance(); // consume name
            let name1 = self.text_at(pos1).to_string();
            let vreg1_name = format!("%{}", name1);

            if self.consume(Token::Comma) {
                // Two results: %r, %f = opcode ...
                self.expect(Token::Percent)?;
                let pos2 = self.advance(); // consume name
                let name2 = self.text_at(pos2).to_string();
                let vreg2_name = format!("%{}", name2);
                self.expect(Token::Eq)?;
                let op_pos = self.advance();
                let opcode = self.text_at(op_pos).to_string();
                let result = self.get_or_create_vreg(&vreg1_name, IrType::Void);
                let flags_result = self.get_or_create_vreg(&vreg2_name, IrType::Flags);
                self.parse_two_result_inst(&opcode, result, flags_result)
            } else if self.consume(Token::Eq) {
                let op_pos = self.advance();
                let opcode = self.text_at(op_pos).to_string();
                let result = self.get_or_create_vreg(&vreg1_name, IrType::Void);
                self.parse_one_result_inst(&opcode, result)
            } else {
                Err(self.error(format!(
                    "expected ',' or '=' after virtual register, got {:?}",
                    self.cur_kind()
                )))
            }
        } else {
            let op_pos = self.advance();
            let opcode = self.text_at(op_pos).to_string();
            self.parse_no_result_inst(&opcode)
        }
    }

    // -- Two-result instructions ---------------------------------------------

    fn parse_two_result_inst(
        &mut self,
        opcode: &str,
        result: Value,
        flags_result: Value,
    ) -> ParseResult<Instruction> {
        match opcode {
            "add" | "sub" | "mul" | "mulh" | "div" | "divu" | "rem" | "remu" | "and" | "or"
            | "xor" | "shl" | "shr" | "sar" | "rotl" | "rotr" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let lhs = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let rhs = self.parse_operand()?;
                macro_rules! bin_arith {
                    ($variant:ident) => {
                        Instruction::$variant { result, lhs, rhs, flags_result }
                    };
                }
                Ok(match opcode {
                    "add" => bin_arith!(Add), "sub" => bin_arith!(Sub),
                    "mul" => bin_arith!(Mul), "mulh" => bin_arith!(Mulh),
                    "div" => bin_arith!(Div), "divu" => bin_arith!(Divu),
                    "rem" => bin_arith!(Rem), "remu" => bin_arith!(Remu),
                    "and" => bin_arith!(And), "or" => bin_arith!(Or),
                    "xor" => bin_arith!(Xor), "shl" => bin_arith!(Shl),
                    "shr" => bin_arith!(Shr), "sar" => bin_arith!(Sar),
                    "rotl" => bin_arith!(Rotl), "rotr" => bin_arith!(Rotr),
                    _ => unreachable!(),
                })
            }
            "neg" | "not" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let operand = self.parse_operand()?;
                Ok(match opcode {
                    "neg" => Instruction::Neg { result, operand, flags_result },
                    "not" => Instruction::Not { result, operand, flags_result },
                    _ => unreachable!(),
                })
            }
            "addi" | "subi" | "muli" | "andi" | "ori" | "xori" | "shli" | "shri" | "sari"
            | "rotli" | "rotri" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let lhs = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let imm = self.expect_number()?;
                macro_rules! bin_imm_flags {
                    ($variant:ident) => {
                        Instruction::$variant { result, lhs, imm, flags_result: Some(flags_result) }
                    };
                }
                Ok(match opcode {
                    "addi" => bin_imm_flags!(Addi), "subi" => bin_imm_flags!(Subi),
                    "muli" => bin_imm_flags!(Muli), "andi" => bin_imm_flags!(Andi),
                    "ori" => bin_imm_flags!(Ori), "xori" => bin_imm_flags!(Xori),
                    "shli" => bin_imm_flags!(Shli), "shri" => bin_imm_flags!(Shri),
                    "sari" => bin_imm_flags!(Sari), "rotli" => bin_imm_flags!(Rotli),
                    "rotri" => bin_imm_flags!(Rotri),
                    _ => unreachable!(),
                })
            }
            _ => Err(self.error(format!("unknown two-result opcode '{}'", opcode))),
        }
    }

    // -- One-result instructions ---------------------------------------------

    fn parse_one_result_inst(&mut self, opcode: &str, result: Value) -> ParseResult<Instruction> {
        match opcode {
            "addi" | "subi" | "muli" | "andi" | "ori" | "xori" | "shli" | "shri" | "sari"
            | "rotli" | "rotri" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let lhs = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let imm = self.expect_number()?;
                macro_rules! bin_imm {
                    ($variant:ident) => {
                        Instruction::$variant { result, lhs, imm, flags_result: None }
                    };
                }
                Ok(match opcode {
                    "addi" => bin_imm!(Addi), "subi" => bin_imm!(Subi),
                    "muli" => bin_imm!(Muli), "andi" => bin_imm!(Andi),
                    "ori" => bin_imm!(Ori), "xori" => bin_imm!(Xori),
                    "shli" => bin_imm!(Shli), "shri" => bin_imm!(Shri),
                    "sari" => bin_imm!(Sari), "rotli" => bin_imm!(Rotli),
                    "rotri" => bin_imm!(Rotri),
                    _ => unreachable!(),
                })
            }
            "movi" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let imm = self.expect_number()?;
                Ok(Instruction::Movi { result, imm })
            }
            "mov" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let src = self.parse_operand()?;
                Ok(Instruction::Mov { result, src })
            }
            "test_eq" | "test_ne" | "test_lt" | "test_le" | "test_ltu" | "test_leu"
            | "test_of" | "test_cf" | "test_sf" | "test_ge" | "test_gt" | "test_geu"
            | "test_gtu" => {
                let flags = self.parse_operand()?;
                let result = self.update_vreg_type(result, IrType::I1);
                macro_rules! flag_consumer {
                    ($variant:ident) => { Instruction::$variant { result, flags } };
                }
                Ok(match opcode {
                    "test_eq" => flag_consumer!(TestEq), "test_ne" => flag_consumer!(TestNe),
                    "test_lt" => flag_consumer!(TestLt), "test_le" => flag_consumer!(TestLe),
                    "test_ltu" => flag_consumer!(TestLtu), "test_leu" => flag_consumer!(TestLeu),
                    "test_of" => flag_consumer!(TestOf), "test_cf" => flag_consumer!(TestCf),
                    "test_sf" => flag_consumer!(TestSf), "test_ge" => flag_consumer!(TestGe),
                    "test_gt" => flag_consumer!(TestGt), "test_geu" => flag_consumer!(TestGeu),
                    "test_gtu" => flag_consumer!(TestGtu),
                    _ => unreachable!(),
                })
            }
            "load" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                self.consume(Token::Comma);
                let addr = self.parse_addr()?;
                Ok(Instruction::Load { result, addr })
            }
            "loadi" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                self.consume(Token::Comma);
                let base = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let offset = self.expect_number()?;
                Ok(Instruction::Loadi { result, base, offset })
            }
            "load_sext" => {
                let from_type = self.parse_type()?;
                self.expect(Token::Arrow)?;
                let to_type = self.parse_type()?;
                let result = self.update_vreg_type(result, to_type);
                self.consume(Token::Comma);
                let addr = self.parse_addr()?;
                Ok(Instruction::LoadSext { result, addr, from_type })
            }
            "load_zext" => {
                let from_type = self.parse_type()?;
                self.expect(Token::Arrow)?;
                let to_type = self.parse_type()?;
                let result = self.update_vreg_type(result, to_type);
                self.consume(Token::Comma);
                let addr = self.parse_addr()?;
                Ok(Instruction::LoadZext { result, addr, from_type })
            }
            "lea" => {
                let addr = self.parse_addr()?;
                let result = self.update_vreg_type(result, IrType::Ptr);
                Ok(Instruction::Lea { result, addr })
            }
            "mem_xchg" => {
                let addr = self.parse_addr()?;
                self.expect(Token::Comma)?;
                let value = self.parse_operand()?;
                let result = self.update_vreg_type(result, value.ty().clone());
                Ok(Instruction::MemXchg { result, addr, value })
            }
            "atomic_xchg" => {
                let addr = self.parse_addr()?;
                self.expect(Token::Comma)?;
                let value = self.parse_operand()?;
                let result = self.update_vreg_type(result, value.ty().clone());
                Ok(Instruction::AtomicMemXchg { result, addr, value })
            }
            "atomic_cas" => {
                let addr = self.parse_addr()?;
                self.expect(Token::Comma)?;
                let expected = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let desired = self.parse_operand()?;
                let result = self.update_vreg_type(result, expected.ty().clone());
                Ok(Instruction::AtomicCas { result, addr, expected, desired })
            }
            "pop" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty);
                Ok(Instruction::Pop { result })
            }
            "call" => {
                self.expect(Token::At)?;
                let pos = self.advance();
                let callee_name = self.text_at(pos).to_string();
                self.expect(Token::LParen)?;
                let args = self.parse_arg_list()?;
                Ok(Instruction::Call { result: Some(result), callee_name, args })
            }
            "call_indirect" => {
                let fnptr = self.parse_operand()?;
                self.expect(Token::LParen)?;
                let args = self.parse_arg_list()?;
                Ok(Instruction::CallIndirect { result: Some(result), fnptr, args })
            }
            "fadd" | "fsub" | "fmul" | "fdiv" | "fmin" | "fmax" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let lhs = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let rhs = self.parse_operand()?;
                macro_rules! fbin {
                    ($variant:ident) => { Instruction::$variant { result, lhs, rhs } };
                }
                Ok(match opcode {
                    "fadd" => fbin!(Fadd), "fsub" => fbin!(Fsub),
                    "fmul" => fbin!(Fmul), "fdiv" => fbin!(Fdiv),
                    "fmin" => fbin!(Fmin), "fmax" => fbin!(Fmax),
                    _ => unreachable!(),
                })
            }
            "fneg" | "fabs" | "fsqrt" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let operand = self.parse_operand()?;
                macro_rules! funary {
                    ($variant:ident) => { Instruction::$variant { result, operand } };
                }
                Ok(match opcode {
                    "fneg" => funary!(Fneg), "fabs" => funary!(Fabs),
                    "fsqrt" => funary!(Fsqrt),
                    _ => unreachable!(),
                })
            }
            "ffma" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let a = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let b = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let c = self.parse_operand()?;
                Ok(Instruction::Ffma { result, a, b, c })
            }
            "fcmp_eq" | "fcmp_ne" | "fcmp_lt" | "fcmp_le" | "fcmp_gt" | "fcmp_ge"
            | "fcmp_ord" | "fcmp_uno" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let lhs = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let rhs = self.parse_operand()?;
                macro_rules! fcmp {
                    ($variant:ident) => { Instruction::$variant { result, lhs, rhs } };
                }
                Ok(match opcode {
                    "fcmp_eq" => fcmp!(FcmpEq), "fcmp_ne" => fcmp!(FcmpNe),
                    "fcmp_lt" => fcmp!(FcmpLt), "fcmp_le" => fcmp!(FcmpLe),
                    "fcmp_gt" => fcmp!(FcmpGt), "fcmp_ge" => fcmp!(FcmpGe),
                    "fcmp_ord" => fcmp!(FcmpOrd), "fcmp_uno" => fcmp!(FcmpUno),
                    _ => unreachable!(),
                })
            }
            "vadd" | "vsub" | "vmul" | "vdiv" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let lhs = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let rhs = self.parse_operand()?;
                macro_rules! vbin {
                    ($variant:ident) => { Instruction::$variant { result, lhs, rhs } };
                }
                Ok(match opcode {
                    "vadd" => vbin!(Vadd), "vsub" => vbin!(Vsub),
                    "vmul" => vbin!(Vmul), "vdiv" => vbin!(Vdiv),
                    _ => unreachable!(),
                })
            }
            "vfma" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let a = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let b = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let c = self.parse_operand()?;
                Ok(Instruction::Vfma { result, a, b, c })
            }
            "vshuffle" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let lhs = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let rhs = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let mask = self.parse_operand()?;
                Ok(Instruction::Vshuffle { result, lhs, rhs, mask })
            }
            "vbroadcast" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let value = self.parse_operand()?;
                Ok(Instruction::Vbroadcast { result, value })
            }
            "vextract" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let vector = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let index = self.expect_number()? as usize;
                Ok(Instruction::Vextract { result, vector, index })
            }
            "vinsert" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let vector = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let value = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let index = self.expect_number()? as usize;
                Ok(Instruction::Vinsert { result, vector, value, index })
            }
            "vreduce_add" | "vreduce_min" | "vreduce_max" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let vector = self.parse_operand()?;
                macro_rules! vreduce {
                    ($variant:ident) => { Instruction::$variant { result, vector } };
                }
                Ok(match opcode {
                    "vreduce_add" => vreduce!(VreduceAdd),
                    "vreduce_min" => vreduce!(VreduceMin),
                    "vreduce_max" => vreduce!(VreduceMax),
                    _ => unreachable!(),
                })
            }
            "vload" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                self.consume(Token::Comma);
                let addr = self.parse_addr()?;
                Ok(Instruction::Vload { result, addr })
            }
            "vgather" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                self.consume(Token::Comma);
                let addr = self.parse_addr()?;
                self.expect(Token::Comma)?;
                let mask = self.parse_operand()?;
                Ok(Instruction::Vgather { result, addr, mask })
            }
            "sext" | "zext" | "trunc" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let from_type = self.parse_type()?;
                let value = self.parse_operand()?;
                macro_rules! conv_from {
                    ($variant:ident) => { Instruction::$variant { result, value, from_type } };
                }
                Ok(match opcode {
                    "sext" => conv_from!(Sext), "zext" => conv_from!(Zext),
                    "trunc" => conv_from!(Trunc),
                    _ => unreachable!(),
                })
            }
            "bitcast" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let to_type = self.parse_type()?;
                let value = self.parse_operand()?;
                Ok(Instruction::Bitcast { result, value, to_type })
            }
            "sitofp" | "uitofp" | "fptosi" | "fptoui" | "fpext" | "fptrunc" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let value = self.parse_operand()?;
                macro_rules! conv {
                    ($variant:ident) => { Instruction::$variant { result, value } };
                }
                Ok(match opcode {
                    "sitofp" => conv!(Sitofp), "uitofp" => conv!(Uitofp),
                    "fptosi" => conv!(Fptosi), "fptoui" => conv!(Fptoui),
                    "fpext" => conv!(Fpext), "fptrunc" => conv!(Fptrunc),
                    _ => unreachable!(),
                })
            }
            "cpuid" => Ok(Instruction::Cpuid { result }),
            "select" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let cond = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let true_val = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let false_val = self.parse_operand()?;
                Ok(Instruction::Select { result, cond, true_val, false_val })
            }
            "phi" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let incoming = self.parse_phi_incoming()?;
                Ok(Instruction::Phi { result, incoming })
            }
            "div" | "rem" | "mulh" => {
                let ty = self.parse_type()?;
                let result = self.update_vreg_type(result, ty.clone());
                let lhs = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let rhs = self.parse_operand()?;
                let flags_result = self.new_vreg(IrType::Flags);
                macro_rules! bin_arith {
                    ($variant:ident) => { Instruction::$variant { result, lhs, rhs, flags_result } };
                }
                Ok(match opcode {
                    "div" => bin_arith!(Div), "rem" => bin_arith!(Rem),
                    "mulh" => bin_arith!(Mulh),
                    _ => unreachable!(),
                })
            }
            _ => Err(self.error(format!("unknown one-result opcode '{}'", opcode))),
        }
    }

    // -- No-result instructions ----------------------------------------------

    fn parse_no_result_inst(&mut self, opcode: &str) -> ParseResult<Instruction> {
        match opcode {
            "store" => {
                let _ty = self.parse_type()?;
                let value = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let addr = self.parse_addr()?;
                Ok(Instruction::Store { value, addr })
            }
            "storei" => {
                let _ty = self.parse_type()?;
                let value = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let base = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let offset = self.expect_number()?;
                Ok(Instruction::Storei { value, base, offset })
            }
            "mem_add" | "mem_sub" | "mem_and" | "mem_or" | "mem_xor" => {
                let addr = self.parse_addr()?;
                self.expect(Token::Comma)?;
                let value = self.parse_operand()?;
                macro_rules! cmem {
                    ($variant:ident) => { Instruction::$variant { addr, value } };
                }
                Ok(match opcode {
                    "mem_add" => cmem!(MemAdd), "mem_sub" => cmem!(MemSub),
                    "mem_and" => cmem!(MemAnd), "mem_or" => cmem!(MemOr),
                    "mem_xor" => cmem!(MemXor),
                    _ => unreachable!(),
                })
            }
            "atomic_add" => {
                let addr = self.parse_addr()?;
                self.expect(Token::Comma)?;
                let value = self.parse_operand()?;
                Ok(Instruction::AtomicMemAdd { addr, value })
            }
            "push" => {
                let _ty = self.parse_type()?;
                let value = self.parse_operand()?;
                Ok(Instruction::Push { value })
            }
            "enter" => {
                let frame_size = self.expect_number()?;
                Ok(Instruction::Enter { frame_size })
            }
            "leave" => Ok(Instruction::Leave),
            "br" => {
                let pos = self.advance();
                let target = self.text_at(pos).to_string();
                Ok(Instruction::Br { target_bb: target })
            }
            "br_cond" => {
                let cond = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let p1 = self.advance();
                let true_bb = self.text_at(p1).to_string();
                self.expect(Token::Comma)?;
                let p2 = self.advance();
                let false_bb = self.text_at(p2).to_string();
                Ok(Instruction::BrCond { cond, true_bb, false_bb })
            }
            "switch" => {
                let value = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let pos = self.advance();
                let default_bb = self.text_at(pos).to_string();
                self.expect(Token::LBracket)?;
                let mut cases = Vec::new();
                while !self.matches_kind(Token::RBracket) {
                    let case_val = self.expect_number()?;
                    self.expect(Token::Colon)?;
                    self.expect(Token::Percent)?;
                    let pos = self.advance();
                    let case_bb = self.text_at(pos).to_string();
                    cases.push((Value::ConstInt { value: case_val, ty: IrType::I64 }, case_bb));
                    if !self.matches_kind(Token::RBracket) {
                        self.expect(Token::Comma)?;
                    }
                }
                self.expect(Token::RBracket)?;
                Ok(Instruction::Switch { value, default_bb, cases })
            }
            "ret" => {
                if self.matches_ident("void") {
                    self.advance();
                    Ok(Instruction::Ret { value: None })
                } else if self.is_type_or_keyword() {
                    let _ty = self.parse_type()?;
                    let value = self.parse_operand()?;
                    Ok(Instruction::Ret { value: Some(value) })
                } else {
                    Ok(Instruction::Ret { value: None })
                }
            }
            "call" => {
                self.expect(Token::At)?;
                let pos = self.advance();
                let callee_name = self.text_at(pos).to_string();
                self.expect(Token::LParen)?;
                let args = self.parse_arg_list()?;
                Ok(Instruction::Call { result: None, callee_name, args })
            }
            "call_indirect" => {
                let fnptr = self.parse_operand()?;
                self.expect(Token::LParen)?;
                let args = self.parse_arg_list()?;
                Ok(Instruction::CallIndirect { result: None, fnptr, args })
            }
            "tail_call" => {
                self.expect(Token::At)?;
                let pos = self.advance();
                let callee_name = self.text_at(pos).to_string();
                self.expect(Token::LParen)?;
                let args = self.parse_arg_list()?;
                Ok(Instruction::TailCall { callee_name, args })
            }
            "vstore" => {
                let _ty = self.parse_type()?;
                let value = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let addr = self.parse_addr()?;
                Ok(Instruction::Vstore { value, addr })
            }
            "vscatter" => {
                let _ty = self.parse_type()?;
                let value = self.parse_operand()?;
                self.expect(Token::Comma)?;
                let addr = self.parse_addr()?;
                self.expect(Token::Comma)?;
                let mask = self.parse_operand()?;
                Ok(Instruction::Vscatter { value, addr, mask })
            }
            "syscall" => Ok(Instruction::Syscall),
            "int" => {
                let vector = self.expect_number()?;
                Ok(Instruction::Int { vector })
            }
            "fence" => Ok(Instruction::Fence),
            "bkpt" => Ok(Instruction::Bkpt),
            "hlt" => Ok(Instruction::Hlt),
            "cli" => Ok(Instruction::Cli),
            "sti" => Ok(Instruction::Sti),
            "nop" => Ok(Instruction::Nop),
            _ => Err(self.error(format!("unknown no-result opcode '{}'", opcode))),
        }
    }

    fn parse_arg_list(&mut self) -> ParseResult<Vec<Value>> {
        let mut args = Vec::new();
        if !self.matches_kind(Token::RParen) {
            args.push(self.parse_operand()?);
            while self.consume(Token::Comma) {
                args.push(self.parse_operand()?);
            }
        }
        self.expect(Token::RParen)?;
        Ok(args)
    }

    fn parse_phi_incoming(&mut self) -> ParseResult<Vec<(Value, String)>> {
        let mut incoming = Vec::new();
        self.expect(Token::LBracket)?;
        let val = self.parse_operand()?;
        self.expect(Token::Comma)?;
        if self.consume(Token::Percent) {
            let pos = self.advance();
            incoming.push((val, self.text_at(pos).to_string()));
        } else {
            let pos = self.advance();
            incoming.push((val, self.text_at(pos).to_string()));
        }
        self.expect(Token::RBracket)?;
        while self.consume(Token::Comma) {
            self.expect(Token::LBracket)?;
            let val = self.parse_operand()?;
            self.expect(Token::Comma)?;
            if self.consume(Token::Percent) {
                let pos = self.advance();
                incoming.push((val, self.text_at(pos).to_string()));
            } else {
                let pos = self.advance();
                incoming.push((val, self.text_at(pos).to_string()));
            }
            self.expect(Token::RBracket)?;
        }
        Ok(incoming)
    }

    fn update_vreg_type(&mut self, value: Value, ty: IrType) -> Value {
        if let Value::VReg { name, ty: old_ty } = &value {
            if old_ty == &IrType::Void {
                let new_vreg = Value::VReg { name: name.clone(), ty };
                self.vreg_table.insert(name.clone(), new_vreg.clone());
                return new_vreg;
            }
        }
        value
    }

    // =========================================================================
    //  Top-level parsing
    // =========================================================================

    pub fn parse_module(&mut self) -> ParseResult<Module> {
        let mut module = Module::new(self.source_name.clone());

        while self.pos < self.tokens.len() {
            match self.cur_kind() {
                Token::Ident => {
                    let text = self.cur_text();
                    match text {
                        "global" => {
                            let gv = self.parse_global()?;
                            module.globals.push(gv);
                        }
                        "func" => {
                            let func = self.parse_function()?;
                            module.functions.push(func);
                        }
                        _ => {
                            return Err(self.error(format!("unexpected '{}' at top level", text)));
                        }
                    }
                }
                _ => {
                    return Err(self.error(format!(
                        "unexpected token {:?} at top level",
                        self.cur_kind()
                    )));
                }
            }
        }

        Ok(module)
    }

    fn parse_global(&mut self) -> ParseResult<Value> {
        self.advance(); // consume "global"
        self.expect(Token::At)?;
        let pos = self.advance();
        let name = self.text_at(pos).to_string();
        self.expect(Token::Colon)?;
        let ty = self.parse_type()?;

        if self.consume(Token::Eq) {
            let _init = self.parse_operand()?;
        }

        Ok(Value::GlobalVar { name, ty })
    }

    fn parse_function(&mut self) -> ParseResult<Function> {
        self.advance(); // consume "func"
        self.expect(Token::At)?;
        let pos = self.advance();
        let name = self.text_at(pos).to_string();
        self.expect(Token::LParen)?;

        let mut params: Vec<(String, IrType)> = Vec::new();
        if !self.matches_kind(Token::RParen) {
            let ppos = self.advance();
            let param_name = self.text_at(ppos).to_string();
            self.expect(Token::Colon)?;
            let param_type = self.parse_type()?;
            params.push((param_name, param_type));
            while self.consume(Token::Comma) {
                let ppos = self.advance();
                let param_name = self.text_at(ppos).to_string();
                self.expect(Token::Colon)?;
                let param_type = self.parse_type()?;
                params.push((param_name, param_type));
            }
        }
        self.expect(Token::RParen)?;

        let mut return_type = IrType::Void;
        if self.consume(Token::Arrow) {
            return_type = self.parse_type()?;
        }

        let mut call_conv = "nova".to_string();
        while self.matches_kind(Token::At) {
            self.advance();
            let apos = self.advance();
            let ann_name = self.text_at(apos).to_string();
            if ann_name == "callconv" {
                self.expect(Token::LParen)?;
                let cpos = self.advance();
                call_conv = self.text_at(cpos).to_string();
                self.expect(Token::RParen)?;
            }
        }

        let mut func = Function::new(name, call_conv);
        func.return_type = return_type;

        self.current_params.clear();
        self.vreg_table.clear();
        self.func_vreg_counter = 0;
        for (i, (pname, ptype)) in params.iter().enumerate() {
            let fp = Value::FuncParam {
                name: format!("%{}", pname),
                ty: ptype.clone(),
                index: i,
            };
            func.parameters.push(fp.clone());
            self.current_params.insert(format!("%{}", pname), fp);
        }

        self.expect(Token::LBrace)?;

        while !self.matches_kind(Token::RBrace) {
            let bb = self.parse_bb()?;
            if func.basic_blocks.is_empty() {
                let mut bb = bb;
                bb.is_entry = true;
                func.basic_blocks.push(bb);
            } else {
                func.basic_blocks.push(bb);
            }
        }

        self.expect(Token::RBrace)?;

        compute_predecessors(&mut func);

        self.current_params.clear();
        Ok(func)
    }

    fn parse_bb(&mut self) -> ParseResult<BasicBlock> {
        let pos = self.advance();
        let bb_name = self.text_at(pos).to_string();
        self.expect(Token::Colon)?;

        let mut bb = BasicBlock::new(bb_name);

        while self.pos < self.tokens.len() && !self.matches_kind(Token::RBrace) {
            if let Token::Ident = self.cur_kind() {
                if self.pos + 1 < self.tokens.len()
                    && matches!(self.tokens[self.pos + 1].kind, Token::Colon)
                {
                    break;
                }
            }
            let inst = self.parse_instruction()?;
            bb.add_instruction(inst);
        }

        Ok(bb)
    }
}

// =============================================================================
//  Helpers
// =============================================================================

fn matches_type_name(name: &str) -> bool {
    if name == "void" || name == "ptr" || name == "flags" {
        return true;
    }
    if let Some(rest) = name.strip_prefix('v') {
        if let Some(pos) = rest.find(|c: char| c == 'i' || c == 'f') {
            let count_str = &rest[..pos];
            let type_str = &rest[pos..];
            if count_str.parse::<u32>().is_ok() && type_str.len() >= 2 {
                let prefix = &type_str[0..1];
                let width_str = &type_str[1..];
                if (prefix == "i" || prefix == "f") && width_str.parse::<u32>().is_ok() {
                    return true;
                }
            }
        }
        return false;
    }
    if name.len() >= 2 {
        let prefix = &name[0..1];
        let rest = &name[1..];
        if (prefix == "i" || prefix == "f") && rest.parse::<u32>().is_ok() {
            return true;
        }
    }
    false
}

fn parse_type_name(name: &str) -> Option<IrType> {
    match name {
        "void" => Some(IrType::Void),
        "ptr" => Some(IrType::Ptr),
        "flags" => Some(IrType::Flags),
        "i1" => Some(IrType::I1),
        "i8" => Some(IrType::I8),
        "i16" => Some(IrType::I16),
        "i32" => Some(IrType::I32),
        "i64" => Some(IrType::I64),
        "f32" => Some(IrType::F32),
        "f64" => Some(IrType::F64),
        _ => {
            if let Some(rest) = name.strip_prefix('v') {
                if let Some(pos) = rest.find(|c: char| c == 'i' || c == 'f') {
                    let count_str = &rest[..pos];
                    let type_str = &rest[pos..];
                    let count = count_str.parse::<u32>().ok()?;
                    let elem = parse_type_name(type_str)?;
                    return Some(IrType::Vector(Box::new(elem), count));
                }
            }
            None
        }
    }
}

fn compute_predecessors(func: &mut Function) {
    for bb in &mut func.basic_blocks {
        bb.predecessors.clear();
    }
    let mut bb_map: HashMap<String, usize> = HashMap::new();
    for (i, bb) in func.basic_blocks.iter().enumerate() {
        bb_map.insert(bb.name.clone(), i);
    }
    for i in 0..func.basic_blocks.len() {
        let bb_name = func.basic_blocks[i].name.clone();
        let successors = func.basic_blocks[i].successors.clone();
        for succ_name in &successors {
            if let Some(&succ_idx) = bb_map.get(succ_name) {
                let preds = &mut func.basic_blocks[succ_idx].predecessors;
                if !preds.contains(&bb_name) {
                    preds.push(bb_name.clone());
                }
            }
        }
    }
}

// =============================================================================
//  Public API
// =============================================================================

/// Parse NIR source text into a `Module`.
pub fn parse(source: &str, source_name: &str) -> Result<Module, ParseError> {
    let mut parser = Parser::new(source, source_name.to_string());
    parser.parse_module()
}

/// Parse a `.nir` file into a `Module`.
pub fn parse_file(path: &str) -> Result<Module, Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;
    Ok(parse(&source, path)?)
}