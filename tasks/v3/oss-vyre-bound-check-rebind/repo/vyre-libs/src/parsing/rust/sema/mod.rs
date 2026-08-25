//! Rust semantic analysis: name resolution, type checking, borrow checking.
//!
//! Reusable substrate (Tier 3), mirroring `vyre-libs::parsing::c::sema`. The
//! algorithms live here so any consumer can run typed Rust analysis without
//! depending on the `vyre-frontend-rust` driver crate. The driver orchestrates.
//!
//! Analyses are side tables over the borrowed AST: a pass is
//! `pass(&Module, &Resolution) -> Result<...>` and never clones the AST. This
//! keeps the pipeline allocation-lean and extends cleanly as the compiler grows
//! (new passes add new side tables, not new copies of the program).
//!
//! `resolve` and `typeck` are implemented for the nano-subset. `borrow_check`
//! implements the mutability rule (E0596) and reports the conflicting-borrow
//! rules (E0499/E0502) as not-yet-wired rather than faking a complete pass.

use std::collections::HashMap;

use thiserror::Error;

use super::lex::tokens::{ANDAND, EQ, GE, GT, LE, LT, MINUS, NE, OROR, PERCENT, PLUS, SLASH, STAR};
use super::parse::{Expr, Module, Stmt, Type};

/// Stable id for a resolved binding (index into [`Resolution::bindings`]).
pub type BindingId = usize;

/// A resolved binding: a function parameter or a `let` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    /// Recovered identifier text.
    pub name: String,
    /// Whether the binding was declared `mut`.
    pub mutable: bool,
    /// Declared type.
    pub ty: Type,
    /// Source byte offset of the defining identifier.
    pub def_offset: u32,
    /// Index of the enclosing function in `Module::functions`.
    pub function: usize,
}

/// Name-resolution result: a binding table plus a use -> binding map over the
/// AST. Holds no copy of the AST; analyses borrow the `Module` alongside it.
#[derive(Debug, Clone, Default)]
pub struct Resolution {
    /// Every parameter and `let` binding, in declaration order.
    pub bindings: Vec<Binding>,
    /// Map from each variable-use source offset to its resolved binding.
    pub uses: HashMap<u32, BindingId>,
    /// Map from each call's name source offset to the callee function index.
    pub calls: HashMap<u32, usize>,
}

/// Errors from the Rust semantic-analysis stages.
#[derive(Debug, Clone, Error)]
pub enum RustSemaError {
    /// A variable use did not resolve to any in-scope binding (rustc E0425).
    #[error("cannot find value `{name}` in this scope (byte {offset})")]
    UnresolvedName {
        /// The unresolved identifier text.
        name: String,
        /// Source byte offset of the use.
        offset: u32,
    },
    /// A call referenced a function that is not defined (rustc E0425).
    #[error("cannot find function `{name}` in this scope (byte {offset})")]
    UnknownFunction {
        /// The unresolved function name.
        name: String,
        /// Source byte offset of the call.
        offset: u32,
    },
    /// A `&mut` borrow targeted an immutable place (rustc E0596).
    #[error("cannot borrow `{name}` as mutable, as it is not declared as mutable (byte {offset})")]
    CannotBorrowImmutableAsMutable {
        /// The immutable binding being borrowed mutably.
        name: String,
        /// Source byte offset of the borrowed place.
        offset: u32,
    },
    /// A function returned a reference to a call-local value (rustc E0597).
    #[error(
        "cannot return a reference to a local value; it does not live long enough (byte {offset})"
    )]
    ReturnsReferenceToLocal {
        /// Source byte offset of the offending place.
        offset: u32,
    },
    /// Two mutable borrows of the same place are live simultaneously (rustc E0499).
    #[error("cannot borrow as mutable more than once at a time (byte {offset})")]
    MultipleMutableBorrows {
        /// Source byte offset of the conflicting borrow.
        offset: u32,
    },
    /// A mutable and a shared borrow of the same place are live at once (rustc E0502).
    #[error("cannot borrow as mutable because it is also borrowed as immutable (byte {offset})")]
    MutableAndSharedBorrow {
        /// Source byte offset of the conflicting borrow.
        offset: u32,
    },
    /// An expression's type did not match the expected type (rustc E0308).
    #[error("mismatched types in {context}: expected `{expected}`, found `{found}`")]
    TypeMismatch {
        /// Where the mismatch occurred (let binding, return, operand, argument).
        context: String,
        /// The expected type.
        expected: String,
        /// The type actually found.
        found: String,
    },
    /// A dereference was applied to a non-reference type (rustc E0614).
    #[error("type `{found}` cannot be dereferenced; only references can")]
    CannotDeref {
        /// The non-reference type.
        found: String,
    },
    /// An `if` condition was not `bool` (rustc E0308).
    #[error("`if` condition must be `bool`, found `{found}`")]
    NonBooleanCondition {
        /// The non-boolean condition type.
        found: String,
    },
    /// A call passed the wrong number of arguments (rustc E0061).
    #[error("function `{function}` expects {expected} argument(s), found {found}")]
    ArgCountMismatch {
        /// The called function name.
        function: String,
        /// The declared parameter count.
        expected: usize,
        /// The supplied argument count.
        found: usize,
    },
    /// A non-unit function body does not return on all paths (rustc E0308).
    #[error("function `{function}` must return `{expected}` on all paths")]
    MissingReturn {
        /// The function name.
        function: String,
        /// The declared return type.
        expected: String,
    },
    /// Assignment to a binding not declared `mut` (rustc E0384).
    #[error("cannot assign twice to immutable variable `{name}`")]
    AssignToImmutable {
        /// The immutable binding being assigned.
        name: String,
    },
    /// Internal pass invariant failed.
    #[error("internal Rust semantic invariant failed: {message}")]
    InternalInvariant {
        /// Actionable invariant diagnostic.
        message: String,
    },
}

