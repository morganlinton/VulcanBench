//! Constant folding as a dispatched compute kernel.
//!
//! V1 scope: literals (`LitU32`) plus the integer-arithmetic
//! BinOps (`Add`, `Sub`, `Mul`, `BitAnd`, `BitOr`, `BitXor`) on u32.
//! Larger op coverage is mechanical extension of `build_const_fold_program`.
//!
//! Architecture: the encoder turns every Expr into a `(kind, arg0,
//! arg1, arg2, arg3)` row in the canonical `ExprArenaEncoding`. The
//! `build_const_fold_program` function constructs a vyre `Program`
//! that scans the arena bottom-up, marking each foldable Expr in a
//! `foldable[]` u32 buffer and writing its computed value into a
//! `value[]` u32 buffer. The `OptimizerDispatcher` runs that Program
//! on the GPU; the decoder walks the IR and rewrites every foldable
//! Expr into a literal.
//!
//! No host-reference escape in production. Tests parity vs the existing
//! `vyre-foundation` const-fold pass via `CpuOracleDispatcher`-style
//! tests (extension follow-up  -  for V1 we run through the real
//! `WgpuBackend` in the driver-wgpu integration test crate).

use std::sync::Arc;
use vyre_foundation::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::dispatch_buffers::{
    decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes, write_zero_bytes,
};

use super::dispatcher::{DispatchError, OptimizerDispatcher};
use super::encode::EncodeError;
use super::expr_arena::{encode_expr_arena, expr_kind, ExprArenaEncoding};

#[derive(Debug, Default)]
struct ConstFoldKernelScratch {
    inputs: Vec<Vec<u8>>,
    current_level: [u32; 1],
}

/// Errors surfaced by `gpu_const_fold`.
#[derive(Debug)]
pub enum ConstFoldError {
    /// Encoder did not accept the input shape.
    Encode(EncodeError),
    /// Dispatcher rejected or failed to run the analysis Program.
    Dispatch(DispatchError),
}

impl std::fmt::Display for ConstFoldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(err) => write!(f, "gpu_const_fold encode error: {err:?}"),
            Self::Dispatch(err) => write!(f, "gpu_const_fold dispatch error: {err}"),
        }
    }
}

impl std::error::Error for ConstFoldError {}

/// Run constant-folding on `program` by encoding its Expr arena,
/// dispatching the bottom-up evaluator Program through `dispatcher`,
/// and rewriting every foldable Expr in the input Program into the
/// computed literal value.
pub fn gpu_const_fold(
    program: Program,
    dispatcher: &dyn OptimizerDispatcher,
) -> Result<Program, ConstFoldError> {
    let arena = encode_expr_arena(&program).map_err(ConstFoldError::Encode)?;
    if arena.expr_count == 0 {
        return Ok(program);
    }
    let mut scratch = ConstFoldKernelScratch::default();
    let mut foldable = Vec::with_capacity(arena.expr_count as usize);
    let mut value = Vec::with_capacity(arena.expr_count as usize);
    run_const_fold_kernel_with_scratch_into(
        &arena,
        dispatcher,
        &mut scratch,
        &mut foldable,
        &mut value,
    )
    .map_err(ConstFoldError::Dispatch)?;
    Ok(rewrite_program_with_folded_values(
        program, &arena, &foldable, &value,
    ))
}

#[cfg(test)]
fn run_const_fold_kernel_into(
    arena: &ExprArenaEncoding,
    dispatcher: &dyn OptimizerDispatcher,
    foldable: &mut Vec<u32>,
    value: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = ConstFoldKernelScratch::default();
    run_const_fold_kernel_with_scratch_into(arena, dispatcher, &mut scratch, foldable, value)
}

