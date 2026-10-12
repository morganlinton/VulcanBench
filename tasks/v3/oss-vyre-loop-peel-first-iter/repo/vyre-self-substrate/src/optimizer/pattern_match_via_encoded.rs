//! Local-pattern rewrite engine as a dispatched compute kernel.
//!
//! V1 ships a hardcoded bank of algebraic-identity rewrites:
//!
//! - `Add 0 ?x   →   ?x`
//! - `Add ?x 0   →   ?x`
//! - `Mul 1 ?x   →   ?x`
//! - `Mul ?x 1   →   ?x`
//! - `Mul 0 ?x   →   0u32`
//! - `Mul ?x 0   →   0u32`
//!
//! Each rule fires per-Expr in a single GPU dispatch (no scope walk,
//! no structural-hash needed for this set). Output is a `rewrite_action`
//! buffer encoding the per-Expr decision; the decoder applies it.
//!
//! This is the architectural prototype for the universal pattern-match
//! engine: V2 takes the pattern bank as input buffers (kind/op/literal-
//! value templates per pattern) and runs the same kernel shape over
//! arbitrary rewrite rules sourced from a TOML database (ROADMAP A6).
//! All the hardcoding below is a fixed instance of that more general
//! kernel.
//!
//! No host-reference escape in production. `OptimizerDispatcher` injects the
//! backend; the same kernel runs unchanged on wgpu + CUDA.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::dispatch_buffers::{
    decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes, write_zero_bytes,
};

use super::dispatcher::{DispatchError, OptimizerDispatcher};
use super::encode::EncodeError;
use super::expr_arena::{encode_expr_arena, expr_kind, ExprArenaEncoding};

#[derive(Debug, Default)]
struct PatternKernelScratch {
    inputs: Vec<Vec<u8>>,
}

/// Per-Expr rewrite-action discriminants written by the kernel.
pub mod rewrite_action {
    /// No rewrite applies  -  keep the Expr as-is.
    pub const NONE: u32 = 0;
    /// Replace the Expr with its left child (the operand at `arg1`).
    pub const REPLACE_WITH_LEFT: u32 = 1;
    /// Replace with the right child (the operand at `arg2`).
    pub const REPLACE_WITH_RIGHT: u32 = 2;
    /// Replace with `LitU32(0)`.
    pub const REPLACE_WITH_LIT_ZERO: u32 = 3;
    /// For a `UnOp(op, UnOp(op, x))`: replace with `x` (the
    /// grand-child at `arg1->arg1`). Fires for `~~x = x`, `--x = x`,
    /// `!!x = x`.
    pub const REPLACE_WITH_GRAND_OPERAND: u32 = 4;
    /// Replace with `LitBool(true)`. Fires for `x == x`, `x <= x`,
    /// `x >= x` after CSE proves the operands are equivalent.
    pub const REPLACE_WITH_LIT_TRUE: u32 = 5;
    /// Replace with `LitBool(false)`. Fires for `x != x`, `x < x`,
    /// `x > x` (irreflexive comparisons of equal operands).
    pub const REPLACE_WITH_LIT_FALSE: u32 = 6;
    /// For a `BinOp(_, BinOp(_, a, b), _)`: replace with `a`
    /// (the outer's left child's left grand-child). Fires for
    /// `(Sub (Add a b) b) → a` after CSE confirms operand equality.
    pub const REPLACE_WITH_LEFT_INNER_LEFT: u32 = 7;
    /// Same as above but pulls the left child's right grand-child.
    /// Fires for `(Sub (Add a b) a) → b`.
    pub const REPLACE_WITH_LEFT_INNER_RIGHT: u32 = 8;
}

/// Errors surfaced by `gpu_algebraic_identities`.
#[derive(Debug)]
pub enum PatternMatchError {
    Encode(EncodeError),
    Dispatch(DispatchError),
}

impl std::fmt::Display for PatternMatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(err) => write!(f, "gpu_algebraic_identities encode error: {err:?}"),
            Self::Dispatch(err) => write!(f, "gpu_algebraic_identities dispatch error: {err}"),
        }
    }
}

impl std::error::Error for PatternMatchError {}

/// Run V1 algebraic-identity pattern-match against `program`. Returns
/// the rewritten Program with simplified BinOps.
pub fn gpu_algebraic_identities(
    program: Program,
    dispatcher: &dyn OptimizerDispatcher,
) -> Result<Program, PatternMatchError> {
    let arena = encode_expr_arena(&program).map_err(PatternMatchError::Encode)?;
    if arena.expr_count == 0 {
        return Ok(program);
    }
    let mut scratch = PatternKernelScratch::default();
    let mut actions = Vec::with_capacity(arena.expr_count as usize);
    run_pattern_kernel_with_scratch_into(&arena, dispatcher, &mut scratch, &mut actions)
        .map_err(PatternMatchError::Dispatch)?;
    Ok(rewrite_program_with_actions(program, &actions))
}

