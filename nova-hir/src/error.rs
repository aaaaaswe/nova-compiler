/// HIR and type checking error types.
use thiserror::Error;

/// A type error with source location information.
#[derive(Error, Debug, Clone)]
pub enum TypeError {
    #[error("type mismatch: expected {expected}, found {found}")]
    TypeMismatch {
        expected: String,
        found: String,
        span: Option<(usize, usize)>,
    },

    #[error("undefined variable: {name}")]
    UndefinedVariable {
        name: String,
        span: Option<(usize, usize)>,
    },

    #[error("undefined function: {name}")]
    UndefinedFunction {
        name: String,
        span: Option<(usize, usize)>,
    },

    #[error("undefined type: {name}")]
    UndefinedType {
        name: String,
        span: Option<(usize, usize)>,
    },

    #[error("duplicate definition: {name}")]
    DuplicateDefinition {
        name: String,
        span: Option<(usize, usize)>,
    },

    #[error("invalid binary operation: {op} between {left} and {right}")]
    InvalidBinaryOp {
        op: String,
        left: String,
        right: String,
        span: Option<(usize, usize)>,
    },

    #[error("invalid unary operation: {op} on {ty}")]
    InvalidUnaryOp {
        op: String,
        ty: String,
        span: Option<(usize, usize)>,
    },

    #[error("cannot assign to immutable variable: {name}")]
    ImmutableAssignment {
        name: String,
        span: Option<(usize, usize)>,
    },

    #[error("return type mismatch: expected {expected}, found {found}")]
    ReturnTypeMismatch {
        expected: String,
        found: String,
        span: Option<(usize, usize)>,
    },

    #[error("break outside of loop")]
    BreakOutsideLoop {
        span: Option<(usize, usize)>,
    },

    #[error("continue outside of loop")]
    ContinueOutsideLoop {
        span: Option<(usize, usize)>,
    },

    #[error("cannot index into non-array type: {ty}")]
    InvalidIndex {
        ty: String,
        span: Option<(usize, usize)>,
    },

    #[error("cannot access field '{field}' on type: {ty}")]
    InvalidFieldAccess {
        field: String,
        ty: String,
        span: Option<(usize, usize)>,
    },

    #[error("cannot dereference non-pointer type: {ty}")]
    InvalidDeref {
        ty: String,
        span: Option<(usize, usize)>,
    },

    #[error("wrong number of arguments: expected {expected}, got {found}")]
    WrongArgCount {
        expected: usize,
        found: usize,
        span: Option<(usize, usize)>,
    },

    #[error("{0}")]
    Other(String),
}