fn run_const_fold_kernel_with_scratch_into(
    arena: &ExprArenaEncoding,
    dispatcher: &dyn OptimizerDispatcher,
    scratch: &mut ConstFoldKernelScratch,
    foldable: &mut Vec<u32>,
    value: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let n = arena.expr_count;
    let analysis = build_const_fold_program(n);
    let words = n as usize;
    let state_bytes = words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: const-fold state byte count overflows usize for expr_count={n}."
            ))
        })?;

    // Level-parallel kernel: each dispatch processes all Exprs at one
    // depth level in parallel via gid_x(). The host loops levels
    // 0..=max_depth, updating the `current_level` buffer between
    // dispatches. Foldable + value buffers persist their state across
    // levels (we re-feed the previous output as the next input).
    //
    // Buffer order matches `build_const_fold_program`'s declarations:
    //   0: arena_kinds (RO)
    //   1: arena_arg0 (RO)
    //   2: arena_arg1 (RO)
    //   3: arena_arg2 (RO)
    //   4: arena_depths (RO)  -  per-Expr depth
    //   5: current_level (RO)  -  single u32, varied per dispatch
    //   6: foldable (RW; init zeros, persists across levels)
    //   7: value (RW; init zeros, persists across levels)
    ensure_input_slots(&mut scratch.inputs, 8);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], &arena.kinds);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], &arena.arg0);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], &arena.arg1);
    write_u32_slice_le_bytes(&mut scratch.inputs[3], &arena.arg2);
    write_u32_slice_le_bytes(&mut scratch.inputs[4], &arena.depths);
    write_zero_bytes(&mut scratch.inputs[6], state_bytes);
    write_zero_bytes(&mut scratch.inputs[7], state_bytes);

    let grid_x = (n + WORKGROUP_X - 1) / WORKGROUP_X;

    for level in 0..=arena.max_depth {
        scratch.current_level[0] = level;
        write_u32_slice_le_bytes(&mut scratch.inputs[5], &scratch.current_level);

        let outputs = dispatcher.dispatch(&analysis, &scratch.inputs, Some([grid_x, 1, 1]))?;
        if outputs.len() != 2 {
            return Err(DispatchError::BackendError(format!(
                "Fix: const-fold dispatch expected exactly 2 outputs (foldable, value), got {}.",
                outputs.len()
            )));
        }
        decode_u32_output_exact(&outputs[0], words, "const-fold foldable", foldable)?;
        decode_u32_output_exact(&outputs[1], words, "const-fold value", value)?;
        // Carry RW state forward to the next level dispatch.
        scratch.inputs[6].clear();
        scratch.inputs[6].extend_from_slice(&outputs[0]);
        scratch.inputs[7].clear();
        scratch.inputs[7].extend_from_slice(&outputs[1]);
    }

    Ok(())
}

/// Workgroup size for the level-parallel const-fold kernel.
const WORKGROUP_X: u32 = 256;

