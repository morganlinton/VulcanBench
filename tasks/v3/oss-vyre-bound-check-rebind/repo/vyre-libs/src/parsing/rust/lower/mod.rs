//! Rust AST to Vyre IR lowering (reusable substrate, Tier 3).
//!
//! Mirrors `vyre-libs::parsing::c::lower`: lowering a resolved language AST to a
//! `vyre::ir::Program` is a Tier-3 concern and lives in the library, not the
//! frontend driver. Lowering borrows the AST plus its resolution.
//!
//! ## Model (nano-subset)
//!
//! The last function in the module is the entry kernel. In scalar mode each
//! `i32`/`bool` parameter becomes a one-element `ReadOnly` input buffer and the
//! return value is stored into `out[0]`. In batched mode each parameter becomes
//! a same-length input buffer indexed by `global_id.x`, and the return value is
//! stored into `out[global_id.x]` behind an out-of-range guard. Local bindings
//! are alpha-renamed to `v{binding_id}` so Rust's legal shadowing never trips
//! Vyre's no-shadowing validator (V008). `if`/`else` whose arms both return is
//! a terminal statement. Anything outside the wired subset returns a loud
//! [`RustLowerError::Unsupported`] rather than miscompiling.

use std::collections::HashMap;

use thiserror::Error;
use vyre::ir::{BufferDecl, DataType, Expr as IrExpr, Node, Program};

use super::lex::tokens::{ANDAND, EQ, GE, GT, LE, LT, MINUS, NE, OROR, PERCENT, PLUS, SLASH, STAR};
use super::parse::{Expr, Module, Stmt, Type};
use super::sema::{BindingId, Resolution};

/// Errors from Rust to Vyre IR lowering.
#[derive(Debug, Clone, Error)]
pub enum RustLowerError {
    /// The module declares no function to use as the entry kernel.
    #[error("Rust lowering needs at least one function to use as the entry kernel")]
    NoEntryFunction,
    /// A construct outside the wired lowering subset was encountered.
    #[error(
        "Rust to Vyre IR lowering does not support {0} yet; not emitting a miscompiled Program"
    )]
    Unsupported(String),
}

/// Lower a resolved module to a Vyre IR program (the last function is the entry).
///
/// # Errors
/// Returns [`RustLowerError::NoEntryFunction`] for an empty module, or
/// [`RustLowerError::Unsupported`] for any construct outside the wired subset.
pub fn lower(module: &Module, resolution: &Resolution) -> Result<Program, RustLowerError> {
    lower_entry(module, resolution, LowerMode::Scalar)
}

/// Lower the module entry as a data-parallel map over `lane_count` elements.
///
/// The Rust function keeps scalar source semantics per lane: parameter `pN`
/// reads from `pN[global_id.x]`, and `return e` writes to `out[global_id.x]`.
/// Extra invocations from workgroup rounding are guarded before any buffer
/// access. This preserves the scalar [`lower`] entrypoint while exposing the
/// source frontend as a GPU-native batch kernel.
///
/// # Errors
/// Returns [`RustLowerError::Unsupported`] when `lane_count == 0`, or any error
/// returned by scalar lowering for unsupported language constructs.
pub fn lower_batched(
    module: &Module,
    resolution: &Resolution,
    lane_count: u32,
) -> Result<Program, RustLowerError> {
    if lane_count == 0 {
        return Err(RustLowerError::Unsupported(
            "batched Rust lowering with zero lanes".to_string(),
        ));
    }
    lower_entry(module, resolution, LowerMode::Batched { lane_count })
}

#[derive(Clone, Copy)]
enum LowerMode {
    Scalar,
    Batched { lane_count: u32 },
}

impl LowerMode {
    fn buffer_count(self) -> u32 {
        match self {
            Self::Scalar => 1,
            Self::Batched { lane_count } => lane_count,
        }
    }

    fn workgroup_size(self) -> [u32; 3] {
        match self {
            Self::Scalar => [1, 1, 1],
            Self::Batched { .. } => [256, 1, 1],
        }
    }