/// Recover the identifier text that begins at `offset` in `source`.
///
/// The AST stores only the start offset of each identifier, so resolution and
/// type checking re-scan the contiguous identifier bytes (`[A-Za-z0-9_]`).
fn ident_at(source: &[u8], offset: u32) -> String {
    let start = (offset as usize).min(source.len());
    let mut end = start;
    while end < source.len() && (source[end].is_ascii_alphanumeric() || source[end] == b'_') {
        end += 1;
    }
    String::from_utf8_lossy(&source[start..end]).into_owned()
}

/// Whether a value of type `found` is accepted where `expected` is required,
/// allowing rustc's `&mut T -> &T` reference coercion (top-level only; the
/// pointee type must match exactly, as references are invariant in it). A shared
/// `&T` never coerces to `&mut T`.
fn coerces(found: &Type, expected: &Type) -> bool {
    match (found, expected) {
        (
            Type::Ref {
                mutable: fm,
                inner: fi,
            },
            Type::Ref {
                mutable: em,
                inner: ei,
            },
        ) => (fm == em || (*fm && !*em)) && fi == ei,
        _ => found == expected,
    }
}

/// Render a type for diagnostics, matching Rust surface syntax.
fn type_str(ty: &Type) -> String {
    match ty {
        Type::I32 => "i32".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Unit => "()".to_string(),
        Type::Ref { mutable, inner } => {
            format!("&{}{}", if *mutable { "mut " } else { "" }, type_str(inner))
        }
    }
}

// ----------------------------------------------------------------------------
// Name resolution
// ----------------------------------------------------------------------------

struct Resolver<'a> {
    source: &'a [u8],
    fn_index: &'a HashMap<String, usize>,
    bindings: Vec<Binding>,
    uses: HashMap<u32, BindingId>,
    calls: HashMap<u32, usize>,
    scopes: Vec<HashMap<String, BindingId>>,
    function: usize,
}

impl Resolver<'_> {
    fn declare(
        &mut self,
        name: String,
        mutable: bool,
        ty: Type,
        def_offset: u32,
    ) -> Result<(), RustSemaError> {
        let id = self.bindings.len();
        self.bindings.push(Binding {
            name: name.clone(),
            mutable,
            ty,
            def_offset,
            function: self.function,
        });
        let scope = self
            .scopes
            .last_mut()
            .ok_or_else(|| RustSemaError::InternalInvariant {
                message:
                    "resolver scope stack was empty while declaring a binding; seed the function scope before resolution"
                        .to_string(),
            })?;
        scope.insert(name, id);
        Ok(())
    }

    fn lookup(&self, name: &str) -> Option<BindingId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|frame| frame.get(name).copied())
    }

    fn resolve_expr(&mut self, expr: &Expr) -> Result<(), RustSemaError> {
        match expr {
            Expr::LiteralInt(..) | Expr::LiteralBool(..) => Ok(()),
            Expr::Var(offset) => {
                let name = ident_at(self.source, *offset);
                match self.lookup(&name) {
                    Some(id) => {
                        self.uses.insert(*offset, id);
                        Ok(())
                    }
                    None => Err(RustSemaError::UnresolvedName {
                        name,
                        offset: *offset,
                    }),
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.resolve_expr(lhs)?;
                self.resolve_expr(rhs)
            }
            Expr::Borrow { expr, .. } => self.resolve_expr(expr),
            Expr::Deref(inner) => self.resolve_expr(inner),
            Expr::Not(inner) => self.resolve_expr(inner),
            Expr::Neg(inner) => self.resolve_expr(inner),
            Expr::Call { name, args } => {
                let fname = ident_at(self.source, *name);
                match self.fn_index.get(&fname) {
                    Some(&idx) => {
                        self.calls.insert(*name, idx);
                    }
                    None => {
                        return Err(RustSemaError::UnknownFunction {
                            name: fname,
                            offset: *name,
                        })
                    }
                }
                for arg in args {
                    self.resolve_expr(arg)?;
                }
                Ok(())
            }
            Expr::Block(stmts) => {
                self.scopes.push(HashMap::new());
                let result = self.resolve_block(stmts);
                self.scopes.pop();
                result
            }
            Expr::If {
                cond,
                then_block,
                else_block,
            } => {
                self.resolve_expr(cond)?;
                self.resolve_expr(then_block)?;
                if let Some(else_block) = else_block {
                    self.resolve_expr(else_block)?;
                }
                Ok(())
            }
        }
    }

    fn resolve_block(&mut self, stmts: &[Stmt]) -> Result<(), RustSemaError> {
        for stmt in stmts {
            match stmt {
                Stmt::Let {
                    mutable,
                    name,
                    ty,
                    init,
                } => {
                    self.resolve_expr(init)?;
                    let recovered = ident_at(self.source, *name);
                    self.declare(recovered, *mutable, ty.clone(), *name)?;
                }
                Stmt::Expr(expr) => self.resolve_expr(expr)?,
                Stmt::Assign { name, value } => {
                    self.resolve_expr(value)?;
                    let n = ident_at(self.source, *name);
                    match self.lookup(&n) {
                        Some(id) => {
                            self.uses.insert(*name, id);
                        }
                        None => {
                            return Err(RustSemaError::UnresolvedName {
                                name: n,
                                offset: *name,
                            })
                        }
                    }
                }
                Stmt::Return(Some(expr)) => self.resolve_expr(expr)?,
                Stmt::Return(None) => {}
                Stmt::While { cond, body } => {
                    self.resolve_expr(cond)?;
                    self.scopes.push(HashMap::new());
                    let r = self.resolve_block(body);
                    self.scopes.pop();
                    r?;
                }
                Stmt::For {
                    name,
                    start,
                    end,
                    body,
                } => {
                    self.resolve_expr(start)?;
                    self.resolve_expr(end)?;
                    self.scopes.push(HashMap::new());
                    let recovered = ident_at(self.source, *name);
                    self.declare(recovered, false, Type::I32, *name)?;
                    let r = self.resolve_block(body);
                    self.scopes.pop();
                    r?;
                }
            }
        }
        Ok(())
    }
}