/// Build the FUSED const-fold analysis Program: a single dispatch that
/// internally iterates `level` from 0..=`max_depth`, with a workgroup-
/// scope barrier between levels. Eliminates the per-level host
/// dispatch loop that dominates chain-shaped Programs.
///
/// Single-workgroup design (`workgroup_size = [256, 1, 1]`, grid =
/// `[1, 1, 1]`). Each thread strides over `expr_count` exprs in
/// chunks of 256 per outer-level iteration. Workgroup-scope `SeqCst`
/// barrier between levels ensures stores from level `k` are visible
/// to reads at level `k+1`.
///
/// Buffer layout (caller-supplied resident handles, in order):
///   0: arena_kinds (RO)
///   1: arena_arg0  (RO)
///   2: arena_arg1  (RO)
///   3: arena_arg2  (RO)
///   4: arena_depths (RO)
///   5: max_depth_buf (RO; single u32 = max depth in arena)
///   6: foldable    (RW; init zeros)
///   7: value       (RW; init zeros)
///
/// Constraints:
///   - `expr_count` may be larger than `WORKGROUP_X`; the kernel
///     strides via an inner Loop. Single-workgroup means workgroup-
///     scope barriers (SeqCst) are sufficient  -  no GridSync needed.
///   - `max_depth_iter_cap` is the static upper bound on the outer
///     Loop in the IR. The actual depth is read from `max_depth_buf`
///     at runtime; the kernel breaks out early when `level >
///     max_depth`. Caller passes a generous bound (e.g. `expr_count`).
#[must_use]
pub fn build_const_fold_program_fused(expr_count: u32, max_depth_iter_cap: u32) -> Program {
    let buffers = vec![
        BufferDecl::storage("arena_kinds", 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg0", 1, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg1", 2, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg2", 3, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_depths", 4, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("max_depth_buf", 5, BufferAccess::ReadOnly, DataType::U32)
            .with_count(1),
        BufferDecl::storage("foldable", 6, BufferAccess::ReadWrite, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("value", 7, BufferAccess::ReadWrite, DataType::U32)
            .with_count(expr_count.max(1)),
    ];

    // Number of stride chunks needed to cover all exprs with WORKGROUP_X
    // threads. Static upper bound  -  the kernel re-checks `i < expr_count`
    // each iteration so over-shoot is safe.
    let chunk_cap = (expr_count + WORKGROUP_X - 1) / WORKGROUP_X;

    // Per-level body: each thread strides over its share of exprs and
    // runs `per_expr_body` against any expr at the current level.
    let chunk_loop = Node::loop_for(
        "chunk",
        Expr::u32(0),
        Expr::u32(chunk_cap.max(1)),
        vec![
            Node::let_bind(
                "i",
                Expr::add(
                    Expr::gid_x(),
                    Expr::mul(Expr::var("chunk"), Expr::u32(WORKGROUP_X)),
                ),
            ),
            Node::if_then(
                Expr::lt(Expr::var("i"), Expr::u32(expr_count)),
                vec![
                    Node::let_bind("my_depth", Expr::load("arena_depths", Expr::var("i"))),
                    Node::if_then(
                        Expr::eq(Expr::var("my_depth"), Expr::var("level")),
                        per_expr_body(),
                    ),
                ],
            ),
        ],
    );

    let outer = Node::loop_for(
        "level",
        Expr::u32(0),
        Expr::u32(max_depth_iter_cap.max(1)),
        vec![
            // Early-out: if level > max_depth, skip the body. No way
            // to break the loop early in the IR, so we just gate the
            // body. The barrier still fires; cheap.
            Node::let_bind("md", Expr::load("max_depth_buf", Expr::u32(0))),
            Node::if_then(
                Expr::le(Expr::var("level"), Expr::var("md")),
                vec![chunk_loop],
            ),
            // Workgroup-scope barrier: stores from this level visible
            // to reads at level+1. Single-workgroup design means this
            // is sufficient  -  no GridSync needed.
            Node::Barrier {
                ordering: vyre_foundation::MemoryOrdering::SeqCst,
            },
        ],
    );

    Program::wrapped(buffers, [WORKGROUP_X, 1, 1], vec![outer])
}

/// Build the const-fold analysis Program. Level-parallel kernel: each
/// GPU thread handles one Expr id via `gid_x()` and acts only when
/// the Expr's depth equals `current_level[0]`. The orchestrator
/// dispatches once per level (0..=max_depth), with foldable + value
/// buffers persisting their state across dispatches.
pub fn build_const_fold_program(expr_count: u32) -> Program {
    let buffers = vec![
        BufferDecl::storage("arena_kinds", 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg0", 1, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg1", 2, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_arg2", 3, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("arena_depths", 4, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("current_level", 5, BufferAccess::ReadOnly, DataType::U32)
            .with_count(1),
        BufferDecl::storage("foldable", 6, BufferAccess::ReadWrite, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("value", 7, BufferAccess::ReadWrite, DataType::U32)
            .with_count(expr_count.max(1)),
    ];

    let body = vec![
        Node::let_bind("i", Expr::gid_x()),
        Node::if_then(
            Expr::lt(Expr::var("i"), Expr::u32(expr_count)),
            vec![
                Node::let_bind("my_depth", Expr::load("arena_depths", Expr::var("i"))),
                Node::let_bind("level", Expr::load("current_level", Expr::u32(0))),
                Node::if_then(
                    Expr::eq(Expr::var("my_depth"), Expr::var("level")),
                    per_expr_body(),
                ),
            ],
        ),
    ];

    Program::wrapped(buffers, [WORKGROUP_X, 1, 1], body)
}

/// Per-Expr-id body of the sequential const-fold scan.
fn per_expr_body() -> Vec<Node> {
    vec![
        // let kind = arena_kinds[i]
        Node::let_bind("kind", Expr::load("arena_kinds", Expr::var("i"))),
        // Literal kinds: foldable=1, value = arena_arg0[i].
        // (V1 covers LitU32 only; LitI32/F32/Bool fold by reinterpret
        //  but the kernel emits the same payload bits, so adding their
        //  kind discriminants follows the same pattern with no new
        //  arithmetic.)
        Node::if_then(
            Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::LIT_U32)),
            vec![
                Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                Node::store(
                    "value",
                    Expr::var("i"),
                    Expr::load("arena_arg0", Expr::var("i")),
                ),
            ],
        ),
        // BIN_OP: only fold if both operands are themselves foldable.
        Node::if_then(
            Expr::eq(Expr::var("kind"), Expr::u32(expr_kind::BIN_OP)),
            bin_op_body(),
        ),
    ]
}

/// Body of the BIN_OP arm: read op tag + child ids, check both
/// children are foldable, compute the result if the op is one of the
/// V1-supported integer arithmetic ops.
fn bin_op_body() -> Vec<Node> {
    vec![
        Node::let_bind("op", Expr::load("arena_arg0", Expr::var("i"))),
        Node::let_bind("l", Expr::load("arena_arg1", Expr::var("i"))),
        Node::let_bind("r", Expr::load("arena_arg2", Expr::var("i"))),
        Node::let_bind("lf", Expr::load("foldable", Expr::var("l"))),
        Node::let_bind("rf", Expr::load("foldable", Expr::var("r"))),
        Node::if_then(
            // lf == 1 && rf == 1
            Expr::and(
                Expr::eq(Expr::var("lf"), Expr::u32(1)),
                Expr::eq(Expr::var("rf"), Expr::u32(1)),
            ),
            vec![
                Node::let_bind("lv", Expr::load("value", Expr::var("l"))),
                Node::let_bind("rv", Expr::load("value", Expr::var("r"))),
                // Add (tag 0x01)
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x01)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::add(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // Sub (tag 0x02)
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x02)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::sub(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // Mul (tag 0x03)
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x03)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::mul(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // BitAnd (tag 0x06)
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x06)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::bitand(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // BitOr (tag 0x07)
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x07)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::bitor(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // BitXor (tag 0x08)
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x08)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::bitxor(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // Shl (tag 0x09). u32 shift; rv must be in 0..32 to
                // be well-defined. We fold for any rv since the
                // wrapping behaviour matches WGSL/PTX semantics.
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x09)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::shl(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // Shr (tag 0x0A)  -  logical shift right.
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x0A)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::shr(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // Div (tag 0x04)  -  fold only if `rv != 0`. Folding
                // a division by zero would crash the compiler at
                // emit time; the host-side rewriter still emits the
                // original Div which lets the program's own runtime
                // semantics decide.
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("op"), Expr::u32(0x04)),
                        Expr::ne(Expr::var("rv"), Expr::u32(0)),
                    ),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::div(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // Mod (tag 0x05)  -  same divide-by-zero guard.
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("op"), Expr::u32(0x05)),
                        Expr::ne(Expr::var("rv"), Expr::u32(0)),
                    ),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::rem(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // Min (tag 0x15)  -  `lv if lv < rv else rv`. Folded
                // via a Select gated on Lt; works for u32 directly.
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x15)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::lt(Expr::var("lv"), Expr::var("rv"))),
                                true_val: Box::new(Expr::var("lv")),
                                false_val: Box::new(Expr::var("rv")),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // Max (tag 0x16)  -  symmetric.
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x16)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::gt(Expr::var("lv"), Expr::var("rv"))),
                                true_val: Box::new(Expr::var("lv")),
                                false_val: Box::new(Expr::var("rv")),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // AbsDiff (tag 0x14)  -  `|lv - rv|` for u32 = if lv >
                // rv then lv-rv else rv-lv. Always non-negative.
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x14)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::gt(Expr::var("lv"), Expr::var("rv"))),
                                true_val: Box::new(Expr::sub(Expr::var("lv"), Expr::var("rv"))),
                                false_val: Box::new(Expr::sub(Expr::var("rv"), Expr::var("lv"))),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // SaturatingAdd (0x17)  -  clamps to u32::MAX when the
                // unsaturated sum would overflow. Detect overflow by
                // checking if the wrapped sum is less than either
                // operand (carry happened).
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x17)),
                    vec![
                        Node::let_bind("sat_sum", Expr::add(Expr::var("lv"), Expr::var("rv"))),
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::lt(Expr::var("sat_sum"), Expr::var("lv"))),
                                true_val: Box::new(Expr::u32(u32::MAX)),
                                false_val: Box::new(Expr::var("sat_sum")),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // SaturatingSub (0x18)  -  clamps to 0 when rv > lv.
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x18)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::ge(Expr::var("lv"), Expr::var("rv"))),
                                true_val: Box::new(Expr::sub(Expr::var("lv"), Expr::var("rv"))),
                                false_val: Box::new(Expr::u32(0)),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // WrappingAdd (0x20)  -  same as `Add` for u32 since
                // backend Add already wraps. Fold straight through.
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x20)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::add(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // WrappingSub (0x21)  -  same as `Sub` for u32.
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x21)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::sub(Expr::var("lv"), Expr::var("rv")),
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // Comparison ops (Eq=0x0B, Ne=0x0C, Lt=0x0D, Gt=0x0E,
                // Le=0x10, Ge=0x11). Writes 0/1 into `value` because
                // the decoder reconstructs LitU32; downstream
                // dead-branch elimination accepts both LitU32(0|1)
                // and LitBool. CPU const-prop re-types to LitBool
                // when it later substitutes through a Var.
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x0B)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::eq(Expr::var("lv"), Expr::var("rv"))),
                                true_val: Box::new(Expr::u32(1)),
                                false_val: Box::new(Expr::u32(0)),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x0C)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::ne(Expr::var("lv"), Expr::var("rv"))),
                                true_val: Box::new(Expr::u32(1)),
                                false_val: Box::new(Expr::u32(0)),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x0D)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::lt(Expr::var("lv"), Expr::var("rv"))),
                                true_val: Box::new(Expr::u32(1)),
                                false_val: Box::new(Expr::u32(0)),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x0E)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::gt(Expr::var("lv"), Expr::var("rv"))),
                                true_val: Box::new(Expr::u32(1)),
                                false_val: Box::new(Expr::u32(0)),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x10)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::le(Expr::var("lv"), Expr::var("rv"))),
                                true_val: Box::new(Expr::u32(1)),
                                false_val: Box::new(Expr::u32(0)),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x11)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::ge(Expr::var("lv"), Expr::var("rv"))),
                                true_val: Box::new(Expr::u32(1)),
                                false_val: Box::new(Expr::u32(0)),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // RotateLeft (0x1E). The backend implements rotate
                // natively; the kernel just emits the BinOp.
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x1E)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::BinOp {
                                op: BinOp::RotateLeft,
                                left: Box::new(Expr::var("lv")),
                                right: Box::new(Expr::var("rv")),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // RotateRight (0x1F).
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x1F)),
                    vec![
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::BinOp {
                                op: BinOp::RotateRight,
                                left: Box::new(Expr::var("lv")),
                                right: Box::new(Expr::var("rv")),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
                // SaturatingMul (0x19). For u32, overflow detection
                // via the inverse-divide identity: if (lv*rv)/lv == rv
                // then no overflow. Guard the divisor with Select to
                // avoid div-by-zero when lv == 0 (in which case the
                // product is trivially 0, no overflow).
                Node::if_then(
                    Expr::eq(Expr::var("op"), Expr::u32(0x19)),
                    vec![
                        Node::let_bind("sm_prod", Expr::mul(Expr::var("lv"), Expr::var("rv"))),
                        Node::let_bind(
                            "sm_divisor",
                            Expr::Select {
                                cond: Box::new(Expr::eq(Expr::var("lv"), Expr::u32(0))),
                                true_val: Box::new(Expr::u32(1)),
                                false_val: Box::new(Expr::var("lv")),
                            },
                        ),
                        Node::let_bind(
                            "sm_quot",
                            Expr::div(Expr::var("sm_prod"), Expr::var("sm_divisor")),
                        ),
                        Node::let_bind(
                            "sm_no_overflow",
                            Expr::or(
                                Expr::eq(Expr::var("lv"), Expr::u32(0)),
                                Expr::eq(Expr::var("sm_quot"), Expr::var("rv")),
                            ),
                        ),
                        Node::store(
                            "value",
                            Expr::var("i"),
                            Expr::Select {
                                cond: Box::new(Expr::var("sm_no_overflow")),
                                true_val: Box::new(Expr::var("sm_prod")),
                                false_val: Box::new(Expr::u32(u32::MAX)),
                            },
                        ),
                        Node::store("foldable", Expr::var("i"), Expr::u32(1)),
                    ],
                ),
            ],
        ),
    ]
}

