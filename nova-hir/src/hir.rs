/// HIR (High-level Intermediate Representation) for Nova.
///
/// The HIR is a typed, resolved representation of Nova source code.
/// It is produced by lowering the AST and running type checking.
use std::fmt;

/// Calling convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallConv {
    Nova,
    C,
}

impl fmt::Display for CallConv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallConv::Nova => write!(f, "nova"),
            CallConv::C => write!(f, "c"),
        }
    }
}

/// A complete HIR program.
#[derive(Debug, Clone)]
pub struct HirProgram {
    pub items: Vec<HirItem>,
}

/// A top-level HIR item.
#[derive(Debug, Clone)]
pub enum HirItem {
    Function(HirFunction),
    Struct(HirStruct),
    Enum(HirEnum),
    Union(HirUnion),
    ExternBlock(HirExternBlock),
}

/// A struct definition in HIR.
#[derive(Debug, Clone)]
pub struct HirStruct {
    pub name: String,
    pub fields: Vec<(String, HirType)>,
    pub is_pub: bool,
}

/// An enum definition in HIR.
#[derive(Debug, Clone)]
pub struct HirEnum {
    pub name: String,
    pub variants: Vec<(String, Option<HirType>)>,
    pub is_pub: bool,
}

/// A union definition in HIR.
#[derive(Debug, Clone)]
pub struct HirUnion {
    pub name: String,
    pub fields: Vec<(String, HirType)>,
    pub is_pub: bool,
}

/// An extern block in HIR.
#[derive(Debug, Clone)]
pub struct HirExternBlock {
    pub abi: String,
    pub functions: Vec<HirFunction>,
}

/// A function parameter.
#[derive(Debug, Clone)]
pub struct HirParam {
    pub name: String,
    pub ty: HirType,
}

/// A function definition in HIR.
#[derive(Debug, Clone)]
pub struct HirFunction {
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: HirType,
    pub body: HirBlock,
    pub call_conv: CallConv,
    pub is_pub: bool,
}

/// A block of statements with an optional tail expression.
#[derive(Debug, Clone)]
pub struct HirBlock {
    pub stmts: Vec<HirStmt>,
    pub expr: Option<HirExpr>,
}

/// A statement in HIR.
#[derive(Debug, Clone)]
pub enum HirStmt {
    Let {
        name: String,
        mutable: bool,
        ty: HirType,
        init: Option<HirExpr>,
    },
    Expr(HirExpr),
    Return(Option<HirExpr>),
    If {
        cond: HirExpr,
        then_block: HirBlock,
        else_block: Option<HirBlock>,
    },
    While {
        cond: HirExpr,
        body: HirBlock,
    },
    For {
        var: String,
        iter: HirForIter,
        body: HirBlock,
    },
    Loop {
        body: HirBlock,
    },
    Break,
    Continue,
    Assign {
        target: HirExpr,
        value: HirExpr,
    },
    Unsafe(HirBlock),
    Asm(String),
}

/// For loop iteration type.
#[derive(Debug, Clone)]
pub enum HirForIter {
    Range {
        start: HirExpr,
        end: HirExpr,
        inclusive: bool,
    },
    Array(HirExpr),
}

/// An expression in HIR (fully typed and resolved).
#[derive(Debug, Clone)]
pub enum HirExpr {
    Ident {
        name: String,
        ty: HirType,
    },
    IntLiteral {
        value: i64,
        ty: HirType,
    },
    FloatLiteral {
        value: f64,
        ty: HirType,
    },
    BoolLiteral {
        value: bool,
        ty: HirType,
    },
    Binary {
        left: Box<HirExpr>,
        op: HirBinOp,
        right: Box<HirExpr>,
        ty: HirType,
    },
    Unary {
        op: HirUnaryOp,
        expr: Box<HirExpr>,
        ty: HirType,
    },
    Call {
        func: Box<HirExpr>,
        args: Vec<HirExpr>,
        ty: HirType,
    },
    FieldAccess {
        expr: Box<HirExpr>,
        field: String,
        ty: HirType,
    },
    Index {
        expr: Box<HirExpr>,
        index: Box<HirExpr>,
        ty: HirType,
    },
    Cast {
        expr: Box<HirExpr>,
        ty: HirType,
    },
    StructLit {
        name: String,
        fields: Vec<(String, HirExpr)>,
        ty: HirType,
    },
    Deref {
        expr: Box<HirExpr>,
        ty: HirType,
    },
    Ref {
        expr: Box<HirExpr>,
        mutable: bool,
        ty: HirType,
    },
    Param {
        name: String,
        index: usize,
        ty: HirType,
    },
}