/// Resolve names in a parsed module against its `source`.
///
/// Recovers identifier text from source offsets, tracks lexical scope
/// (parameters plus block-nested `let`, with shadowing), maps every variable
/// use to its binding, and rejects uses of names not in scope (rustc E0425).
///
/// # Errors
/// Returns [`RustSemaError::UnresolvedName`] or [`RustSemaError::UnknownFunction`]
/// for a use with no in-scope definition.
pub fn resolve(module: &Module, source: &[u8]) -> Result<Resolution, RustSemaError> {
    let fn_index: HashMap<String, usize> = module
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| (ident_at(source, f.name), i))
        .collect();

    let mut resolver = Resolver {
        source,
        fn_index: &fn_index,
        bindings: Vec::new(),
        uses: HashMap::new(),
        calls: HashMap::new(),
        scopes: Vec::new(),
        function: 0,
    };

    for (index, func) in module.functions.iter().enumerate() {
        resolver.function = index;
        resolver.scopes = vec![HashMap::new()];
        for (offset, ty) in &func.params {
            let name = ident_at(source, *offset);
            resolver.declare(name, false, ty.clone(), *offset)?;
        }
        resolver.resolve_block(&func.body)?;
    }

    Ok(Resolution {
        bindings: resolver.bindings,
        uses: resolver.uses,
        calls: resolver.calls,
    })
}

// ----------------------------------------------------------------------------
// Type checking
// ----------------------------------------------------------------------------

struct FnSig {
    params: Vec<Type>,
    ret: Type,
}

struct TypeCk<'a> {
    source: &'a [u8],
    resolution: &'a Resolution,
    sigs: &'a HashMap<String, FnSig>,
    ret: &'a Type,
}