fn rewrite_program_with_folded_values(
    program: Program,
    arena: &ExprArenaEncoding,
    foldable: &[u32],
    value: &[u32],
) -> Program {
    // Rebuild the entry tree, walking Exprs in the same post-order
    // the encoder used. Each Expr we encounter consumes one slot of
    // the arena (and any children it has). If the Expr is foldable,
    // replace it with a LitU32(value).
    let body: Vec<Node> = match program.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    };

    let mut counter = 0u32;
    let rebuilt = rewrite_scope(&body, arena, foldable, value, &mut counter);

    let new_entry = match program.entry() {
        [Node::Region {
            generator,
            source_region,
            ..
        }] => vec![Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(rebuilt),
        }],
        _ => rebuilt,
    };
    program.with_rewritten_entry(new_entry)
}

fn rewrite_scope(
    body: &[Node],
    arena: &ExprArenaEncoding,
    foldable: &[u32],
    value: &[u32],
    counter: &mut u32,
) -> Vec<Node> {
    let prefix_len = super::encode::reachable_prefix_len(body);
    let mut out = Vec::with_capacity(prefix_len);
    for node in &body[..prefix_len] {
        out.push(rewrite_node(node, arena, foldable, value, counter));
    }
    out
}

fn rewrite_node(
    node: &Node,
    arena: &ExprArenaEncoding,
    foldable: &[u32],
    value: &[u32],
    counter: &mut u32,
) -> Node {
    match node {
        Node::Let { name, value: e } => Node::let_bind(
            name.clone(),
            rewrite_expr(e, arena, foldable, value, counter),
        ),
        Node::Assign { name, value: e } => Node::assign(
            name.clone(),
            rewrite_expr(e, arena, foldable, value, counter),
        ),
        Node::Store {
            buffer,
            index,
            value: e,
        } => Node::store(
            buffer.clone(),
            rewrite_expr(index, arena, foldable, value, counter),
            rewrite_expr(e, arena, foldable, value, counter),
        ),
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::if_then_else(
            rewrite_expr(cond, arena, foldable, value, counter),
            rewrite_scope(then, arena, foldable, value, counter),
            rewrite_scope(otherwise, arena, foldable, value, counter),
        ),
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::loop_for(
            var.clone(),
            rewrite_expr(from, arena, foldable, value, counter),
            rewrite_expr(to, arena, foldable, value, counter),
            rewrite_scope(body, arena, foldable, value, counter),
        ),
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } => Node::AsyncLoad {
            source: source.clone(),
            destination: destination.clone(),
            offset: Box::new(rewrite_expr(offset, arena, foldable, value, counter)),
            size: Box::new(rewrite_expr(size, arena, foldable, value, counter)),
            tag: tag.clone(),
        },
        Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            tag,
        } => Node::AsyncStore {
            source: source.clone(),
            destination: destination.clone(),
            offset: Box::new(rewrite_expr(offset, arena, foldable, value, counter)),
            size: Box::new(rewrite_expr(size, arena, foldable, value, counter)),
            tag: tag.clone(),
        },
        Node::Trap { address, tag } => Node::Trap {
            address: Box::new(rewrite_expr(address, arena, foldable, value, counter)),
            tag: tag.clone(),
        },
        Node::Block(body) => Node::Block(rewrite_scope(body, arena, foldable, value, counter)),
        Node::Region {
            generator,
            source_region,
            body,
        } => Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(rewrite_scope(
                body.as_slice(),
                arena,
                foldable,
                value,
                counter,
            )),
        },
        // No-Expr-payload Nodes pass through.
        Node::Return
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => node.clone(),
        // Future variants  -  leave untouched.
        _ => node.clone(),
    }
}