/// Binary operators in HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirBinOp {
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

impl fmt::Display for HirBinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            HirBinOp::Add => "+",
            HirBinOp::Sub => "-",
            HirBinOp::Mul => "*",
            HirBinOp::Div => "/",
            HirBinOp::Rem => "%",
            HirBinOp::And => "&",
            HirBinOp::Or => "|",
            HirBinOp::Xor => "^",
            HirBinOp::Shl => "<<",
            HirBinOp::Shr => ">>",
            HirBinOp::Eq => "==",
            HirBinOp::Ne => "!=",
            HirBinOp::Lt => "<",
            HirBinOp::Le => "<=",
            HirBinOp::Gt => ">",
            HirBinOp::Ge => ">=",
        };
        write!(f, "{s}")
    }
}

/// Unary operators in HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirUnaryOp {
    Neg,
    Not,
    Deref,
    Ref,
    RefMut,
}

impl fmt::Display for HirUnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            HirUnaryOp::Neg => "-",
            HirUnaryOp::Not => "!",
            HirUnaryOp::Deref => "*",
            HirUnaryOp::Ref => "&",
            HirUnaryOp::RefMut => "&mut",
        };
        write!(f, "{s}")
    }
}

/// Types in HIR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HirType {
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
    Ptr(Box<HirType>),
    Array(Box<HirType>, usize),
    Struct(String),
    Fn(Vec<HirType>, Box<HirType>),
}

impl fmt::Display for HirType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HirType::I8 => write!(f, "i8"),
            HirType::I16 => write!(f, "i16"),
            HirType::I32 => write!(f, "i32"),
            HirType::I64 => write!(f, "i64"),
            HirType::U8 => write!(f, "u8"),
            HirType::U16 => write!(f, "u16"),
            HirType::U32 => write!(f, "u32"),
            HirType::U64 => write!(f, "u64"),
            HirType::F32 => write!(f, "f32"),
            HirType::F64 => write!(f, "f64"),
            HirType::Bool => write!(f, "bool"),
            HirType::Void => write!(f, "void"),
            HirType::Ptr(inner) => write!(f, "*{inner}"),
            HirType::Array(inner, len) => write!(f, "[{inner}; {len}]"),
            HirType::Struct(name) => write!(f, "{name}"),
            HirType::Fn(params, ret) => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {ret}")
            }
        }
    }
}

/// Return the type of an HIR expression.
impl HirExpr {
    pub fn ty(&self) -> &HirType {
        match self {
            HirExpr::Ident { ty, .. }
            | HirExpr::IntLiteral { ty, .. }
            | HirExpr::FloatLiteral { ty, .. }
            | HirExpr::BoolLiteral { ty, .. }
            | HirExpr::Binary { ty, .. }
            | HirExpr::Unary { ty, .. }
            | HirExpr::Call { ty, .. }
            | HirExpr::FieldAccess { ty, .. }
            | HirExpr::Index { ty, .. }
            | HirExpr::Cast { ty, .. }
            | HirExpr::StructLit { ty, .. }
            | HirExpr::Deref { ty, .. }
            | HirExpr::Ref { ty, .. }
            | HirExpr::Param { ty, .. } => ty,
        }
    }
}