    fn lane_index(self) -> IrExpr {
        match self {
            Self::Scalar => IrExpr::u32(0),
            Self::Batched { .. } => IrExpr::var(BATCH_LANE_VAR),
        }
    }
}

const BATCH_LANE_VAR: &str = "__rust_lane";

fn lower_entry(
    module: &Module,
    resolution: &Resolution,
    mode: LowerMode,
) -> Result<Program, RustLowerError> {
    let entry_index = module
        .functions
        .len()
        .checked_sub(1)
        .ok_or(RustLowerError::NoEntryFunction)?;
    let func = &module.functions[entry_index];

    let def_to_id: HashMap<u32, BindingId> = resolution
        .bindings
        .iter()
        .enumerate()
        .map(|(id, b)| (b.def_offset, id))
        .collect();

    let mut buffers = Vec::with_capacity(func.params.len() + 1);
    let mut entry_nodes = Vec::new();
    for (i, (offset, ty)) in func.params.iter().enumerate() {
        let dtype = scalar_dtype(ty)?;
        let buf = format!("p{i}");
        buffers.push(BufferDecl::read(&buf, i as u32, dtype).with_count(mode.buffer_count()));
        let binding = def_to_id
            .get(offset)
            .copied()
            .ok_or_else(|| RustLowerError::Unsupported("unresolved parameter".to_string()))?;
        entry_nodes.push(Node::let_bind(
            format!("v{binding}"),
            IrExpr::load(buf, mode.lane_index()),
        ));
    }
    let out_dtype = scalar_dtype(&func.ret)?;
    buffers.push(
        BufferDecl::output("out", func.params.len() as u32, out_dtype)
            .with_count(mode.buffer_count()),
    );

    let ctx = LowerCtx {
        module,
        resolution,
        def_to_id: &def_to_id,
        output_index: mode.lane_index(),
    };
    entry_nodes.extend(ctx.lower_stmts(&func.body, Subst::Local(None))?);
    let entry_nodes = match mode {
        LowerMode::Scalar => entry_nodes,
        LowerMode::Batched { lane_count } => vec![
            Node::let_bind(BATCH_LANE_VAR, IrExpr::gid_x()),
            Node::if_then(
                IrExpr::lt(IrExpr::var(BATCH_LANE_VAR), IrExpr::u32(lane_count)),
                entry_nodes,
            ),
        ],
    };
    Ok(Program::wrapped(
        buffers,
        mode.workgroup_size(),
        entry_nodes,
    ))
}

/// The Vyre element type for a nano scalar type (only `i32`/`bool` so far).
fn scalar_dtype(ty: &Type) -> Result<DataType, RustLowerError> {
    match ty {
        Type::I32 => Ok(DataType::I32),
        Type::Bool => Ok(DataType::Bool),
        Type::Unit => Err(RustLowerError::Unsupported(
            "unit-typed parameter or return".to_string(),
        )),
        // A reference parameter carries its pointee value: in the
        // assignment-free nano-subset a `&T` is a pure read-alias, so it lowers
        // to a buffer of the pointee's element type.
        Type::Ref { inner, .. } => scalar_dtype(inner),
    }
}

struct LowerCtx<'a> {
    module: &'a Module,
    resolution: &'a Resolution,
    def_to_id: &'a HashMap<u32, BindingId>,
    output_index: IrExpr,
}