#[allow(clippy::only_used_in_recursion)]
fn rewrite_expr(
    expr: &Expr,
    arena: &ExprArenaEncoding,
    foldable: &[u32],
    value: &[u32],
    counter: &mut u32,
) -> Expr {
    // Determine this Expr's id by mirroring the encoder's post-order
    // walk: recurse into children first, then this Expr's own slot.
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
            let id = *counter;
            *counter += 1;
            decide(expr, id, foldable, value)
        }
        Expr::Load { buffer, index } => {
            let new_index = rewrite_expr(index, arena, foldable, value, counter);
            let id = *counter;
            *counter += 1;
            // Loads are not foldable; reuse the rewritten index.
            let _ = (foldable, value, id);
            Expr::Load {
                buffer: buffer.clone(),
                index: Box::new(new_index),
            }
        }
        Expr::BinOp { op, left, right } => {
            let new_left = rewrite_expr(left, arena, foldable, value, counter);
            let new_right = rewrite_expr(right, arena, foldable, value, counter);
            let id = *counter;
            *counter += 1;
            if foldable[id as usize] == 1 {
                Expr::LitU32(value[id as usize])
            } else {
                Expr::BinOp {
                    op: *op,
                    left: Box::new(new_left),
                    right: Box::new(new_right),
                }
            }
        }
        Expr::UnOp { op, operand } => {
            let new_operand = rewrite_expr(operand, arena, foldable, value, counter);
            let id = *counter;
            *counter += 1;
            if foldable[id as usize] == 1 {
                Expr::LitU32(value[id as usize])
            } else {
                Expr::UnOp {
                    op: op.clone(),
                    operand: Box::new(new_operand),
                }
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            let new_cond = rewrite_expr(cond, arena, foldable, value, counter);
            let new_true = rewrite_expr(true_val, arena, foldable, value, counter);
            let new_false = rewrite_expr(false_val, arena, foldable, value, counter);
            let id = *counter;
            *counter += 1;
            let _ = (foldable, value, id);
            Expr::Select {
                cond: Box::new(new_cond),
                true_val: Box::new(new_true),
                false_val: Box::new(new_false),
            }
        }
        Expr::Fma { a, b, c } => {
            let na = rewrite_expr(a, arena, foldable, value, counter);
            let nb = rewrite_expr(b, arena, foldable, value, counter);
            let nc = rewrite_expr(c, arena, foldable, value, counter);
            let id = *counter;
            *counter += 1;
            let _ = (foldable, value, id);
            Expr::Fma {
                a: Box::new(na),
                b: Box::new(nb),
                c: Box::new(nc),
            }
        }
        // Unsupported Expr variants pass through unchanged. The
        // encoder bails on these, so we never reach this arm during
        // a Program the encoder accepted.
        _ => expr.clone(),
    }
}