#[cfg(test)]
fn run_pattern_kernel_into(
    arena: &ExprArenaEncoding,
    dispatcher: &dyn OptimizerDispatcher,
    actions: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = PatternKernelScratch::default();
    run_pattern_kernel_with_scratch_into(arena, dispatcher, &mut scratch, actions)
}

fn run_pattern_kernel_with_scratch_into(
    arena: &ExprArenaEncoding,
    dispatcher: &dyn OptimizerDispatcher,
    scratch: &mut PatternKernelScratch,
    actions: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let n = arena.expr_count;
    let analysis = build_pattern_match_program(n);
    let words = n as usize;
    let output_bytes = words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: pattern-match output byte count overflows usize for expr_count={n}."
            ))
        })?;

    ensure_input_slots(&mut scratch.inputs, 5);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], &arena.kinds);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], &arena.arg0);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], &arena.arg1);
    write_u32_slice_le_bytes(&mut scratch.inputs[3], &arena.arg2);
    write_zero_bytes(&mut scratch.inputs[4], output_bytes);

    let grid_x = (n + WORKGROUP_X - 1) / WORKGROUP_X;
    let outputs = dispatcher.dispatch(&analysis, &scratch.inputs, Some([grid_x, 1, 1]))?;
    if outputs.len() != 1 {
        return Err(DispatchError::BackendError(format!(
            "Fix: pattern-match dispatch expected exactly one rewrite_action output, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], words, "pattern-match rewrite_action", actions)
}

/// Workgroup size for the parallel pattern-match kernel.
const WORKGROUP_X: u32 = 256;

/// Build a CSE-aware pattern-match Program. Identical to
/// `build_pattern_match_program` but with an extra `canonical` (RO,
/// binding 5) buffer that lets the kernel fire structural-equality
/// rules: `x ^ x → 0`, `x - x → 0`, `x & x → x`, `x | x → x`. These
/// rules only fire when `canonical[arg1] == canonical[arg2]` after
/// CSE. Caller must populate `canonical` by running
/// `gpu_cse_canonicals` first.
#[must_use]
pub fn build_pattern_match_program_with_cse(expr_count: u32) -> Program {
    let buffers = vec![
        BufferDecl::storage("arena_kinds", 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg0", 1, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg1", 2, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg2", 3, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("rewrite_action", 4, BufferAccess::ReadWrite, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("canonical", 5, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
    ];

    let body = vec![
        Node::let_bind("i", Expr::gid_x()),
        Node::if_then(
            Expr::lt(Expr::var("i"), Expr::u32(expr_count)),
            vec![
                Node::let_bind("kind", Expr::load("arena_kinds", Expr::var("i"))),
                Node::if_then(
                    Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::BIN_OP)),
                    bin_op_match_body_with_cse(),
                ),
                Node::if_then(
                    Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::UN_OP)),
                    un_op_match_body(),
                ),
            ],
        ),
    ];

    Program::wrapped(buffers, [WORKGROUP_X, 1, 1], body)
}

/// UnOp double-application matcher. Fires when:
///   `Expr i = UnOp(op, UnOp(op, x))` and `op` is involutive
/// (i.e. `op(op(x)) == x` for all x). Writes
/// `REPLACE_WITH_GRAND_OPERAND` so the rewriter collapses to `x`.
///
/// Restricted to the three truly involutive UnOps: `Negate` (0x01),
/// `BitNot` (0x02), `LogicalNot` (0x03). NOT `Abs`/`Sign`/`Floor`/
/// `Ceil`/`Round`/`Trunc` etc., which are idempotent (`f(f(x)) ==
/// f(x)`) but NOT identity. Folding those to `x` would change
/// behaviour when `x` lies outside the op's range.
fn un_op_match_body() -> Vec<Node> {
    vec![
        Node::let_bind("u_op", Expr::load("arena_arg0", Expr::var("i"))),
        Node::let_bind("u_child", Expr::load("arena_arg1", Expr::var("i"))),
        Node::let_bind(
            "u_child_kind",
            Expr::load("arena_kinds", Expr::var("u_child")),
        ),
        Node::let_bind(
            "u_op_is_involutive",
            Expr::or(
                Expr::or(
                    Expr::eq(Expr::var("u_op"), Expr::u32(0x01)),
                    Expr::eq(Expr::var("u_op"), Expr::u32(0x02)),
                ),
                Expr::eq(Expr::var("u_op"), Expr::u32(0x03)),
            ),
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("u_child_kind"), Expr::u32(expr_kind::UN_OP)),
                Expr::var("u_op_is_involutive"),
            ),
            vec![
                Node::let_bind("u_child_op", Expr::load("arena_arg0", Expr::var("u_child"))),
                Node::if_then(
                    Expr::eq(Expr::var("u_child_op"), Expr::var("u_op")),
                    vec![Node::store(
                        "rewrite_action",
                        Expr::var("i"),
                        Expr::u32(rewrite_action::REPLACE_WITH_GRAND_OPERAND),
                    )],
                ),
            ],
        ),
    ]
}

