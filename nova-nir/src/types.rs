//! NIR type system for the MacroCore-X compiler.

use std::fmt;
use thiserror::Error;

/// NIR error types.
#[derive(Error, Debug)]
pub enum NirError {
    /// Invalid scale in address expression.
    #[error("invalid scale {0}, must be 1, 2, 4, or 8")]
    InvalidScale(i32),
}

/// NIR intermediate representation types.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IrType {
    /// 1-bit integer (boolean).
    I1,
    /// 8-bit integer.
    I8,
    /// 16-bit integer.
    I16,
    /// 32-bit integer.
    I32,
    /// 64-bit integer.
    I64,
    /// 32-bit floating-point.
    F32,
    /// 64-bit floating-point.
    F64,
    /// Pointer type.
    Ptr,
    /// Flags / condition-codes type.
    Flags,
    /// Void type (no value).
    Void,
    /// Vector type: N consecutive elements of the same type.
    Vector(Box<IrType>, u32),
    /// Struct type with named or unnamed fields.
    Struct(Vec<IrType>),
    /// Function type: parameter types and return type.
    Function(Vec<IrType>, Box<IrType>),
}

impl fmt::Display for IrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrType::I1 => write!(f, "i1"),
            IrType::I8 => write!(f, "i8"),
            IrType::I16 => write!(f, "i16"),
            IrType::I32 => write!(f, "i32"),
            IrType::I64 => write!(f, "i64"),
            IrType::F32 => write!(f, "f32"),
            IrType::F64 => write!(f, "f64"),
            IrType::Ptr => write!(f, "ptr"),
            IrType::Flags => write!(f, "flags"),
            IrType::Void => write!(f, "void"),
            IrType::Vector(elem, count) => write!(f, "v{count}{elem}"),
            IrType::Struct(fields) => {
                write!(f, "{{")?;
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{field}")?;
                }
                write!(f, "}}")
            }
            IrType::Function(params, ret) => {
                write!(f, "(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{param}")?;
                }
                write!(f, ") -> {ret}")
            }
        }
    }
}

/// A value in the IR – virtual register, constant, global, or parameter.
#[derive(Clone, Debug)]
pub enum Value {
    /// Virtual register (SSA value).
    VReg {
        /// Register name (e.g. "%0", "%1").
        name: String,
        /// The type of the register.
        ty: IrType,
    },
    /// Compile-time integer constant.
    ConstInt {
        /// The constant value.
        value: i64,
        /// The integer type.
        ty: IrType,
    },
    /// Compile-time floating-point constant.
    ConstFloat {
        /// The constant value.
        value: f64,
        /// The floating-point type.
        ty: IrType,
    },
    /// Reference to a global variable.
    GlobalVar {
        /// Variable name (without @ prefix).
        name: String,
        /// The type of the global.
        ty: IrType,
    },
    /// Function parameter.
    FuncParam {
        /// Parameter name.
        name: String,
        /// The type of the parameter.
        ty: IrType,
        /// Parameter index (0-based).
        index: usize,
    },
}

impl Value {
    /// Return the type of this value.
    pub fn ty(&self) -> &IrType {
        match self {
            Value::VReg { ty, .. }
            | Value::ConstInt { ty, .. }
            | Value::ConstFloat { ty, .. }
            | Value::GlobalVar { ty, .. }
            | Value::FuncParam { ty, .. } => ty,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::VReg { name, .. } => write!(f, "{name}"),
            Value::ConstInt { value, .. } => write!(f, "{value}"),
            Value::ConstFloat { value, .. } => write!(f, "{value}"),
            Value::GlobalVar { name, .. } => write!(f, "@{name}"),
            Value::FuncParam { name, .. } => write!(f, "{name}"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::VReg { name: n1, .. }, Value::VReg { name: n2, .. }) => n1 == n2,
            (Value::ConstInt { value: v1, ty: t1 }, Value::ConstInt { value: v2, ty: t2 }) => {
                v1 == v2 && t1 == t2
            }
            (
                Value::ConstFloat { value: v1, ty: t1 },
                Value::ConstFloat { value: v2, ty: t2 },
            ) => v1 == v2 && t1 == t2,
            (Value::GlobalVar { name: n1, .. }, Value::GlobalVar { name: n2, .. }) => n1 == n2,
            (Value::FuncParam { name: n1, index: i1, .. }, Value::FuncParam { name: n2, index: i2, .. }) => {
                n1 == n2 && i1 == i2
            }
            _ => false,
        }
    }
}

impl Eq for Value {}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Value::VReg { name, .. } => name.hash(state),
            Value::ConstInt { value, ty } => {
                value.hash(state);
                ty.hash(state);
            }
            Value::ConstFloat { value, ty } => {
                value.to_bits().hash(state);
                ty.hash(state);
            }
            Value::GlobalVar { name, .. } => name.hash(state),
            Value::FuncParam { name, index, .. } => {
                name.hash(state);
                index.hash(state);
            }
        }
    }
}