fn decide(expr: &Expr, id: u32, foldable: &[u32], value: &[u32]) -> Expr {
    if foldable[id as usize] == 1 {
        // V1: only u32 literals fold from the kernel. Other literal
        // kinds ride through their own encode → lit branch as future
        // extensions.
        match expr {
            Expr::LitU32(_) | Expr::LitI32(_) | Expr::LitF32(_) | Expr::LitBool(_) => expr.clone(),
            _ => Expr::LitU32(value[id as usize]),
        }
    } else {
        expr.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;

    struct ConstFoldDispatcher {
        outputs: Vec<Vec<u8>>,
    }

    impl OptimizerDispatcher for ConstFoldDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            // The orchestrator always passes Some([grid_x, 1, 1]) where
            // grid_x = ceil(expr_count / WORKGROUP_X). For a 1-expr arena
            // grid_x = 1; for 257-expr arena grid_x = 2. Asserting
            // Some([1, 1, 1]) is only accidentally correct for the trivial
            // one-expr case. Assert the invariant that y and z are always 1.
            if let Some(grid) = grid_override {
                assert_eq!(
                    grid[1], 1,
                    "const-fold grid y-dimension must always be 1; got {grid:?}"
                );
                assert_eq!(
                    grid[2], 1,
                    "const-fold grid z-dimension must always be 1; got {grid:?}"
                );
                // Derive the expected expr_count from the arena_kinds buffer
                // (input slot 0, one u32 per expr).
                let expected_expr_count = (inputs[0].len() / 4) as u32;
                let expected_grid_x = (expected_expr_count + super::WORKGROUP_X - 1)
                    / super::WORKGROUP_X;
                assert_eq!(
                    grid[0],
                    expected_grid_x,
                    "const-fold grid x must equal ceil(expr_count / WORKGROUP_X) = {expected_grid_x}; got {grid:?}"
                );
            }
            if inputs.len() != 8 {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: const-fold test dispatcher expected 8 inputs, got {}.",
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
            depths: vec![0],
            max_depth: 0,
            ..ExprArenaEncoding::default()
        }
    }

    #[test]
    fn kernel_into_decodes_exact_outputs_into_reused_buffers() {
        let dispatcher = ConstFoldDispatcher {
            outputs: vec![u32_slice_to_le_bytes(&[1]), u32_slice_to_le_bytes(&[7])],
        };
        let mut foldable = Vec::with_capacity(4);
        let mut value = Vec::with_capacity(4);
        let foldable_ptr = foldable.as_ptr();
        let value_ptr = value.as_ptr();
        run_const_fold_kernel_into(&one_expr_arena(), &dispatcher, &mut foldable, &mut value)
            .expect("Fix: dispatch succeeds");
        assert_eq!(foldable, vec![1]);
        assert_eq!(value, vec![7]);
        assert_eq!(foldable.as_ptr(), foldable_ptr);
        assert_eq!(value.as_ptr(), value_ptr);
    }

    #[test]
    fn kernel_with_scratch_reuses_dispatch_state_and_outputs() {
        let dispatcher = ConstFoldDispatcher {
            outputs: vec![u32_slice_to_le_bytes(&[1]), u32_slice_to_le_bytes(&[7])],
        };
        let arena = one_expr_arena();
        let mut scratch = ConstFoldKernelScratch::default();
        let mut foldable = Vec::with_capacity(1);
        let mut value = Vec::with_capacity(1);

        run_const_fold_kernel_with_scratch_into(
            &arena,
            &dispatcher,
            &mut scratch,
            &mut foldable,
            &mut value,
        )
        .expect("Fix: dispatch succeeds");

        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        let foldable_capacity = foldable.capacity();
        let value_capacity = value.capacity();

        run_const_fold_kernel_with_scratch_into(
            &arena,
            &dispatcher,
            &mut scratch,
            &mut foldable,
            &mut value,
        )
        .expect("Fix: dispatch succeeds");

        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
        assert_eq!(foldable.capacity(), foldable_capacity);
        assert_eq!(value.capacity(), value_capacity);
        assert_eq!(foldable, vec![1]);
        assert_eq!(value, vec![7]);
    }

    #[test]
    fn kernel_rejects_extra_outputs() {
        let dispatcher = ConstFoldDispatcher {
            outputs: vec![
                u32_slice_to_le_bytes(&[1]),
                u32_slice_to_le_bytes(&[7]),
                u32_slice_to_le_bytes(&[0]),
            ],
        };
        let mut foldable = Vec::new();
        let mut value = Vec::new();
        let err =
            run_const_fold_kernel_into(&one_expr_arena(), &dispatcher, &mut foldable, &mut value)
                .expect_err("extra outputs must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn kernel_rejects_trailing_value_bytes() {
        let dispatcher = ConstFoldDispatcher {
            outputs: vec![u32_slice_to_le_bytes(&[1]), vec![7, 0, 0, 0, 1]],
        };
        let mut foldable = Vec::new();
        let mut value = Vec::new();
        let err =
            run_const_fold_kernel_into(&one_expr_arena(), &dispatcher, &mut foldable, &mut value)
                .expect_err("trailing bytes must be rejected");
        assert!(
            matches!(err, DispatchError::BackendError(_)),
            "unexpected error: {err:?}"
        );
    }

    /// P2 regression: the const-fold dispatcher receives `Some([grid_x, 1, 1])`
    /// where `grid_x = ceil(expr_count / WORKGROUP_X)`. For a 257-expr arena
    /// `grid_x` must be 2. The previous `ConstFoldDispatcher` asserted
    /// `Some([1, 1, 1])` which accidentally passed only for the trivial 1-expr
    /// case and would panic (or silently accept) for any real multi-workgroup
    /// arena, hiding the fact that multi-workgroup dispatch was never exercised.
    #[test]
    fn const_fold_kernel_sends_correct_multi_workgroup_grid_for_257_exprs() {
        struct GridCapture {
            /// Store the x-grid from each dispatch call.
            grid_x_values: std::cell::RefCell<Vec<u32>>,
            expr_count: u32,
        }
        impl OptimizerDispatcher for GridCapture {
            fn dispatch(
                &self,
                _program: &Program,
                _inputs: &[Vec<u8>],
                grid_override: Option<[u32; 3]>,
            ) -> Result<Vec<Vec<u8>>, DispatchError> {
                let gx = grid_override.map(|g| g[0]).unwrap_or(0);
                self.grid_x_values.borrow_mut().push(gx);
                let n = self.expr_count as usize;
                Ok(vec![
                    u32_slice_to_le_bytes(&vec![0u32; n]),
                    u32_slice_to_le_bytes(&vec![0u32; n]),
                ])
            }
        }

        let n: u32 = 257;
        let arena = ExprArenaEncoding {
            expr_count: n,
            kinds: vec![expr_kind::LIT_U32; n as usize],
            arg0: vec![0u32; n as usize],
            arg1: vec![0u32; n as usize],
            arg2: vec![0u32; n as usize],
            depths: vec![0u32; n as usize],
            max_depth: 0,
            ..ExprArenaEncoding::default()
        };
        let dispatcher = GridCapture {
            grid_x_values: std::cell::RefCell::new(Vec::new()),
            expr_count: n,
        };
        let mut foldable = Vec::new();
        let mut value = Vec::new();
        run_const_fold_kernel_into(&arena, &dispatcher, &mut foldable, &mut value)
            .expect("Fix: 257-expr const-fold dispatch succeeds");

        assert_eq!(foldable.len(), n as usize);
        // For n=257 and WORKGROUP_X=256: ceil(257/256) = 2.
        let expected_grid_x = (n + WORKGROUP_X - 1) / WORKGROUP_X;
        assert_eq!(expected_grid_x, 2, "sanity: expected_grid_x for 257 exprs must be 2");
        for (dispatch_idx, &gx) in dispatcher.grid_x_values.borrow().iter().enumerate() {
            assert_eq!(
                gx,
                expected_grid_x,
                "dispatch {dispatch_idx}: grid_x must be {expected_grid_x} for expr_count={n}; \
                 the prior ConstFoldDispatcher asserted Some([1,1,1]) and only worked by \
                 accident for 1-expr arenas"
            );
        }
    }
}