/// How a variable use lowers in the current scope.
///
/// The Rust lowering walks two structurally different scopes, and a variable
/// that is *not* named in the scope's map means opposite things in each. A
/// single `Option<&HashMap>` cannot express both, so the scope is explicit:
///
/// - [`Subst::Local`] is the entry-function scope and every loop body: a
///   variable lowers to its local `v{binding}` unless the overlay renames it
///   (e.g. a loop induction variable rebound to its loop variable). A binding
///   absent from the overlay is a normal local, not an error.
/// - [`Subst::Inline`] is the scope while a straight-line callee is being
///   inlined: every callee binding must be present in the map (mapped to its
///   caller-scope argument or folded `let`). A binding absent from the map is a
///   lowering bug (an un-substituted callee variable), not a local.
#[derive(Clone, Copy)]
enum Subst<'a> {
    /// Entry/loop-body scope with an optional rename overlay.
    Local(Option<&'a HashMap<BindingId, IrExpr>>),
    /// Callee-inline scope: every variable must be substituted.
    Inline(&'a HashMap<BindingId, IrExpr>),
}

impl LowerCtx<'_> {
    fn lower_stmts(&self, stmts: &[Stmt], subst: Subst<'_>) -> Result<Vec<Node>, RustLowerError> {
        let mut nodes = Vec::new();
        for stmt in stmts {
            match stmt {
                Stmt::Let { name, init, .. } => {
                    let binding = self.def_to_id.get(name).copied().ok_or_else(|| {
                        RustLowerError::Unsupported("unresolved let binding".to_string())
                    })?;
                    nodes.push(Node::let_bind(
                        format!("v{binding}"),
                        self.lower_value(init, subst)?,
                    ));
                }
                Stmt::Return(Some(expr)) => {
                    nodes.push(Node::store(
                        "out",
                        self.output_index.clone(),
                        self.lower_value(expr, subst)?,
                    ));
                    return Ok(nodes);
                }
                Stmt::Return(None) => return Ok(nodes),
                Stmt::Assign { name, value } => {
                    let binding = self.resolution.uses.get(name).copied().ok_or_else(|| {
                        RustLowerError::Unsupported("unresolved assignment target".to_string())
                    })?;
                    nodes.push(Node::assign(
                        format!("v{binding}"),
                        self.lower_value(value, subst)?,
                    ));
                }
                Stmt::Expr(Expr::If {
                    cond,
                    then_block,
                    else_block,
                }) => {
                    let then_nodes = self.lower_stmts(block_stmts(then_block), subst)?;
                    let else_nodes = match else_block {
                        Some(block) => self.lower_stmts(block_stmts(block), subst)?,
                        None => Vec::new(),
                    };
                    nodes.push(Node::if_then_else(
                        self.lower_value(cond, subst)?,
                        then_nodes,
                        else_nodes,
                    ));
                    let then_div = stmts_diverge(block_stmts(then_block));
                    let else_div = else_block
                        .as_ref()
                        .is_some_and(|b| stmts_diverge(block_stmts(b)));
                    if then_div && else_div {
                        return Ok(nodes);
                    }
                }
                Stmt::While { cond, body } => {
                    nodes.extend(self.lower_while(cond, body, subst)?);
                }
                Stmt::For {
                    name,
                    start,
                    end,
                    body,
                } => {
                    nodes.extend(self.lower_for_range(*name, start, end, body, subst)?);
                }
                // Pure expression statements have no observable effect; drop them.
                Stmt::Expr(_) => {}
            }
        }
        Ok(nodes)
    }

    /// Half-open `[0, trip)` u32 trip count for a counted loop over the signed
    /// range `[lo, hi)`: `hi - lo` when `hi > lo`, else 0. The subtraction is a
    /// *u32* wrapping subtraction of the two's-complement bit patterns, so a
    /// range exceeding `i32::MAX` stays exact and a negative `lo` does not wrap
    /// to ~4.29e9 iterations; the signed `hi > lo` guard (comparisons never
    /// overflow) zeroes the empty/negative-length case, matching Rust. This is
    /// the single source of the counted-loop bound invariant — both `while` and
    /// `for` lowering call it, so the correctness fix lives in exactly one place.
    fn counted_loop_trip(lo: &IrExpr, hi: &IrExpr) -> IrExpr {
        let span_u32 = IrExpr::sub(
            IrExpr::cast(DataType::U32, hi.clone()),
            IrExpr::cast(DataType::U32, lo.clone()),
        );
        IrExpr::select(IrExpr::gt(hi.clone(), lo.clone()), span_u32, IrExpr::u32(0))
    }

    /// Reconstruct the signed induction variable inside a counted-loop body:
    /// `lo + lv`, where `lv` is the fresh 0-based u32 loop counter read back as
    /// `i32` so body arithmetic stays well-typed (and correct for negative `lo`).
    fn counted_loop_induction(lo: &IrExpr, loop_var: &str) -> IrExpr {
        IrExpr::add(
            lo.clone(),
            IrExpr::cast(DataType::I32, IrExpr::var(loop_var.to_string())),
        )
    }

    /// Lower `while i < BOUND { ...; i = i + 1; }` to a counted `Node::Loop`.
    /// Only this exact counting form is supported; anything else (data-dependent
    /// exit, mutated bound, `i` assigned outside the trailing increment) returns
    /// a loud `Unsupported` rather than a miscompiled loop.
    fn lower_while(
        &self,
        cond: &Expr,
        body: &[Stmt],
        subst: Subst<'_>,
    ) -> Result<Vec<Node>, RustLowerError> {
        let bad = || {
            RustLowerError::Unsupported(
                "while loop that is not a canonical `while i < BOUND { ...; i = i + 1; }` counting loop"
                    .to_string(),
            )
        };
        // cond must be `i < BOUND`.
        let (i_off, bound) = match cond {
            Expr::Binary { op, lhs, rhs } if *op == LT => match lhs.as_ref() {
                Expr::Var(off) => (*off, rhs.as_ref()),
                _ => return Err(bad()),
            },
            _ => return Err(bad()),
        };
        let b_i = self.resolution.uses.get(&i_off).copied().ok_or_else(bad)?;
        // Trailing statement must be `i = i + 1`.
        let Some((last, init_stmts)) = body.split_last() else {
            return Err(bad());
        };
        let inc_ok = matches!(last, Stmt::Assign { name, value }
            if self.resolution.uses.get(name).copied() == Some(b_i)
            && matches!(value, Expr::Binary { op, lhs, rhs }
                if *op == PLUS
                && matches!(lhs.as_ref(), Expr::Var(o) if self.resolution.uses.get(o).copied() == Some(b_i))
                && matches!(rhs.as_ref(), Expr::LiteralInt(_, 1))));
        if !inc_ok {
            return Err(bad());
        }
        // `i` must not be assigned anywhere except the trailing increment.
        if stmts_assign_binding(init_stmts, b_i, self.resolution) {
            return Err(bad());
        }
        // BOUND must be loop-invariant: none of its free variables are assigned
        // in the body (so evaluating it once for the loop bound matches Rust).
        for v in expr_var_bindings(bound, self.resolution) {
            if stmts_assign_binding(body, v, self.resolution) {
                return Err(bad());
            }
        }
        let loop_var = format!("v{b_i}__w");
        // Extend the current scope's overlay with the induction-variable rename,
        // preserving the scope kind (a `while` only appears in `Local` scope
        // today, since inlined callees are straight-line; the `Inline` arm keeps
        // the invariant sound if that ever changes).
        let mut inner: HashMap<BindingId, IrExpr> = match subst {
            Subst::Local(Some(m)) => m.clone(),
            Subst::Local(None) => HashMap::new(),
            Subst::Inline(m) => m.clone(),
        };
        // The IR loop variable is `u32` by contract (`Node::Loop` bounds and
        // index are u32), but the Rust induction variable is `i32`. Inside the
        // body every use of `i` must read back as `i32`, so the overlay maps the
        // binding to `cast(i32, loop_var)`; otherwise `acc + i` would mix u32 and
        // i32 operands and fail IR validation.
        // The induction variable `i` is reconstructed as `i0 + lv`, where `lv`
        // is a fresh 0-based u32 loop counter and `i0` is the (possibly negative)
        // signed initial value held in `v{b_i}`. Reading `i` back as i32 keeps
        // body arithmetic well-typed and stays correct even when `i0 < 0` — a
        // plain `cast(u32, i0)` loop bound would wrap a negative start to ~4.29e9
        // and iterate billions of times instead of matching Rust.
        inner.insert(
            b_i,
            Self::counted_loop_induction(&IrExpr::var(format!("v{b_i}")), &loop_var),
        );
        let inner_subst = match subst {
            Subst::Inline(_) => Subst::Inline(&inner),
            Subst::Local(_) => Subst::Local(Some(&inner)),
        };
        // The IR loop is a half-open `[0, trip)` u32 range. Computing the trip
        // count in *signed* space and clamping to zero before the u32 cast makes
        // a zero- or negative-length range run the body zero times, exactly like
        // Rust (`while i < n` with `i >= n` never enters) — instead of wrapping a
        // negative `n - i0` into billions of iterations (a DoS + miscompile).
        let from_i32 = IrExpr::var(format!("v{b_i}"));
        let to_i32 = self.lower_value(bound, subst)?;
        let trip = Self::counted_loop_trip(&from_i32, &to_i32);
        let from = IrExpr::u32(0);
        let to = trip;
        let loop_body = self.lower_stmts(init_stmts, inner_subst)?;
        // After the loop `i == max(i0, n)`: the bound `n` when the loop ran at
        // least once, or the unchanged start `i0` for a zero-trip loop. The old
        // `i := n` was wrong for the zero-trip case (Rust leaves `i` untouched).
        let post = IrExpr::select(
            IrExpr::gt(to_i32.clone(), from_i32.clone()),
            to_i32,
            from_i32,
        );
        Ok(vec![
            Node::loop_for(loop_var, from, to, loop_body),
            Node::assign(format!("v{b_i}"), post),
        ])
    }

    /// Lower `for i in START..END { body }` to a signed, half-open counted
    /// `Node::Loop`. Range bounds are snapshotted before the loop so mutation in
    /// the body cannot change how many iterations run, matching Rust's iterator
    /// construction semantics for `Range<i32>`.
    fn lower_for_range(
        &self,
        name: u32,
        start: &Expr,
        end: &Expr,
        body: &[Stmt],
        subst: Subst<'_>,
    ) -> Result<Vec<Node>, RustLowerError> {
        let b_i = self.def_to_id.get(&name).copied().ok_or_else(|| {
            RustLowerError::Unsupported("unresolved for-loop binding".to_string())
        })?;
        let start_name = format!("v{b_i}__for_start");
        let end_name = format!("v{b_i}__for_end");
        let loop_var = format!("v{b_i}__for");

        let start_i32 = IrExpr::var(start_name.clone());
        let end_i32 = IrExpr::var(end_name.clone());
        let trip = Self::counted_loop_trip(&start_i32, &end_i32);

        let mut inner: HashMap<BindingId, IrExpr> = match subst {
            Subst::Local(Some(m)) => m.clone(),
            Subst::Local(None) => HashMap::new(),
            Subst::Inline(m) => m.clone(),
        };
        inner.insert(b_i, Self::counted_loop_induction(&start_i32, &loop_var));
        let inner_subst = match subst {
            Subst::Inline(_) => Subst::Inline(&inner),
            Subst::Local(_) => Subst::Local(Some(&inner)),
        };
        let loop_body = self.lower_stmts(body, inner_subst)?;

        Ok(vec![
            Node::let_bind(start_name, self.lower_value(start, subst)?),
            Node::let_bind(end_name, self.lower_value(end, subst)?),
            Node::loop_for(loop_var, IrExpr::u32(0), trip, loop_body),
        ])
    }

    /// Lower a value expression in the current scope. In a [`Subst::Local`]
    /// scope a variable lowers to its alpha-renamed local `v{id}` unless the
    /// overlay renames it (a loop induction variable); in a [`Subst::Inline`]
    /// scope every callee variable must be present in the map (mapped to its
    /// caller-scope argument or a folded `let`), and an absent one is a bug.
    fn lower_value(&self, expr: &Expr, subst: Subst<'_>) -> Result<IrExpr, RustLowerError> {
        match expr {
            Expr::LiteralInt(_, value) => Ok(IrExpr::i32(*value as i32)),
            Expr::LiteralBool(_, value) => Ok(IrExpr::bool(*value)),
            Expr::Var(offset) => {
                let binding = self.resolution.uses.get(offset).copied().ok_or_else(|| {
                    RustLowerError::Unsupported("unresolved variable use".to_string())
                })?;
                match subst {
                    // Local scope: an overlay rename wins, otherwise the binding
                    // is its own local. An absent binding is a normal local.
                    Subst::Local(Some(map)) => Ok(map
                        .get(&binding)
                        .cloned()
                        .unwrap_or_else(|| IrExpr::var(format!("v{binding}")))),
                    Subst::Local(None) => Ok(IrExpr::var(format!("v{binding}"))),
                    // Inline scope: every callee binding must be substituted.
                    Subst::Inline(map) => map.get(&binding).cloned().ok_or_else(|| {
                        RustLowerError::Unsupported("callee variable not substituted".to_string())
                    }),
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let l = self.lower_value(lhs, subst)?;
                let r = self.lower_value(rhs, subst)?;
                Ok(match *op {
                    PLUS => IrExpr::add(l, r),
                    MINUS => IrExpr::sub(l, r),
                    STAR => IrExpr::mul(l, r),
                    SLASH => IrExpr::div(l, r),
                    // Vyre types `Mod`'s result as u32 even for i32 operands;
                    // the value is signed-correct, so cast back to i32 to keep
                    // composition and the i32 store well-typed.
                    PERCENT => IrExpr::cast(DataType::I32, IrExpr::rem(l, r)),
                    EQ => IrExpr::eq(l, r),
                    NE => IrExpr::ne(l, r),
                    LT => IrExpr::lt(l, r),
                    GT => IrExpr::gt(l, r),
                    LE => IrExpr::le(l, r),
                    GE => IrExpr::ge(l, r),
                    ANDAND => IrExpr::and(l, r),
                    OROR => IrExpr::or(l, r),
                    other => {
                        return Err(RustLowerError::Unsupported(format!(
                            "binary operator {other}"
                        )))
                    }
                })
            }
            Expr::Call { name, args } => self.lower_call(name, args, subst),
            // In the assignment-free nano-subset a reference is a pure read
            // alias: `&e` evaluates to e's value and `*r` reads that value back,
            // so borrow and dereference are value-transparent.
            Expr::Borrow { expr, .. } => self.lower_value(expr, subst),
            Expr::Deref(inner) => self.lower_value(inner, subst),
            Expr::Not(inner) => Ok(IrExpr::not(self.lower_value(inner, subst)?)),
            // Rust unary `-x` on i32 is wrapping negation. The IR's total
            // `Negate` is illegal on i32 (the i32::MIN overflow case), so lower
            // to `0 - x`, which is value-identical for all in-range x and
            // wrapping-correct at i32::MIN — matching Rust release semantics.
            Expr::Neg(inner) => Ok(IrExpr::sub(IrExpr::i32(0), self.lower_value(inner, subst)?)),
            Expr::Block(_) | Expr::If { .. } => Err(RustLowerError::Unsupported(
                "block/if used as a value".to_string(),
            )),
        }
    }

    /// Inline a call to a straight-line single-return callee: substitute its
    /// parameters with the (caller-scope) argument expressions, fold its `let`
    /// bindings, and return its lowered return expression. A callee with control
    /// flow or no terminal return is a loud `Unsupported` (never miscompiled).
    fn lower_call(
        &self,
        name: &u32,
        args: &[Expr],
        caller_subst: Subst<'_>,
    ) -> Result<IrExpr, RustLowerError> {
        let callee_index = self
            .resolution
            .calls
            .get(name)
            .copied()
            .ok_or_else(|| RustLowerError::Unsupported("unresolved call".to_string()))?;
        let callee = &self.module.functions[callee_index];
        if args.len() != callee.params.len() {
            return Err(RustLowerError::Unsupported(
                "call arity mismatch".to_string(),
            ));
        }
        let mut subst: HashMap<BindingId, IrExpr> = HashMap::new();
        for (i, (offset, _)) in callee.params.iter().enumerate() {
            let binding = self.def_to_id.get(offset).copied().ok_or_else(|| {
                RustLowerError::Unsupported("unresolved callee parameter".to_string())
            })?;
            subst.insert(binding, self.lower_value(&args[i], caller_subst)?);
        }
        for stmt in &callee.body {
            match stmt {
                Stmt::Let {
                    name: offset, init, ..
                } => {
                    let value = self.lower_value(init, Subst::Inline(&subst))?;
                    let binding = self.def_to_id.get(offset).copied().ok_or_else(|| {
                        RustLowerError::Unsupported("unresolved callee binding".to_string())
                    })?;
                    subst.insert(binding, value);
                }
                Stmt::Return(Some(expr)) => return self.lower_value(expr, Subst::Inline(&subst)),
                _ => {
                    return Err(RustLowerError::Unsupported(
                        "call to a callee with control flow or no terminal return".to_string(),
                    ))
                }
            }
        }
        Err(RustLowerError::Unsupported(
            "call to a callee with no return".to_string(),
        ))
    }
}

/// Whether any statement assigns binding `b` (recursing into if/while bodies).
fn stmts_assign_binding(stmts: &[Stmt], b: BindingId, res: &Resolution) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Assign { name, .. } => res.uses.get(name).copied() == Some(b),
        Stmt::Expr(Expr::If {
            then_block,
            else_block,
            ..
        }) => {
            stmts_assign_binding(block_stmts(then_block), b, res)
                || else_block
                    .as_ref()
                    .is_some_and(|e| stmts_assign_binding(block_stmts(e), b, res))
        }
        Stmt::While { body, .. } => stmts_assign_binding(body, b, res),
        Stmt::For { body, .. } => stmts_assign_binding(body, b, res),
        _ => false,
    })
}