impl TypeCk<'_> {
    fn type_of(&self, expr: &Expr) -> Result<Type, RustSemaError> {
        match expr {
            Expr::LiteralInt(..) => Ok(Type::I32),
            Expr::LiteralBool(..) => Ok(Type::Bool),
            Expr::Var(offset) => {
                let id = *self.resolution.uses.get(offset).ok_or_else(|| {
                    RustSemaError::InternalInvariant {
                        message: format!(
                            "resolve did not record variable use at byte {offset} before typeck"
                        ),
                    }
                })?;
                Ok(self.resolution.bindings[id].ty.clone())
            }
            Expr::Binary { op, lhs, rhs } => {
                let lt = self.type_of(lhs)?;
                let rt = self.type_of(rhs)?;
                match *op {
                    PLUS | MINUS | STAR | SLASH | PERCENT => {
                        self.require(&lt, &Type::I32, "arithmetic operand")?;
                        self.require(&rt, &Type::I32, "arithmetic operand")?;
                        Ok(Type::I32)
                    }
                    LT | GT | LE | GE => {
                        self.require(&lt, &Type::I32, "comparison operand")?;
                        self.require(&rt, &Type::I32, "comparison operand")?;
                        Ok(Type::Bool)
                    }
                    EQ | NE => {
                        if lt != rt {
                            return Err(RustSemaError::TypeMismatch {
                                context: "equality operands".to_string(),
                                expected: type_str(&lt),
                                found: type_str(&rt),
                            });
                        }
                        Ok(Type::Bool)
                    }
                    ANDAND | OROR => {
                        self.require(&lt, &Type::Bool, "logical operand")?;
                        self.require(&rt, &Type::Bool, "logical operand")?;
                        Ok(Type::Bool)
                    }
                    _ => Ok(Type::I32),
                }
            }
            Expr::Borrow { mutable, expr } => {
                let inner = self.type_of(expr)?;
                Ok(Type::Ref {
                    mutable: *mutable,
                    inner: Box::new(inner),
                })
            }
            Expr::Deref(inner) => match self.type_of(inner)? {
                Type::Ref { inner, .. } => Ok(*inner),
                other => Err(RustSemaError::CannotDeref {
                    found: type_str(&other),
                }),
            },
            Expr::Not(inner) => {
                let it = self.type_of(inner)?;
                self.require(&it, &Type::Bool, "logical-not operand")?;
                Ok(Type::Bool)
            }
            Expr::Neg(inner) => {
                let it = self.type_of(inner)?;
                self.require(&it, &Type::I32, "arithmetic-negation operand")?;
                Ok(Type::I32)
            }
            Expr::Call { name, args } => {
                let fname = ident_at(self.source, *name);
                let sig = self
                    .sigs
                    .get(&fname)
                    .ok_or(RustSemaError::UnknownFunction {
                        name: fname.clone(),
                        offset: *name,
                    })?;
                if args.len() != sig.params.len() {
                    return Err(RustSemaError::ArgCountMismatch {
                        function: fname,
                        expected: sig.params.len(),
                        found: args.len(),
                    });
                }
                for (arg, param_ty) in args.iter().zip(&sig.params) {
                    let at = self.type_of(arg)?;
                    self.require(&at, param_ty, "function argument")?;
                }
                Ok(sig.ret.clone())
            }
            Expr::Block(stmts) => {
                self.check_block(stmts)?;
                Ok(Type::Unit)
            }
            Expr::If {
                cond,
                then_block,
                else_block,
            } => {
                let ct = self.type_of(cond)?;
                if ct != Type::Bool {
                    return Err(RustSemaError::NonBooleanCondition {
                        found: type_str(&ct),
                    });
                }
                let tt = self.type_of(then_block)?;
                let et = match else_block {
                    Some(else_block) => self.type_of(else_block)?,
                    None => Type::Unit,
                };
                if tt != et {
                    return Err(RustSemaError::TypeMismatch {
                        context: "if/else branches".to_string(),
                        expected: type_str(&tt),
                        found: type_str(&et),
                    });
                }
                Ok(tt)
            }
        }
    }

    fn require(&self, found: &Type, expected: &Type, context: &str) -> Result<(), RustSemaError> {
        if coerces(found, expected) {
            Ok(())
        } else {
            Err(RustSemaError::TypeMismatch {
                context: context.to_string(),
                expected: type_str(expected),
                found: type_str(found),
            })
        }
    }

    fn check_block(&self, stmts: &[Stmt]) -> Result<(), RustSemaError> {
        for stmt in stmts {
            match stmt {
                Stmt::Let { ty, init, .. } => {
                    let it = self.type_of(init)?;
                    self.require(&it, ty, "let binding")?;
                }
                Stmt::Expr(expr) => {
                    self.type_of(expr)?;
                }
                Stmt::Assign { name, value } => {
                    let id = self.resolution.uses[name];
                    let (mutable, target_ty, bname) = {
                        let b = &self.resolution.bindings[id];
                        (b.mutable, b.ty.clone(), b.name.clone())
                    };
                    if !mutable {
                        return Err(RustSemaError::AssignToImmutable { name: bname });
                    }
                    let vt = self.type_of(value)?;
                    self.require(&vt, &target_ty, "assignment")?;
                }
                Stmt::Return(Some(expr)) => {
                    let rt = self.type_of(expr)?;
                    self.require(&rt, self.ret, "return value")?;
                }
                Stmt::Return(None) => {
                    self.require(&Type::Unit, self.ret, "return value")?;
                }
                Stmt::While { cond, body } => {
                    let ct = self.type_of(cond)?;
                    if ct != Type::Bool {
                        return Err(RustSemaError::NonBooleanCondition {
                            found: type_str(&ct),
                        });
                    }
                    self.check_block(body)?;
                }
                Stmt::For {
                    start, end, body, ..
                } => {
                    let st = self.type_of(start)?;
                    self.require(&st, &Type::I32, "for range start")?;
                    let et = self.type_of(end)?;
                    self.require(&et, &Type::I32, "for range end")?;
                    self.check_block(body)?;
                }
            }
        }
        Ok(())
    }
}

/// Type-check a resolved module against its `source` (rustc E0308 / E0061 / E0614).
///
/// Checks `let` initializer types, return-value types, binary-operator operand
/// types, dereference of references only, boolean `if` conditions, call arity
/// and argument types, and that a non-unit function returns on all paths.
///
/// # Errors
/// Returns the matching [`RustSemaError`] variant on a type error.
pub fn typeck(
    module: &Module,
    source: &[u8],
    resolution: &Resolution,
) -> Result<(), RustSemaError> {
    let sigs: HashMap<String, FnSig> = module
        .functions
        .iter()
        .map(|f| {
            (
                ident_at(source, f.name),
                FnSig {
                    params: f.params.iter().map(|(_, t)| t.clone()).collect(),
                    ret: f.ret.clone(),
                },
            )
        })
        .collect();

    for func in &module.functions {
        let ck = TypeCk {
            source,
            resolution,
            sigs: &sigs,
            ret: &func.ret,
        };
        ck.check_block(&func.body)?;
        if func.ret != Type::Unit && !block_diverges(&func.body) {
            return Err(RustSemaError::MissingReturn {
                function: ident_at(source, func.name),
                expected: type_str(&func.ret),
            });
        }
    }
    Ok(())
}

