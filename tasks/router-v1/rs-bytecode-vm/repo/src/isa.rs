//! The shared contract between the compiler and the VM.
//!
//! Nothing in this file needs to change to make the hidden tests pass.

use std::collections::HashMap;
use std::fmt;

/// A runtime value. Truthiness follows the usual rules: zero, `false` and the
/// empty string are falsy, everything else is truthy.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Bool(bool),
    Str(String),
}

impl Value {
    pub fn truthy(&self) -> bool {
        match self {
            Value::Num(n) => *n != 0.0,
            Value::Bool(b) => *b,
            Value::Str(s) => !s.is_empty(),
        }
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Num(n)
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Value::Num(n as f64)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}

/// Binary operators of the source language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Lt,
    Gt,
}

/// The abstract syntax tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// A literal value.
    Lit(Value),
    /// Read a variable from the environment.
    Var(String),
    /// Logical negation; always produces a `Bool`.
    Not(Box<Expr>),
    /// A binary operation.
    Bin(BinOp, Box<Expr>, Box<Expr>),
    /// Short-circuit `and`: the left operand if it is falsy, else the right.
    And(Box<Expr>, Box<Expr>),
    /// Short-circuit `or`: the left operand if it is truthy, else the right.
    Or(Box<Expr>, Box<Expr>),
    /// `if cond { then } else { otherwise }`; only the taken branch runs.
    If(Box<Expr>, Box<Expr>, Box<Expr>),
}

impl Expr {
    pub fn lit(v: impl Into<Value>) -> Expr {
        Expr::Lit(v.into())
    }
    pub fn var(name: &str) -> Expr {
        Expr::Var(name.to_string())
    }
    pub fn not(a: Expr) -> Expr {
        Expr::Not(Box::new(a))
    }
    pub fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
        Expr::Bin(op, Box::new(a), Box::new(b))
    }
    pub fn and(a: Expr, b: Expr) -> Expr {
        Expr::And(Box::new(a), Box::new(b))
    }
    pub fn or(a: Expr, b: Expr) -> Expr {
        Expr::Or(Box::new(a), Box::new(b))
    }
    pub fn if_(c: Expr, t: Expr, e: Expr) -> Expr {
        Expr::If(Box::new(c), Box::new(t), Box::new(e))
    }
}

/// One instruction of the flat program. Jump operands are ABSOLUTE indices
/// into the instruction list.
#[derive(Clone, Debug, PartialEq)]
pub enum Instr {
    /// Push a value.
    Push(Value),
    /// Push `env[name]`; `VmError::UnboundVariable` if it is not bound.
    Load(String),
    /// Pop `b`, pop `a`, push `a + b` (numbers only).
    Add,
    Sub,
    Mul,
    /// Pop `b`, pop `a`, push `a / b`; `VmError::DivisionByZero` if `b == 0`.
    Div,
    /// Pop `b`, pop `a`, push `a == b` as a `Bool`.
    Eq,
    /// Pop `b`, pop `a`, push `a < b` (numbers only).
    Lt,
    /// Pop `b`, pop `a`, push `a > b` (numbers only).
    Gt,
    /// Pop `a`, push `!truthy(a)`.
    Not,
    /// Duplicate the top of the stack.
    Dup,
    /// Discard the top of the stack.
    Pop,
    /// Set the instruction pointer to `target`.
    Jump(usize),
    /// Pop `a`; if it is falsy, set the instruction pointer to `target`.
    JumpIfFalse(usize),
}

/// Variable bindings for one run.
pub type Env = HashMap<String, Value>;

/// Everything that can go wrong while running a program.
#[derive(Clone, Debug, PartialEq)]
pub enum VmError {
    UnboundVariable(String),
    DivisionByZero,
    TypeError(&'static str),
    StackUnderflow,
    /// The program finished with a stack that did not hold exactly one value.
    BadFinalStack(usize),
    /// A jump target outside the program.
    BadJump(usize),
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::UnboundVariable(n) => write!(f, "unbound variable `{n}`"),
            VmError::DivisionByZero => write!(f, "division by zero"),
            VmError::TypeError(what) => write!(f, "type error: {what}"),
            VmError::StackUnderflow => write!(f, "stack underflow"),
            VmError::BadFinalStack(n) => write!(f, "program left {n} values on the stack"),
            VmError::BadJump(t) => write!(f, "jump target {t} is outside the program"),
        }
    }
}

impl std::error::Error for VmError {}
