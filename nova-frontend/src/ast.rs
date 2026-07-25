/// Top-level program: a sequence of items.
#[derive(Debug, PartialEq, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

/// A top-level item in a Nova source file.
#[derive(Debug, PartialEq, Clone)]
pub enum Item {
    Function(Function),
    Struct(StructDef),
    Enum(EnumDef),
    Union(UnionDef),
    Impl(ImplBlock),
    Mod(ModDecl),
    Use(UseDecl),
    ExternBlock(ExternBlock),
}

/// Visibility modifier.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Visibility {
    Public,
    Private,
}

/// An attribute (e.g., `#[inline]`).
#[derive(Debug, PartialEq, Clone)]
pub struct Attribute {
    pub name: String,
    pub args: Vec<String>,
}

/// A function definition.
#[derive(Debug, PartialEq, Clone)]
pub struct Function {
    pub vis: Visibility,
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Option<Block>,
    pub attrs: Vec<Attribute>,
    pub generics: Vec<GenericParam>,
}

/// A function parameter.
#[derive(Debug, PartialEq, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

/// A struct definition.
#[derive(Debug, PartialEq, Clone)]
pub struct StructDef {
    pub vis: Visibility,
    pub name: String,
    pub fields: Vec<Field>,
    pub generics: Vec<GenericParam>,
    pub attrs: Vec<Attribute>,
}

/// An enum definition.
#[derive(Debug, PartialEq, Clone)]
pub struct EnumDef {
    pub vis: Visibility,
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub generics: Vec<GenericParam>,
    pub attrs: Vec<Attribute>,
}

/// An enum variant.
#[derive(Debug, PartialEq, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub data: Option<Type>,
}

/// A union definition.
#[derive(Debug, PartialEq, Clone)]
pub struct UnionDef {
    pub vis: Visibility,
    pub name: String,
    pub fields: Vec<Field>,
    pub generics: Vec<GenericParam>,
    pub attrs: Vec<Attribute>,
}

/// An impl block.
#[derive(Debug, PartialEq, Clone)]
pub struct ImplBlock {
    pub generics: Vec<GenericParam>,
    pub target: Type,
    pub items: Vec<Item>,
}

/// A module declaration (`mod foo;`).
#[derive(Debug, PartialEq, Clone)]
pub struct ModDecl {
    pub vis: Visibility,
    pub name: String,
    pub attrs: Vec<Attribute>,
}

/// A use declaration (`use some::path;`).
#[derive(Debug, PartialEq, Clone)]
pub struct UseDecl {
    pub vis: Visibility,
    pub path: Vec<String>,
    pub attrs: Vec<Attribute>,
}

/// An extern block.
#[derive(Debug, PartialEq, Clone)]
pub struct ExternBlock {
    pub vis: Visibility,
    pub abi: Option<String>,
    pub items: Vec<Item>,
    pub attrs: Vec<Attribute>,
}

/// A struct/enum/union field.
#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

/// A generic parameter (e.g., `T` in `fn foo<T>()`).
#[derive(Debug, PartialEq, Clone)]
pub struct GenericParam {
    pub name: String,
}

/// A block of statements.
#[derive(Debug, PartialEq, Clone)]
pub struct Block {
    pub statements: Vec<Statement>,
}

/// A statement.
#[derive(Debug, PartialEq, Clone)]
pub enum Statement {
    Let {
        mutable: bool,
        name: String,
        ty: Option<Type>,
        init: Option<Expr>,
    },
    Expr(Expr),
    Return(Option<Expr>),
    If {
        cond: Expr,
        then_branch: Block,
        else_branch: Option<Box<Statement>>,
    },
    While {
        cond: Expr,
        body: Block,
    },
    For {
        var: String,
        iter: Expr,
        body: Block,
    },
    Loop {
        body: Block,
    },
    Break,
    Continue,
    Unsafe(Block),
    Asm(String),
    Defer(Box<Statement>),
}

/// A range expression used in for-loops.
#[derive(Debug, PartialEq, Clone)]
pub enum RangeExpr {
    Range { start: Box<Expr>, end: Box<Expr> },
    RangeInclusive { start: Box<Expr>, end: Box<Expr> },
}

/// An expression.
#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Ident(String),
    IntLiteral(i64),
    FloatLiteral(f64),
    BoolLiteral(bool),
    StringLiteral(String),
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
    },
    FieldAccess {
        expr: Box<Expr>,
        field: String,
    },
    Index {
        expr: Box<Expr>,
        index: Box<Expr>,
    },
    Cast {
        expr: Box<Expr>,
        ty: Type,
    },
    Block(Block),
    IfExpr {
        cond: Box<Expr>,
        then_branch: Block,
        else_branch: Option<Box<Expr>>,
    },
    StructLit {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    ArrayLit(Vec<Expr>),
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },
    Sizeof(Type),
    Alignof(Type),
    Ref {
        mutable: bool,
        expr: Box<Expr>,
    },
    Deref(Box<Expr>),
    Self_,
    Assign {
        target: Box<Expr>,
        op: Option<BinOp>,
        value: Box<Expr>,
    },
}

/// A type expression.
#[derive(Debug, PartialEq, Clone)]
pub enum Type {
    Named(String),
    Ptr(Box<Type>),
    MutPtr(Box<Type>),
    ConstPtr(Box<Type>),
    Array(Box<Type>, usize),
    Fn(Vec<Type>, Option<Box<Type>>),
    Generic(String),
}

/// Binary operators.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BinOp {
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
    Assign,
}

/// Unary operators.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum UnaryOp {
    Neg,
    Not,
    Deref,
    Ref,
    RefMut,
}