/// Whether a statement sequence is guaranteed to return on all paths.
fn block_diverges(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_diverges)
}

fn stmt_diverges(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) => true,
        Stmt::Expr(expr) => expr_diverges(expr),
        Stmt::Assign { .. } => false,
        Stmt::While { .. } => false,
        Stmt::For { .. } => false,
        Stmt::Let { init, .. } => expr_diverges(init),
    }
}

fn expr_diverges(expr: &Expr) -> bool {
    match expr {
        Expr::Block(stmts) => block_diverges(stmts),
        Expr::If {
            then_block,
            else_block: Some(else_block),
            ..
        } => expr_diverges(then_block) && expr_diverges(else_block),
        _ => false,
    }
}

// ----------------------------------------------------------------------------
// Borrow checking
// ----------------------------------------------------------------------------

/// Borrow-check a resolved module: the nano-subset verdict.
///
/// Runs mutability (E0596) via [`check_mutability`], dangling-reference / escape
/// (E0597) via [`check_escape`], and conflicting borrows (E0499/E0502) via
/// [`check_conflicts`], which lowers each function to a CFG and runs the NLL
/// loan-liveness engine (branches and through-deref reborrows included). On the
/// nano-subset this verdict is accept/reject-identical to rustc; the
/// `rust_sema_borrow_oracle` differential gates that agreement against a real
/// rustc over generated straight-line, branch, and reborrow programs.
///
/// # Errors
/// Returns the matching [`RustSemaError`] for an E0596/E0597/E0499/E0502 violation.
pub fn borrow_check(module: &Module, resolution: &Resolution) -> Result<(), RustSemaError> {
    check_mutability(module, resolution)?;
    check_escape(module, resolution)?;
    check_conflicts(module, resolution)?;
    Ok(())
}

/// Check the mutability borrow rule (rustc E0596) over a resolved module.
///
/// A `&mut` borrow is rejected when the borrowed place is an immutable binding
/// or a dereference of a shared (`&T`) reference. Borrowing a temporary is
/// allowed.
///
/// # Errors
/// Returns [`RustSemaError::CannotBorrowImmutableAsMutable`] on a violation.
pub fn check_mutability(module: &Module, resolution: &Resolution) -> Result<(), RustSemaError> {
    for func in &module.functions {
        check_mut_stmts(&func.body, resolution)?;
    }
    Ok(())
}

fn check_mut_stmts(stmts: &[Stmt], resolution: &Resolution) -> Result<(), RustSemaError> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { init, .. } => check_mut_expr(init, resolution)?,
            Stmt::Expr(expr) => check_mut_expr(expr, resolution)?,
            Stmt::Assign { value, .. } => check_mut_expr(value, resolution)?,
            Stmt::While { cond, body } => {
                check_mut_expr(cond, resolution)?;
                check_mut_stmts(body, resolution)?;
            }
            Stmt::For {
                start, end, body, ..
            } => {
                check_mut_expr(start, resolution)?;
                check_mut_expr(end, resolution)?;
                check_mut_stmts(body, resolution)?;
            }
            Stmt::Return(Some(expr)) => check_mut_expr(expr, resolution)?,
            Stmt::Return(None) => {}
        }
    }
    Ok(())
}

fn check_mut_expr(expr: &Expr, resolution: &Resolution) -> Result<(), RustSemaError> {
    match expr {
        Expr::Borrow { mutable, expr } => {
            if *mutable {
                check_mutable_place(expr, resolution)?;
            }
            check_mut_expr(expr, resolution)
        }
        Expr::Binary { lhs, rhs, .. } => {
            check_mut_expr(lhs, resolution)?;
            check_mut_expr(rhs, resolution)
        }
        Expr::Deref(inner) => check_mut_expr(inner, resolution),
        Expr::Not(inner) => check_mut_expr(inner, resolution),
        Expr::Neg(inner) => check_mut_expr(inner, resolution),
        Expr::Call { args, .. } => {
            for arg in args {
                check_mut_expr(arg, resolution)?;
            }
            Ok(())
        }
        Expr::Block(stmts) => check_mut_stmts(stmts, resolution),
        Expr::If {
            cond,
            then_block,
            else_block,
        } => {
            check_mut_expr(cond, resolution)?;
            check_mut_expr(then_block, resolution)?;
            if let Some(else_block) = else_block {
                check_mut_expr(else_block, resolution)?;
            }
            Ok(())
        }
        Expr::LiteralInt(..) | Expr::LiteralBool(..) | Expr::Var(..) => Ok(()),
    }
}