/// CSE-aware variant of `bin_op_match_body`. Inlines all the literal
/// rules from the base body, then adds structural-equality rules
/// using the `canonical` buffer. All bindings (l, r, is_*) live in
/// the same scope so the canonical-equality rules can reference them
/// directly.
fn bin_op_match_body_with_cse() -> Vec<Node> {
    let mut body = bin_op_match_body();
    // Append CSE-aware rules using the same scope (l, r, is_*
    // already bound in `body`). Fetch canonical[l] / canonical[r]
    // and gate on equality.
    body.push(Node::let_bind(
        "can_l",
        Expr::load("canonical", Expr::var("l")),
    ));
    body.push(Node::let_bind(
        "can_r",
        Expr::load("canonical", Expr::var("r")),
    ));
    body.push(Node::let_bind(
        "operands_equal",
        Expr::eq(Expr::var("can_l"), Expr::var("can_r")),
    ));
    // Min/Max/AbsDiff op tag flags  -  needed for the next rule batch.
    body.push(Node::let_bind(
        "is_min",
        Expr::eq(Expr::var("op"), Expr::u32(0x15)),
    ));
    body.push(Node::let_bind(
        "is_max",
        Expr::eq(Expr::var("op"), Expr::u32(0x16)),
    ));
    body.push(Node::let_bind(
        "is_absdiff",
        Expr::eq(Expr::var("op"), Expr::u32(0x14)),
    ));
    body.push(Node::let_bind(
        "is_sat_add",
        Expr::eq(Expr::var("op"), Expr::u32(0x17)),
    ));
    body.push(Node::let_bind(
        "is_sat_sub",
        Expr::eq(Expr::var("op"), Expr::u32(0x18)),
    ));
    body.push(Node::let_bind(
        "is_sat_mul",
        Expr::eq(Expr::var("op"), Expr::u32(0x19)),
    ));
    body.push(Node::let_bind(
        "is_wrap_add",
        Expr::eq(Expr::var("op"), Expr::u32(0x20)),
    ));
    body.push(Node::let_bind(
        "is_wrap_sub",
        Expr::eq(Expr::var("op"), Expr::u32(0x21)),
    ));
    // (SaturatingSub ?x ?x) → 0u
    body.push(Node::if_then(
        Expr::and(Expr::var("is_sat_sub"), Expr::var("operands_equal")),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
        )],
    ));
    // (WrappingSub ?x ?x) → 0u
    body.push(Node::if_then(
        Expr::and(Expr::var("is_wrap_sub"), Expr::var("operands_equal")),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
        )],
    ));
    // (Min ?x ?x) → ?x   -  idempotent under operand equality.
    body.push(Node::if_then(
        Expr::and(Expr::var("is_min"), Expr::var("operands_equal")),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (Max ?x ?x) → ?x   -  idempotent under operand equality.
    body.push(Node::if_then(
        Expr::and(Expr::var("is_max"), Expr::var("operands_equal")),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (AbsDiff ?x ?x) → 0u32   -  |x - x| = 0.
    body.push(Node::if_then(
        Expr::and(Expr::var("is_absdiff"), Expr::var("operands_equal")),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
        )],
    ));
    // (Sub ?x ?x) → 0u32
    body.push(Node::if_then(
        Expr::and(Expr::var("is_sub"), Expr::var("operands_equal")),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
        )],
    ));
    // (BitXor ?x ?x) → 0u32
    body.push(Node::if_then(
        Expr::and(Expr::var("is_bitxor"), Expr::var("operands_equal")),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
        )],
    ));
    // (BitAnd ?x ?x) → ?x
    body.push(Node::if_then(
        Expr::and(Expr::var("is_bitand"), Expr::var("operands_equal")),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (BitOr ?x ?x) → ?x
    body.push(Node::if_then(
        Expr::and(Expr::var("is_bitor"), Expr::var("operands_equal")),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (And ?x ?x) → ?x   -  bool-level idempotency.
    body.push(Node::if_then(
        Expr::and(Expr::var("is_bool_and"), Expr::var("operands_equal")),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (Or ?x ?x) → ?x   -  bool-level idempotency.
    body.push(Node::if_then(
        Expr::and(Expr::var("is_bool_or"), Expr::var("operands_equal")),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (Add ?x ?x): no canonical simplification  -  keep as-is
    //   (no rewrite). Skipped here intentionally so other passes
    //   that prefer doubling-as-shift can inspect the pattern.

    // (Eq ?x ?x), (Le ?x ?x), (Ge ?x ?x) → LitBool(true)
    body.push(Node::if_then(
        Expr::and(
            Expr::or(
                Expr::or(Expr::var("is_cmp_eq"), Expr::var("is_cmp_le")),
                Expr::var("is_cmp_ge"),
            ),
            Expr::var("operands_equal"),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LIT_TRUE),
        )],
    ));
    // (Ne ?x ?x), (Lt ?x ?x), (Gt ?x ?x) → LitBool(false)
    body.push(Node::if_then(
        Expr::and(
            Expr::or(
                Expr::or(Expr::var("is_cmp_ne"), Expr::var("is_cmp_lt")),
                Expr::var("is_cmp_gt"),
            ),
            Expr::var("operands_equal"),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LIT_FALSE),
        )],
    ));

    // Sub-Add cancellation: `(Sub (Add a b) c)` where canonical
    // identifies `a == c` or `b == c` collapses to the unmatched
    // operand. Detect by inspecting the left child (`l`)  -  if its
    // kind is BIN_OP with op == 0x01 (Add), and one of its
    // canonical operand-children matches `canonical[r]`, fire.
    body.push(Node::let_bind(
        "l_kind_full",
        Expr::load("arena_kinds", Expr::var("l")),
    ));
    body.push(Node::let_bind(
        "l_op",
        Expr::load("arena_arg0", Expr::var("l")),
    ));
    body.push(Node::let_bind(
        "l_inner_left",
        Expr::load("arena_arg1", Expr::var("l")),
    ));
    body.push(Node::let_bind(
        "l_inner_right",
        Expr::load("arena_arg2", Expr::var("l")),
    ));
    body.push(Node::let_bind(
        "l_inner_left_canon",
        Expr::load("canonical", Expr::var("l_inner_left")),
    ));
    body.push(Node::let_bind(
        "l_inner_right_canon",
        Expr::load("canonical", Expr::var("l_inner_right")),
    ));
    // `(Sub (Add a b) c)` and canonical[a] == canonical[c]: take b
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_sub"),
            Expr::and(
                Expr::eq(Expr::var("l_kind_full"), Expr::u32(expr_kind::BIN_OP)),
                Expr::and(
                    Expr::eq(Expr::var("l_op"), Expr::u32(0x01)),
                    Expr::eq(Expr::var("l_inner_left_canon"), Expr::var("can_r")),
                ),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT_INNER_RIGHT),
        )],
    ));
    // `(Sub (Add a b) c)` and canonical[b] == canonical[c]: take a
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_sub"),
            Expr::and(
                Expr::eq(Expr::var("l_kind_full"), Expr::u32(expr_kind::BIN_OP)),
                Expr::and(
                    Expr::eq(Expr::var("l_op"), Expr::u32(0x01)),
                    Expr::eq(Expr::var("l_inner_right_canon"), Expr::var("can_r")),
                ),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT_INNER_LEFT),
        )],
    ));

    // Add-Sub cancellation: `(Add (Sub a b) b) → a` and the
    // commutative variant `(Add (Sub a b) b)` after canon. Op tag
    // for Sub is 0x02; the left's op must be Sub for this rule.
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_add"),
            Expr::and(
                Expr::eq(Expr::var("l_kind_full"), Expr::u32(expr_kind::BIN_OP)),
                Expr::and(
                    // Left's op == Sub (0x02)
                    Expr::eq(Expr::var("l_op"), Expr::u32(0x02)),
                    // canonical[(left's right operand b)] == canonical[r]
                    Expr::eq(Expr::var("l_inner_right_canon"), Expr::var("can_r")),
                ),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT_INNER_LEFT),
        )],
    ));

    // BitXor self-cancellation through a chain: BitXor is its own
    // inverse, so `(BitXor (BitXor a b) b) → a` and the symmetric
    // `(BitXor (BitXor a b) a) → b`. Op tag for BitXor is 0x08.
    // `(BitXor (BitXor a b) c)` and canonical[b] == canonical[c]: take a
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_bitxor"),
            Expr::and(
                Expr::eq(Expr::var("l_kind_full"), Expr::u32(expr_kind::BIN_OP)),
                Expr::and(
                    Expr::eq(Expr::var("l_op"), Expr::u32(0x08)),
                    Expr::eq(Expr::var("l_inner_right_canon"), Expr::var("can_r")),
                ),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT_INNER_LEFT),
        )],
    ));
    // `(BitXor (BitXor a b) c)` and canonical[a] == canonical[c]: take b
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_bitxor"),
            Expr::and(
                Expr::eq(Expr::var("l_kind_full"), Expr::u32(expr_kind::BIN_OP)),
                Expr::and(
                    Expr::eq(Expr::var("l_op"), Expr::u32(0x08)),
                    Expr::eq(Expr::var("l_inner_left_canon"), Expr::var("can_r")),
                ),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT_INNER_RIGHT),
        )],
    ));

    // Min/Max literal-identity rules. For u32: 0 is the absolute
    // minimum (any value is >= 0), MAX is the absolute maximum.
    // (Min ?x 0u) → 0u
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_min"),
            Expr::and(
                Expr::var("r_is_lit_u32"),
                Expr::eq(Expr::var("r_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
        )],
    ));
    // (Min 0u ?x) → 0u
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_min"),
            Expr::and(
                Expr::var("l_is_lit_u32"),
                Expr::eq(Expr::var("l_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
        )],
    ));
    // (Min ?x MAX) → ?x
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_min"),
            Expr::and(
                Expr::var("r_is_lit_u32"),
                Expr::eq(Expr::var("r_val"), Expr::u32(u32::MAX)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (Min MAX ?x) → ?x
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_min"),
            Expr::and(
                Expr::var("l_is_lit_u32"),
                Expr::eq(Expr::var("l_val"), Expr::u32(u32::MAX)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
        )],
    ));
    // (Max ?x MAX) → MAX  (replace with right; right IS the literal MAX)
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_max"),
            Expr::and(
                Expr::var("r_is_lit_u32"),
                Expr::eq(Expr::var("r_val"), Expr::u32(u32::MAX)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
        )],
    ));
    // (Max MAX ?x) → MAX
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_max"),
            Expr::and(
                Expr::var("l_is_lit_u32"),
                Expr::eq(Expr::var("l_val"), Expr::u32(u32::MAX)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (Max ?x 0u) → ?x  (max with 0 is identity for u32)
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_max"),
            Expr::and(
                Expr::var("r_is_lit_u32"),
                Expr::eq(Expr::var("r_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (Max 0u ?x) → ?x
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_max"),
            Expr::and(
                Expr::var("l_is_lit_u32"),
                Expr::eq(Expr::var("l_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
        )],
    ));
    // Saturating/Wrapping zero/one identities (literal cases).
    // (SaturatingAdd ?x 0) → ?x
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_sat_add"),
            Expr::and(
                Expr::var("r_is_lit_u32"),
                Expr::eq(Expr::var("r_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (SaturatingAdd 0 ?x) → ?x
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_sat_add"),
            Expr::and(
                Expr::var("l_is_lit_u32"),
                Expr::eq(Expr::var("l_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
        )],
    ));
    // (SaturatingSub ?x 0) → ?x
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_sat_sub"),
            Expr::and(
                Expr::var("r_is_lit_u32"),
                Expr::eq(Expr::var("r_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (SaturatingMul ?x 0) → 0
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_sat_mul"),
            Expr::and(
                Expr::var("r_is_lit_u32"),
                Expr::eq(Expr::var("r_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
        )],
    ));
    // (SaturatingMul 0 ?x) → 0
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_sat_mul"),
            Expr::and(
                Expr::var("l_is_lit_u32"),
                Expr::eq(Expr::var("l_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
        )],
    ));
    // (SaturatingMul ?x 1) → ?x
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_sat_mul"),
            Expr::and(
                Expr::var("r_is_lit_u32"),
                Expr::eq(Expr::var("r_val"), Expr::u32(1)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (SaturatingMul 1 ?x) → ?x
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_sat_mul"),
            Expr::and(
                Expr::var("l_is_lit_u32"),
                Expr::eq(Expr::var("l_val"), Expr::u32(1)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
        )],
    ));
    // (WrappingAdd ?x 0) → ?x
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_wrap_add"),
            Expr::and(
                Expr::var("r_is_lit_u32"),
                Expr::eq(Expr::var("r_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    // (WrappingAdd 0 ?x) → ?x
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_wrap_add"),
            Expr::and(
                Expr::var("l_is_lit_u32"),
                Expr::eq(Expr::var("l_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
        )],
    ));
    // (WrappingSub ?x 0) → ?x
    body.push(Node::if_then(
        Expr::and(
            Expr::var("is_wrap_sub"),
            Expr::and(
                Expr::var("r_is_lit_u32"),
                Expr::eq(Expr::var("r_val"), Expr::u32(0)),
            ),
        ),
        vec![Node::store(
            "rewrite_action",
            Expr::var("i"),
            Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
        )],
    ));
    body
}

/// Build the pattern-match analysis Program. Parallel kernel: each
/// GPU thread handles one Expr id via `gid_x()`. The orchestrator
/// dispatches `ceil(expr_count / 256)` workgroups.

pub fn build_pattern_match_program(expr_count: u32) -> Program {
    let buffers = vec![
        BufferDecl::storage("arena_kinds", 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg0", 1, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg1", 2, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg2", 3, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("rewrite_action", 4, BufferAccess::ReadWrite, DataType::U32)
            .with_count(expr_count.max(1)),
    ];

    let body = vec![
        Node::let_bind("i", Expr::gid_x()),
        Node::if_then(
            Expr::lt(Expr::var("i"), Expr::u32(expr_count)),
            per_expr_body(),
        ),
    ];

    Program::wrapped(buffers, [WORKGROUP_X, 1, 1], body)
}

fn per_expr_body() -> Vec<Node> {
    vec![
        Node::let_bind("kind", Expr::load("arena_kinds", Expr::var("i"))),
        Node::if_then(
            Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::BIN_OP)),
            bin_op_match_body(),
        ),
    ]
}

fn bin_op_match_body() -> Vec<Node> {
    // Look up op tag, child ids, and child kind+value.
    vec![
        Node::let_bind("op", Expr::load("arena_arg0", Expr::var("i"))),
        Node::let_bind("l", Expr::load("arena_arg1", Expr::var("i"))),
        Node::let_bind("r", Expr::load("arena_arg2", Expr::var("i"))),
        Node::let_bind("l_kind", Expr::load("arena_kinds", Expr::var("l"))),
        Node::let_bind("r_kind", Expr::load("arena_kinds", Expr::var("r"))),
        Node::let_bind("l_val", Expr::load("arena_arg0", Expr::var("l"))),
        Node::let_bind("r_val", Expr::load("arena_arg0", Expr::var("r"))),
        // Op tags: Add=0x01, Sub=0x02, Mul=0x03, BitAnd=0x06,
        // BitOr=0x07, BitXor=0x08, Eq=0x0B, Ne=0x0C, Lt=0x0D,
        // Gt=0x0E, Le=0x10, Ge=0x11, And=0x12, Or=0x13.
        Node::let_bind("is_add", Expr::eq(Expr::var("op"), Expr::u32(0x01))),
        Node::let_bind("is_sub", Expr::eq(Expr::var("op"), Expr::u32(0x02))),
        Node::let_bind("is_mul", Expr::eq(Expr::var("op"), Expr::u32(0x03))),
        Node::let_bind("is_bitand", Expr::eq(Expr::var("op"), Expr::u32(0x06))),
        Node::let_bind("is_bitor", Expr::eq(Expr::var("op"), Expr::u32(0x07))),
        Node::let_bind("is_bitxor", Expr::eq(Expr::var("op"), Expr::u32(0x08))),
        Node::let_bind("is_cmp_eq", Expr::eq(Expr::var("op"), Expr::u32(0x0B))),
        Node::let_bind("is_cmp_ne", Expr::eq(Expr::var("op"), Expr::u32(0x0C))),
        Node::let_bind("is_cmp_lt", Expr::eq(Expr::var("op"), Expr::u32(0x0D))),
        Node::let_bind("is_cmp_gt", Expr::eq(Expr::var("op"), Expr::u32(0x0E))),
        Node::let_bind("is_cmp_le", Expr::eq(Expr::var("op"), Expr::u32(0x10))),
        Node::let_bind("is_cmp_ge", Expr::eq(Expr::var("op"), Expr::u32(0x11))),
        Node::let_bind("is_bool_and", Expr::eq(Expr::var("op"), Expr::u32(0x12))),
        Node::let_bind("is_bool_or", Expr::eq(Expr::var("op"), Expr::u32(0x13))),
        Node::let_bind(
            "l_is_lit_bool",
            Expr::eq(Expr::var("l_kind"), Expr::u32(expr_kind::LIT_BOOL)),
        ),
        Node::let_bind(
            "r_is_lit_bool",
            Expr::eq(Expr::var("r_kind"), Expr::u32(expr_kind::LIT_BOOL)),
        ),
        Node::let_bind(
            "l_is_lit_u32",
            Expr::eq(Expr::var("l_kind"), Expr::u32(expr_kind::LIT_U32)),
        ),
        Node::let_bind(
            "r_is_lit_u32",
            Expr::eq(Expr::var("r_kind"), Expr::u32(expr_kind::LIT_U32)),
        ),
        // (Add 0 ?x) → ?x   (left child is LitU32(0); replace with right)
        Node::if_then(
            Expr::and(
                Expr::var("is_add"),
                Expr::and(
                    Expr::var("l_is_lit_u32"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
            )],
        ),
        // (Add ?x 0) → ?x   (right child is LitU32(0); replace with left)
        Node::if_then(
            Expr::and(
                Expr::var("is_add"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (Mul 1 ?x) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_mul"),
                Expr::and(
                    Expr::var("l_is_lit_u32"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(1)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
            )],
        ),
        // (Mul ?x 1) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_mul"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(1)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (Mul 0 ?x) → 0u32
        Node::if_then(
            Expr::and(
                Expr::var("is_mul"),
                Expr::and(
                    Expr::var("l_is_lit_u32"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
            )],
        ),
        // (Mul ?x 0) → 0u32
        Node::if_then(
            Expr::and(
                Expr::var("is_mul"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
            )],
        ),
        // (Sub ?x 0) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_sub"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (BitAnd ?x MAX) → ?x   (mask-everything And is identity)
        Node::if_then(
            Expr::and(
                Expr::var("is_bitand"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(u32::MAX)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (BitAnd MAX ?x) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_bitand"),
                Expr::and(
                    Expr::var("l_is_lit_u32"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(u32::MAX)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
            )],
        ),
        // (BitOr ?x MAX) → MAX  (saturated). Replace with right
        // (the literal MAX itself).
        Node::if_then(
            Expr::and(
                Expr::var("is_bitor"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(u32::MAX)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
            )],
        ),
        // (BitOr MAX ?x) → MAX  (saturated). Replace with left.
        Node::if_then(
            Expr::and(
                Expr::var("is_bitor"),
                Expr::and(
                    Expr::var("l_is_lit_u32"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(u32::MAX)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (BitAnd 0 ?x) → 0u32
        Node::if_then(
            Expr::and(
                Expr::var("is_bitand"),
                Expr::and(
                    Expr::var("l_is_lit_u32"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
            )],
        ),
        // (BitAnd ?x 0) → 0u32
        Node::if_then(
            Expr::and(
                Expr::var("is_bitand"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
            )],
        ),
        // (BitOr 0 ?x) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_bitor"),
                Expr::and(
                    Expr::var("l_is_lit_u32"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
            )],
        ),
        // (BitOr ?x 0) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_bitor"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (BitXor 0 ?x) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_bitxor"),
                Expr::and(
                    Expr::var("l_is_lit_u32"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
            )],
        ),
        // (BitXor ?x 0) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_bitxor"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (Div ?x 1) → ?x   -  division by 1 is identity. Op tag for
        // Div is 0x04. Reject divisor zero (no rule fires there).
        Node::let_bind("is_div", Expr::eq(Expr::var("op"), Expr::u32(0x04))),
        Node::if_then(
            Expr::and(
                Expr::var("is_div"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(1)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (Mod ?x 1) → 0   -  modulo 1 is always zero. Op tag 0x05.
        Node::let_bind("is_mod", Expr::eq(Expr::var("op"), Expr::u32(0x05))),
        Node::if_then(
            Expr::and(
                Expr::var("is_mod"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(1)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
            )],
        ),
        // Shl=0x09, Shr=0x0A. Shift-by-zero keeps the value; shift
        // of zero is always zero (any positive shift count).
        Node::let_bind("is_shl", Expr::eq(Expr::var("op"), Expr::u32(0x09))),
        Node::let_bind("is_shr", Expr::eq(Expr::var("op"), Expr::u32(0x0A))),
        // (Shl ?x 0) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_shl"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (Shr ?x 0) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_shr"),
                Expr::and(
                    Expr::var("r_is_lit_u32"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (Shl 0 ?x) → 0  (zero left-shifted by anything stays 0)
        Node::if_then(
            Expr::and(
                Expr::var("is_shl"),
                Expr::and(
                    Expr::var("l_is_lit_u32"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
            )],
        ),
        // (Shr 0 ?x) → 0  (zero right-shifted is still 0)
        Node::if_then(
            Expr::and(
                Expr::var("is_shr"),
                Expr::and(
                    Expr::var("l_is_lit_u32"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LIT_ZERO),
            )],
        ),
        // Bool And/Or identity rules. LitBool(true) is encoded as
        // arg0=1; LitBool(false) as arg0=0 in the arena.
        // (And ?x false) → false
        Node::if_then(
            Expr::and(
                Expr::var("is_bool_and"),
                Expr::and(
                    Expr::var("r_is_lit_bool"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LIT_FALSE),
            )],
        ),
        // (And false ?x) → false
        Node::if_then(
            Expr::and(
                Expr::var("is_bool_and"),
                Expr::and(
                    Expr::var("l_is_lit_bool"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LIT_FALSE),
            )],
        ),
        // (And ?x true) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_bool_and"),
                Expr::and(
                    Expr::var("r_is_lit_bool"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(1)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (And true ?x) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_bool_and"),
                Expr::and(
                    Expr::var("l_is_lit_bool"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(1)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
            )],
        ),
        // (Or ?x true) → true
        Node::if_then(
            Expr::and(
                Expr::var("is_bool_or"),
                Expr::and(
                    Expr::var("r_is_lit_bool"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(1)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LIT_TRUE),
            )],
        ),
        // (Or true ?x) → true
        Node::if_then(
            Expr::and(
                Expr::var("is_bool_or"),
                Expr::and(
                    Expr::var("l_is_lit_bool"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(1)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LIT_TRUE),
            )],
        ),
        // (Or ?x false) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_bool_or"),
                Expr::and(
                    Expr::var("r_is_lit_bool"),
                    Expr::eq(Expr::var("r_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_LEFT),
            )],
        ),
        // (Or false ?x) → ?x
        Node::if_then(
            Expr::and(
                Expr::var("is_bool_or"),
                Expr::and(
                    Expr::var("l_is_lit_bool"),
                    Expr::eq(Expr::var("l_val"), Expr::u32(0)),
                ),
            ),
            vec![Node::store(
                "rewrite_action",
                Expr::var("i"),
                Expr::u32(rewrite_action::REPLACE_WITH_RIGHT),
            )],
        ),
    ]
}

fn rewrite_program_with_actions(program: Program, actions: &[u32]) -> Program {
    super::rewrite_walk::rewrite_program_with_expr_rewriter(program, |expr, counter| {
        rewrite_expr(expr, actions, counter)
    })
}

fn rewrite_expr(expr: &Expr, actions: &[u32], counter: &mut u32) -> Expr {
    match expr {
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => {
            *counter += 1;
            expr.clone()
        }
        Expr::Load { buffer, index } => {
            let new_index = rewrite_expr(index, actions, counter);
            *counter += 1;
            Expr::Load {
                buffer: buffer.clone(),
                index: Box::new(new_index),
            }
        }
        Expr::BinOp { op, left, right } => {
            let new_left = rewrite_expr(left, actions, counter);
            let new_right = rewrite_expr(right, actions, counter);
            let id = *counter;
            *counter += 1;
            match actions
                .get(id as usize)
                .copied()
                .unwrap_or(rewrite_action::NONE)
            {
                rewrite_action::REPLACE_WITH_LEFT => new_left,
                rewrite_action::REPLACE_WITH_RIGHT => new_right,
                rewrite_action::REPLACE_WITH_LIT_ZERO => Expr::LitU32(0),
                _ => Expr::BinOp {
                    op: *op,
                    left: Box::new(new_left),
                    right: Box::new(new_right),
                },
            }
        }
        Expr::UnOp { op, operand } => {
            let new_operand = rewrite_expr(operand, actions, counter);
            *counter += 1;
            Expr::UnOp {
                op: op.clone(),
                operand: Box::new(new_operand),
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            let new_cond = rewrite_expr(cond, actions, counter);
            let new_true = rewrite_expr(true_val, actions, counter);
            let new_false = rewrite_expr(false_val, actions, counter);
            *counter += 1;
            Expr::Select {
                cond: Box::new(new_cond),
                true_val: Box::new(new_true),
                false_val: Box::new(new_false),
            }
        }
        Expr::Fma { a, b, c } => {
            let na = rewrite_expr(a, actions, counter);
            let nb = rewrite_expr(b, actions, counter);
            let nc = rewrite_expr(c, actions, counter);
            *counter += 1;
            Expr::Fma {
                a: Box::new(na),
                b: Box::new(nb),
                c: Box::new(nc),
            }
        }
        _ => expr.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;

    struct PatternDispatcher {
        outputs: Vec<Vec<u8>>,
    }

    impl OptimizerDispatcher for PatternDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            if inputs.len() != 5 {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: pattern test dispatcher expected 5 inputs, got {}.",
                    inputs.len()
                )));
            }
            Ok(self.outputs.clone())
        }
    }

    fn one_expr_arena() -> ExprArenaEncoding {
        ExprArenaEncoding {
            expr_count: 1,
            kinds: vec![expr_kind::LIT_U32],
            arg0: vec![0],
            arg1: vec![0],
            arg2: vec![0],
            ..ExprArenaEncoding::default()
        }
    }

    #[test]
    fn kernel_into_decodes_exact_actions_into_reused_buffer() {
        let dispatcher = PatternDispatcher {
            outputs: vec![u32_slice_to_le_bytes(&[rewrite_action::NONE])],
        };
        let mut actions = Vec::with_capacity(4);
        let ptr = actions.as_ptr();
        run_pattern_kernel_into(&one_expr_arena(), &dispatcher, &mut actions)
            .expect("Fix: dispatch succeeds");
        assert_eq!(actions, vec![rewrite_action::NONE]);
        assert_eq!(actions.as_ptr(), ptr);
    }

    #[test]
    fn kernel_with_scratch_reuses_dispatch_and_output_storage() {
        let dispatcher = PatternDispatcher {
            outputs: vec![u32_slice_to_le_bytes(&[rewrite_action::NONE])],
        };
        let arena = one_expr_arena();
        let mut scratch = PatternKernelScratch::default();
        let mut actions = Vec::with_capacity(1);

        run_pattern_kernel_with_scratch_into(&arena, &dispatcher, &mut scratch, &mut actions)
            .expect("Fix: dispatch succeeds");

        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        let actions_capacity = actions.capacity();

        run_pattern_kernel_with_scratch_into(&arena, &dispatcher, &mut scratch, &mut actions)
            .expect("Fix: dispatch succeeds");

        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
        assert_eq!(actions.capacity(), actions_capacity);
        assert_eq!(actions, vec![rewrite_action::NONE]);
    }

    #[test]
    fn kernel_rejects_extra_outputs() {
        let dispatcher = PatternDispatcher {
            outputs: vec![u32_slice_to_le_bytes(&[0]), u32_slice_to_le_bytes(&[0])],
        };
        let mut actions = Vec::new();
        let err = run_pattern_kernel_into(&one_expr_arena(), &dispatcher, &mut actions)
            .expect_err("extra outputs must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn kernel_rejects_trailing_action_bytes() {
        let dispatcher = PatternDispatcher {
            outputs: vec![vec![0, 0, 0, 0, 1]],
        };
        let mut actions = Vec::new();
        let err = run_pattern_kernel_into(&one_expr_arena(), &dispatcher, &mut actions)
            .expect_err("trailing bytes must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }
}
