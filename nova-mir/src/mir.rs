/// MIR (Mid-level Intermediate Representation) for Nova.
///
/// MIR is a control-flow-graph-based IR with basic blocks,
/// explicit terminators, and temporary variables for intermediate values.
use std::fmt;

/// A complete MIR program.
#[derive(Debug, Clone)]
pub struct MirProgram {
    pub functions: Vec<MirFunction>,
}

/// A function in MIR.
#[derive(Debug, Clone)]
pub struct MirFunction {
    pub name: String,
    pub params: Vec<MirParam>,
    pub return_type: MirType,
    pub basic_blocks: Vec<MirBasicBlock>,
    pub call_conv: String,
}

/// A function parameter in MIR.
#[derive(Debug, Clone)]
pub struct MirParam {
    pub name: String,
    pub ty: MirType,
}

/// A basic block in MIR.
#[derive(Debug, Clone)]
pub struct MirBasicBlock {
    pub name: String,
    pub stmts: Vec<MirStmt>,
    pub terminator: MirTerminator,
    /// Predecessor block names (computed during lowering).
    pub predecessors: Vec<String>,
    /// Successor block names (computed during lowering).
    pub successors: Vec<String>,
    /// Whether this is the entry block.
    pub is_entry: bool,
}

/// A statement in MIR.
#[derive(Debug, Clone)]
pub enum MirStmt {
    Assign {
        dst: MirPlace,
        src: MirRvalue,
    },
    StorageLive(String),
    StorageDead(String),
}

/// A place (lvalue) in MIR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MirPlace {
    Var(String),
    Temp(usize),
    Deref(Box<MirPlace>),
    Field(Box<MirPlace>, String),
}

impl fmt::Display for MirPlace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MirPlace::Var(name) => write!(f, "%{name}"),
            MirPlace::Temp(n) => write!(f, "%t{n}"),
            MirPlace::Deref(p) => write!(f, "*{p}"),
            MirPlace::Field(p, field) => write!(f, "{p}.{field}"),
        }
    }
}

/// A constant value in MIR.
#[derive(Debug, Clone)]
pub enum MirConstant {
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

/// A right-hand side value in MIR.
#[derive(Debug, Clone)]
pub enum MirRvalue {
    Use(MirOperand),
    Constant(MirConstant),
    BinaryOp(MirBinOp, Box<MirOperand>, Box<MirOperand>),
    UnaryOp(MirUnaryOp, Box<MirOperand>),
    Call {
        func: String,
        args: Vec<MirOperand>,
    },
    Ref(Box<MirPlace>),
}

/// An operand (place + type) in MIR.
#[derive(Debug, Clone)]
pub struct MirOperand {
    pub place: MirPlace,
    pub ty: MirType,
}

/// A terminator instruction in MIR.
#[derive(Debug, Clone)]
pub enum MirTerminator {
    Return(Option<MirOperand>),
    Goto(String),
    If {
        cond: MirOperand,
        then_bb: String,
        else_bb: String,
    },
    Call {
        func: String,
        args: Vec<MirOperand>,
        dest: Option<MirPlace>,
        next_bb: String,
    },
    Unreachable,
}

/// Binary operators in MIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Unary operators in MIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirUnaryOp {
    Neg,
    Not,
}

/// Types in MIR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MirType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Bool,
    Void,
    Ptr(Box<MirType>),
    Array(Box<MirType>, usize),
    Struct(String),
}

impl fmt::Display for MirType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MirType::I8 => write!(f, "i8"),
            MirType::I16 => write!(f, "i16"),
            MirType::I32 => write!(f, "i32"),
            MirType::I64 => write!(f, "i64"),
            MirType::U8 => write!(f, "u8"),
            MirType::U16 => write!(f, "u16"),
            MirType::U32 => write!(f, "u32"),
            MirType::U64 => write!(f, "u64"),
            MirType::F32 => write!(f, "f32"),
            MirType::F64 => write!(f, "f64"),
            MirType::Bool => write!(f, "bool"),
            MirType::Void => write!(f, "void"),
            MirType::Ptr(inner) => write!(f, "*{inner}"),
            MirType::Array(inner, len) => write!(f, "[{inner}; {len}]"),
            MirType::Struct(name) => write!(f, "{name}"),
        }
    }
}