/// Collect the binding ids of every variable read in `expr`.
fn expr_var_bindings(expr: &Expr, res: &Resolution) -> Vec<BindingId> {
    let mut out = Vec::new();
    collect_var_bindings(expr, res, &mut out);
    out
}

fn collect_var_bindings(expr: &Expr, res: &Resolution, out: &mut Vec<BindingId>) {
    match expr {
        Expr::Var(off) => {
            if let Some(&id) = res.uses.get(off) {
                out.push(id);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_var_bindings(lhs, res, out);
            collect_var_bindings(rhs, res, out);
        }
        Expr::Borrow { expr, .. } => collect_var_bindings(expr, res, out),
        Expr::Deref(inner) => collect_var_bindings(inner, res, out),
        Expr::Not(inner) => collect_var_bindings(inner, res, out),
        Expr::Neg(inner) => collect_var_bindings(inner, res, out),
        Expr::Call { args, .. } => {
            for a in args {
                collect_var_bindings(a, res, out);
            }
        }
        _ => {}
    }
}

/// Statement list of a block expression (empty for anything else).
fn block_stmts(expr: &Expr) -> &[Stmt] {
    match expr {
        Expr::Block(stmts) => stmts,
        _ => &[],
    }
}

/// Whether a statement sequence returns on every path (so trailing code is
/// unreachable). Mirrors the typeck divergence check.
fn stmts_diverge(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|stmt| match stmt {
        Stmt::Return(_) => true,
        Stmt::Expr(Expr::If {
            then_block,
            else_block: Some(else_block),
            ..
        }) => stmts_diverge(block_stmts(then_block)) && stmts_diverge(block_stmts(else_block)),
        _ => false,
    })
}