/// Verify that `place` denotes a mutable place for a `&mut` borrow.
fn check_mutable_place(place: &Expr, resolution: &Resolution) -> Result<(), RustSemaError> {
    match place {
        Expr::Var(offset) => {
            if let Some(&id) = resolution.uses.get(offset) {
                let binding = &resolution.bindings[id];
                if !binding.mutable {
                    return Err(RustSemaError::CannotBorrowImmutableAsMutable {
                        name: binding.name.clone(),
                        offset: *offset,
                    });
                }
            }
            Ok(())
        }
        Expr::Deref(inner) => {
            if let Expr::Var(offset) = inner.as_ref() {
                if let Some(&id) = resolution.uses.get(offset) {
                    let binding = &resolution.bindings[id];
                    if let Type::Ref { mutable: false, .. } = binding.ty {
                        return Err(RustSemaError::CannotBorrowImmutableAsMutable {
                            name: binding.name.clone(),
                            offset: *offset,
                        });
                    }
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Check that no function returns a reference to a call-local value (rustc E0597).
///
/// A reference escapes only if it borrows a binding's storage (`&x`, for any
/// local binding) or transitively points at such a borrow. A reference derived
/// from a `&T` parameter's pointee (`return r`, `&*r`) is allowed. The check
/// never rejects a reference whose provenance it cannot prove local, so it never
/// diverges from rustc on acceptance.
///
/// # Errors
/// Returns [`RustSemaError::ReturnsReferenceToLocal`] when a returned reference
/// provably borrows a local value.
pub fn check_escape(module: &Module, resolution: &Resolution) -> Result<(), RustSemaError> {
    let def_to_id: HashMap<u32, BindingId> = resolution
        .bindings
        .iter()
        .enumerate()
        .map(|(id, b)| (b.def_offset, id))
        .collect();

    for func in &module.functions {
        let returns_ref = matches!(func.ret, Type::Ref { .. });
        // A reference's borrows-local provenance: true if the reference value
        // ultimately points at a call-local binding's storage. Parameters point
        // outside the call, so they start false.
        let mut borrows_local: HashMap<BindingId, bool> = HashMap::new();
        for (offset, _ty) in &func.params {
            if let Some(&id) = def_to_id.get(offset) {
                borrows_local.insert(id, false);
            }
        }
        walk_escape(
            &func.body,
            returns_ref,
            &def_to_id,
            resolution,
            &mut borrows_local,
        )?;
    }
    Ok(())
}

fn walk_escape(
    stmts: &[Stmt],
    returns_ref: bool,
    def_to_id: &HashMap<u32, BindingId>,
    resolution: &Resolution,
    borrows_local: &mut HashMap<BindingId, bool>,
) -> Result<(), RustSemaError> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, ty, init, .. } => {
                if let Some(&id) = def_to_id.get(name) {
                    let escapes = matches!(ty, Type::Ref { .. })
                        && escapes_offset(init, resolution, borrows_local).is_some();
                    borrows_local.insert(id, escapes);
                }
                descend_escape(init, returns_ref, def_to_id, resolution, borrows_local)?;
            }
            Stmt::Expr(expr) => {
                descend_escape(expr, returns_ref, def_to_id, resolution, borrows_local)?;
            }
            Stmt::Assign { value, .. } => {
                descend_escape(value, returns_ref, def_to_id, resolution, borrows_local)?;
            }
            Stmt::While { cond, body } => {
                descend_escape(cond, returns_ref, def_to_id, resolution, borrows_local)?;
                walk_escape(body, returns_ref, def_to_id, resolution, borrows_local)?;
            }
            Stmt::For {
                start, end, body, ..
            } => {
                descend_escape(start, returns_ref, def_to_id, resolution, borrows_local)?;
                descend_escape(end, returns_ref, def_to_id, resolution, borrows_local)?;
                walk_escape(body, returns_ref, def_to_id, resolution, borrows_local)?;
            }
            Stmt::Return(Some(expr)) => {
                if returns_ref {
                    if let Some(offset) = escapes_offset(expr, resolution, borrows_local) {
                        return Err(RustSemaError::ReturnsReferenceToLocal { offset });
                    }
                }
                descend_escape(expr, returns_ref, def_to_id, resolution, borrows_local)?;
            }
            Stmt::Return(None) => {}
        }
    }
    Ok(())
}

fn descend_escape(
    expr: &Expr,
    returns_ref: bool,
    def_to_id: &HashMap<u32, BindingId>,
    resolution: &Resolution,
    borrows_local: &mut HashMap<BindingId, bool>,
) -> Result<(), RustSemaError> {
    match expr {
        Expr::Block(stmts) => walk_escape(stmts, returns_ref, def_to_id, resolution, borrows_local),
        Expr::If {
            cond,
            then_block,
            else_block,
        } => {
            descend_escape(cond, returns_ref, def_to_id, resolution, borrows_local)?;
            descend_escape(
                then_block,
                returns_ref,
                def_to_id,
                resolution,
                borrows_local,
            )?;
            if let Some(else_block) = else_block {
                descend_escape(
                    else_block,
                    returns_ref,
                    def_to_id,
                    resolution,
                    borrows_local,
                )?;
            }
            Ok(())
        }
        Expr::Binary { lhs, rhs, .. } => {
            descend_escape(lhs, returns_ref, def_to_id, resolution, borrows_local)?;
            descend_escape(rhs, returns_ref, def_to_id, resolution, borrows_local)
        }
        Expr::Borrow { expr, .. } => {
            descend_escape(expr, returns_ref, def_to_id, resolution, borrows_local)
        }
        Expr::Deref(inner) => {
            descend_escape(inner, returns_ref, def_to_id, resolution, borrows_local)
        }
        Expr::Not(inner) => {
            descend_escape(inner, returns_ref, def_to_id, resolution, borrows_local)
        }
        Expr::Neg(inner) => {
            descend_escape(inner, returns_ref, def_to_id, resolution, borrows_local)
        }
        Expr::Call { args, .. } => {
            for arg in args {
                descend_escape(arg, returns_ref, def_to_id, resolution, borrows_local)?;
            }
            Ok(())
        }
        Expr::Var(..) | Expr::LiteralInt(..) | Expr::LiteralBool(..) => Ok(()),
    }
}

/// If the reference value `expr` provably borrows a call-local binding, return
/// the source offset of the offending place; otherwise `None`.
fn escapes_offset(
    expr: &Expr,
    resolution: &Resolution,
    borrows_local: &HashMap<BindingId, bool>,
) -> Option<u32> {
    match expr {
        Expr::Borrow { expr, .. } => match expr.as_ref() {
            // `&x` borrows the storage of a call-local binding; it escapes.
            Expr::Var(offset) => Some(*offset),
            // `&*r` points at r's pointee; it escapes iff that pointee is local.
            Expr::Deref(inner) => {
                if let Expr::Var(offset) = inner.as_ref() {
                    let id = resolution.uses.get(offset)?;
                    if *borrows_local.get(id).unwrap_or(&false) {
                        Some(*offset)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        },
        // Returning a reference binding escapes iff it holds a local borrow.
        Expr::Var(offset) => {
            let id = resolution.uses.get(offset)?;
            if *borrows_local.get(id).unwrap_or(&false) {
                Some(*offset)
            } else {
                None
            }
        }
        _ => None,
    }
}

// ----------------------------------------------------------------------------
// Conflicting borrows (E0499 / E0502)
// ----------------------------------------------------------------------------

/// Conflicting-borrow rules (rustc E0499 / E0502).
///
/// Lowers each function to neutral [`crate::borrowck::BorrowFacts`] (a CFG with
/// loan issue/use points, branches included) and runs the front-end-agnostic
/// engine, which computes NLL loan liveness as a CFG dataflow. Correct across
/// control flow: borrows live across a branch point conflict; borrows confined
/// to mutually exclusive branches do not. Only direct `&[mut] x` borrows are
/// tracked as loans today; through-deref and nested-block borrows are a sound
/// gap (never false-rejects).
///
/// # Errors
/// Returns [`RustSemaError::MultipleMutableBorrows`] (E0499) or
/// [`RustSemaError::MutableAndSharedBorrow`] (E0502) on a detected conflict.
pub fn check_conflicts(module: &Module, resolution: &Resolution) -> Result<(), RustSemaError> {
    use crate::borrowck::{analyze, ConflictKind};

    let def_to_id: HashMap<u32, BindingId> = resolution
        .bindings
        .iter()
        .enumerate()
        .map(|(id, b)| (b.def_offset, id))
        .collect();

    for func in &module.functions {
        let facts = build_borrow_facts(func, resolution, &def_to_id);
        if let Some(conflict) = analyze(&facts).into_iter().next() {
            return Err(match conflict.kind {
                ConflictKind::TwoMutable => RustSemaError::MultipleMutableBorrows {
                    offset: conflict.offset,
                },
                ConflictKind::MutableAndShared => RustSemaError::MutableAndSharedBorrow {
                    offset: conflict.offset,
                },
            });
        }
    }
    Ok(())
}

/// Lower one function body to neutral borrow facts: a CFG over program points,
/// the `&[mut] x` loans, and each loan's use points.
fn build_borrow_facts(
    func: &super::parse::Function,
    resolution: &Resolution,
    def_to_id: &HashMap<u32, BindingId>,
) -> crate::borrowck::BorrowFacts {
    let mut builder = FactBuilder {
        resolution,
        def_to_id,
        facts: crate::borrowck::BorrowFacts::default(),
        binding_to_loan: HashMap::new(),
    };
    builder.build_block(&func.body, &[]);
    builder.facts
}

struct FactBuilder<'a> {
    resolution: &'a Resolution,
    def_to_id: &'a HashMap<u32, BindingId>,
    facts: crate::borrowck::BorrowFacts,
    binding_to_loan: HashMap<BindingId, crate::borrowck::Loan>,
}

impl FactBuilder<'_> {
    fn alloc_point(&mut self) -> u32 {
        let point = self.facts.point_count;
        self.facts.point_count += 1;
        point
    }

    /// Build the CFG for `stmts`; `preds` are the points flowing in. Returns the
    /// points flowing out (empty if every path returns).
    fn build_block(&mut self, stmts: &[Stmt], preds: &[u32]) -> Vec<u32> {
        let mut cur: Vec<u32> = preds.to_vec();
        for stmt in stmts {
            let point = self.alloc_point();
            for &pred in &cur {
                self.facts.cfg_edges.push((pred, point));
            }
            match stmt {
                Stmt::Let { name, ty, init, .. } => {
                    self.record_uses(init, point);
                    self.record_loan(name, ty, init, point);
                    cur = vec![point];
                }
                Stmt::Return(value) => {
                    if let Some(expr) = value {
                        self.record_uses(expr, point);
                    }
                    cur = Vec::new();
                }
                Stmt::Assign { value, .. } => {
                    self.record_uses(value, point);
                    cur = vec![point];
                }
                Stmt::While { cond, body } => {
                    self.record_uses(cond, point);
                    let out = self.build_block(body, &[point]);
                    for &b in &out {
                        self.facts.cfg_edges.push((b, point));
                    }
                    cur = vec![point];
                }
                Stmt::For {
                    start, end, body, ..
                } => {
                    self.record_uses(start, point);
                    self.record_uses(end, point);
                    let out = self.build_block(body, &[point]);
                    for &b in &out {
                        self.facts.cfg_edges.push((b, point));
                    }
                    cur = vec![point];
                }
                Stmt::Expr(Expr::If {
                    cond,
                    then_block,
                    else_block,
                }) => {
                    self.record_uses(cond, point);
                    let mut out = self.build_block(block_stmts(then_block), &[point]);
                    match else_block {
                        Some(else_block) => {
                            out.extend(self.build_block(block_stmts(else_block), &[point]))
                        }
                        None => out.push(point),
                    }
                    cur = out;
                }
                Stmt::Expr(expr) => {
                    self.record_uses(expr, point);
                    cur = vec![point];
                }
            }
        }
        cur
    }

    /// Record a loan for a borrow-introducing `let`:
    /// - `let a = &[mut] x;` borrows place `x`;
    /// - `let a = &[mut] *r;` reborrows through `r` (place `r`);
    /// - `let a: &[mut] T = r;` is a reborrow coercion (the grammar requires the
    ///   annotation, so it is never a move): it reborrows `*r` with the let
    ///   type's mutability, place `r`.
    ///
    /// In the nano-subset a reborrow conflicts exactly when another borrow of
    /// the same `r` is live, matching rustc.
    fn record_loan(&mut self, name: &u32, ty: &Type, init: &Expr, point: u32) {
        let (place_off, mutable) = match init {
            Expr::Borrow { mutable, expr } => {
                let off = match expr.as_ref() {
                    Expr::Var(off) => Some(*off),
                    Expr::Deref(inner) => match inner.as_ref() {
                        Expr::Var(off) => Some(*off),
                        _ => None,
                    },
                    _ => None,
                };
                match off {
                    Some(off) => (off, *mutable),
                    None => return,
                }
            }
            Expr::Var(off) => match ty {
                Type::Ref { mutable, .. } => (*off, *mutable),
                _ => return,
            },
            _ => return,
        };
        if let (Some(&place), Some(&binding)) = (
            self.resolution.uses.get(&place_off),
            self.def_to_id.get(name),
        ) {
            let loan = self.facts.loan_place.len() as crate::borrowck::Loan;
            self.facts.loan_place.push(place as crate::borrowck::Place);
            self.facts.loan_kind.push(if mutable {
                crate::borrowck::LoanKind::Mut
            } else {
                crate::borrowck::LoanKind::Shared
            });
            self.facts.loan_issued_at.push(point);
            self.facts.loan_offset.push(*name);
            self.binding_to_loan.insert(binding, loan);
        }
    }

    /// Record uses of loan bindings in `expr` at `point` (not descending into
    /// nested `if`/block bodies, which carry their own points).
    fn record_uses(&mut self, expr: &Expr, point: u32) {
        let mut used = Vec::new();
        collect_expr_uses(expr, self.resolution, &mut used);
        for binding in used {
            if let Some(&loan) = self.binding_to_loan.get(&binding) {
                self.facts.loan_used_at.push((loan, point));
            }
        }
    }
}

/// Statement list of a block expression (empty for anything else).
fn block_stmts(expr: &Expr) -> &[Stmt] {
    match expr {
        Expr::Block(stmts) => stmts,
        _ => &[],
    }
}

fn collect_expr_uses(expr: &Expr, resolution: &Resolution, into: &mut Vec<BindingId>) {
    match expr {
        Expr::Var(off) => {
            if let Some(&id) = resolution.uses.get(off) {
                into.push(id);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_uses(lhs, resolution, into);
            collect_expr_uses(rhs, resolution, into);
        }
        Expr::Borrow { expr, .. } => collect_expr_uses(expr, resolution, into),
        Expr::Deref(inner) => collect_expr_uses(inner, resolution, into),
        Expr::Not(inner) => collect_expr_uses(inner, resolution, into),
        Expr::Neg(inner) => collect_expr_uses(inner, resolution, into),
        Expr::Call { args, .. } => {
            for arg in args {
                collect_expr_uses(arg, resolution, into);
            }
        }
        // Do not descend into branch bodies: uses inside `if` blocks are not
        // counted, keeping the straight-line check sound (never false-rejects).
        Expr::Block(..) | Expr::If { .. } | Expr::LiteralInt(..) | Expr::LiteralBool(..) => {}
    }
}
