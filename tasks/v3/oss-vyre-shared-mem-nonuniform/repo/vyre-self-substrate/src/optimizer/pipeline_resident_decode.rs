//! Combined arena-pass delta application.
//!
//! Walks the input Program in the same DFS post-order as the
//! ExprArena encoder, applying the canonicalize swap_mask, const-fold
//! foldable+value, and pattern-match rewrite_action in priority order:
//!
//! 1. **const-fold** wins: if `foldable[id] == 1`, replace the Expr
//!    with `LitU32(value[id])`.
//! 2. **pattern-match** next: apply the `rewrite_action` from the
//!    pattern bank (replace with left/right child or LitU32(0)).
//! 3. **canonicalize** last: if it's a BinOp and `swap_mask[id] == 1`,
//!    swap operands.
//!
//! Per the V1 rule sets (see module docs in `pipeline_resident`),
//! this priority is sound  -  the three passes are independent at the
//! Expr level for the rules currently shipped.

use std::sync::Arc;

use rustc_hash::FxHashMap;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::pattern_match_via_encoded::rewrite_action as ra;

/// Lookup contract for arena-pass deltas.
pub trait ArenaDeltaLookup {
    /// Canonicalization swap flag for `id`.
    fn swap_mask(&self, id: usize) -> u32;

    /// Const-fold flag for `id`.
    fn foldable(&self, id: usize) -> u32;

    /// Const-fold value for `id`.
    fn value(&self, id: usize) -> u32;

    /// Pattern-match rewrite action for `id`.
    fn rewrite_action(&self, id: usize) -> u32;
}

struct DenseArenaDeltas<'a> {
    swap_mask: &'a [u32],
    foldable: &'a [u32],
    value: &'a [u32],
    rewrite_action: &'a [u32],
}

/// Fixed-size compressed arena deltas: bitsets for boolean planes,
/// dense u32 arrays for payload/action planes.
pub struct BitsetArenaDeltas<'a> {
    swap_bits: &'a [u32],
    fold_bits: &'a [u32],
    value: &'a [u32],
    rewrite_action: &'a [u32],
}

impl<'a> BitsetArenaDeltas<'a> {
    /// Build a borrowed compressed-delta view.
    #[must_use]
    pub fn new(
        swap_bits: &'a [u32],
        fold_bits: &'a [u32],
        value: &'a [u32],
        rewrite_action: &'a [u32],
    ) -> Self {
        Self {
            swap_bits,
            fold_bits,
            value,
            rewrite_action,
        }
    }

    fn bit(words: &[u32], id: usize) -> u32 {
        words
            .get(id / 32)
            .map(|word| (word >> (id % 32)) & 1)
            .unwrap_or(0)
    }
}

impl ArenaDeltaLookup for BitsetArenaDeltas<'_> {
    fn swap_mask(&self, id: usize) -> u32 {
        Self::bit(self.swap_bits, id)
    }

    fn foldable(&self, id: usize) -> u32 {
        Self::bit(self.fold_bits, id)
    }

    fn value(&self, id: usize) -> u32 {
        self.value.get(id).copied().unwrap_or(0)
    }

    fn rewrite_action(&self, id: usize) -> u32 {
        self.rewrite_action.get(id).copied().unwrap_or(ra::NONE)
    }
}

impl ArenaDeltaLookup for DenseArenaDeltas<'_> {
    fn swap_mask(&self, id: usize) -> u32 {
        self.swap_mask.get(id).copied().unwrap_or(0)
    }

    fn foldable(&self, id: usize) -> u32 {
        self.foldable.get(id).copied().unwrap_or(0)
    }

    fn value(&self, id: usize) -> u32 {
        self.value.get(id).copied().unwrap_or(0)
    }

    fn rewrite_action(&self, id: usize) -> u32 {
        self.rewrite_action.get(id).copied().unwrap_or(ra::NONE)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ArenaDeltaRecord {
    swap_mask: u32,
    foldable: u32,
    value: u32,
    rewrite_action: u32,
}

/// Sparse arena deltas decoded from resident CUDA compaction.
#[derive(Debug, Clone, Default)]
pub struct SparseArenaDeltas {
    expr_count: u32,
    overrides: FxHashMap<u32, ArenaDeltaRecord>,
}

impl SparseArenaDeltas {
    /// Decode compacted records emitted by
    /// [`build_resident_delta_compact_program`].
    pub fn from_compacted_record_words(
        expr_count: u32,
        record_count: u32,
        record_words: &[u32],
        context: &str,
    ) -> Result<Self, super::dispatcher::DispatchError> {
        let count = record_count as usize;
        let expected_words = count.checked_mul(5).ok_or_else(|| {
            super::dispatcher::DispatchError::BadInputs(format!(
                "Fix: {context} compact arena record count overflows usize: {record_count}."
            ))
        })?;
        if record_words.len() != expected_words {
            return Err(super::dispatcher::DispatchError::BadInputs(format!(
                "Fix: {context} compact arena expected {expected_words} record word(s) for {record_count} record(s), got {}.",
                record_words.len()
            )));
        }

        let mut overrides = FxHashMap::default();
        overrides.try_reserve(count).map_err(|error| {
            super::dispatcher::DispatchError::BackendError(format!(
                "Fix: reserve {context} compact arena map for {count} record(s): {error}."
            ))
        })?;
        for record in record_words.chunks_exact(5) {
            let id = record[0];
            if id >= expr_count {
                return Err(super::dispatcher::DispatchError::BadInputs(format!(
                    "Fix: {context} compact arena record id {id} exceeds expr_count {expr_count}."
                )));
            }
            let delta = ArenaDeltaRecord {
                swap_mask: record[1],
                foldable: record[2],
                value: record[3],
                rewrite_action: record[4],
            };
            if delta.swap_mask == 0 && delta.foldable == 0 && delta.rewrite_action == ra::NONE {
                continue;
            }
            if overrides.insert(id, delta).is_some() {
                return Err(super::dispatcher::DispatchError::BadInputs(format!(
                    "Fix: {context} compact arena emitted duplicate expr id {id}."
                )));
            }
        }

        Ok(Self {
            expr_count,
            overrides,
        })
    }

    /// Number of non-identity arena delta records.
    #[must_use]
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }

    fn delta(&self, id: usize) -> Option<ArenaDeltaRecord> {
        let id_u32 = u32::try_from(id).ok()?;
        if id_u32 >= self.expr_count {
            return None;
        }
        self.overrides.get(&id_u32).copied()
    }
}

impl ArenaDeltaLookup for SparseArenaDeltas {
    fn swap_mask(&self, id: usize) -> u32 {
        self.delta(id).map(|delta| delta.swap_mask).unwrap_or(0)
    }

    fn foldable(&self, id: usize) -> u32 {
        self.delta(id).map(|delta| delta.foldable).unwrap_or(0)
    }

    fn value(&self, id: usize) -> u32 {
        self.delta(id).map(|delta| delta.value).unwrap_or(0)
    }

    fn rewrite_action(&self, id: usize) -> u32 {
        self.delta(id)
            .map(|delta| delta.rewrite_action)
            .unwrap_or(ra::NONE)
    }
}

/// Build the resident optimizer compaction Program.
///
/// Buffer layout:
///   0: swap_mask (RO)
///   1: foldable (RO)
///   2: value (RO)
///   3: rewrite_action (RO)
///   4: canonical (RO)
///   5: arena_delta_count (RW), word 0 = record count
///   6: arena_delta_records (RW), records are
///      `(expr_id, swap, foldable, value, rewrite_action)`
///   7: canonical_delta_count (RW), word 0 = pair count
///   8: canonical_delta_pairs (RW), records are
///      `(expr_id, canonical_id)`
#[must_use]
pub fn build_resident_delta_compact_program(expr_count: u32) -> Program {
    let arena_delta_words = expr_count.saturating_mul(5).max(1);
    let canonical_delta_words = expr_count.saturating_mul(2).max(1);
    let buffers = vec![
        BufferDecl::storage("swap_mask", 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("foldable", 1, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("value", 2, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("rewrite_action", 3, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("canonical", 4, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage(
            "arena_delta_count",
            5,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(1),
        BufferDecl::storage(
            "arena_delta_records",
            6,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(arena_delta_words),
        BufferDecl::storage(
            "canonical_delta_count",
            7,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(1),
        BufferDecl::storage(
            "canonical_delta_pairs",
            8,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(canonical_delta_words),
    ];
    let body = vec![
        Node::let_bind("i", Expr::gid_x()),
        Node::if_then(
            Expr::lt(Expr::var("i"), Expr::u32(expr_count)),
            vec![
                Node::let_bind("swap", Expr::load("swap_mask", Expr::var("i"))),
                Node::let_bind("fold", Expr::load("foldable", Expr::var("i"))),
                Node::let_bind("val", Expr::load("value", Expr::var("i"))),
                Node::let_bind("action", Expr::load("rewrite_action", Expr::var("i"))),
                Node::if_then(
                    Expr::or(
                        Expr::or(
                            Expr::ne(Expr::var("swap"), Expr::u32(0)),
                            Expr::ne(Expr::var("fold"), Expr::u32(0)),
                        ),
                        Expr::ne(Expr::var("action"), Expr::u32(ra::NONE)),
                    ),
                    vec![
                        Node::let_bind(
                            "arena_slot",
                            Expr::atomic_add("arena_delta_count", Expr::u32(0), Expr::u32(1)),
                        ),
                        Node::let_bind(
                            "arena_base",
                            Expr::mul(Expr::var("arena_slot"), Expr::u32(5)),
                        ),
                        Node::store(
                            "arena_delta_records",
                            Expr::var("arena_base"),
                            Expr::var("i"),
                        ),
                        Node::store(
                            "arena_delta_records",
                            Expr::add(Expr::var("arena_base"), Expr::u32(1)),
                            Expr::var("swap"),
                        ),
                        Node::store(
                            "arena_delta_records",
                            Expr::add(Expr::var("arena_base"), Expr::u32(2)),
                            Expr::var("fold"),
                        ),
                        Node::store(
                            "arena_delta_records",
                            Expr::add(Expr::var("arena_base"), Expr::u32(3)),
                            Expr::var("val"),
                        ),
                        Node::store(
                            "arena_delta_records",
                            Expr::add(Expr::var("arena_base"), Expr::u32(4)),
                            Expr::var("action"),
                        ),
                    ],
                ),
                Node::let_bind("canonical_id", Expr::load("canonical", Expr::var("i"))),
                Node::if_then(
                    Expr::ne(Expr::var("canonical_id"), Expr::var("i")),
                    vec![
                        Node::let_bind(
                            "canonical_slot",
                            Expr::atomic_add("canonical_delta_count", Expr::u32(0), Expr::u32(1)),
                        ),
                        Node::let_bind(
                            "canonical_base",
                            Expr::mul(Expr::var("canonical_slot"), Expr::u32(2)),
                        ),
                        Node::store(
                            "canonical_delta_pairs",
                            Expr::var("canonical_base"),
                            Expr::var("i"),
                        ),
                        Node::store(
                            "canonical_delta_pairs",
                            Expr::add(Expr::var("canonical_base"), Expr::u32(1)),
                            Expr::var("canonical_id"),
                        ),
                    ],
                ),
            ],
        ),
    ];

    Program::wrapped(buffers, [256, 1, 1], body)
}

/// Build a fixed-size resident delta packer.
///
/// This compresses the two boolean delta planes (`swap_mask` and
/// `foldable`) into u32 bitsets so the release path can read one
/// bounded range set with no extra host fence. Dense value/action and
/// canonical planes are read directly from their existing resident
/// buffers by the caller.
#[must_use]
pub fn build_resident_delta_bitset_pack_program(expr_count: u32) -> Program {
    let bit_words = expr_count.div_ceil(32).max(1);
    let buffers = vec![
        BufferDecl::storage("swap_mask", 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("foldable", 1, BufferAccess::ReadOnly, DataType::U32)
            .with_count(expr_count.max(1)),
        BufferDecl::storage("swap_bits", 2, BufferAccess::ReadWrite, DataType::U32)
            .with_count(bit_words),
        BufferDecl::storage("fold_bits", 3, BufferAccess::ReadWrite, DataType::U32)
            .with_count(bit_words),
    ];
    let body = vec![
        Node::let_bind("i", Expr::gid_x()),
        Node::if_then(
            Expr::lt(Expr::var("i"), Expr::u32(expr_count)),
            vec![
                Node::let_bind("word", Expr::div(Expr::var("i"), Expr::u32(32))),
                Node::let_bind(
                    "bit",
                    Expr::shl(Expr::u32(1), Expr::bitand(Expr::var("i"), Expr::u32(31))),
                ),
                Node::if_then(
                    Expr::ne(Expr::load("swap_mask", Expr::var("i")), Expr::u32(0)),
                    vec![Node::let_bind(
                        "swap_old",
                        Expr::atomic_or("swap_bits", Expr::var("word"), Expr::var("bit")),
                    )],
                ),
                Node::if_then(
                    Expr::ne(Expr::load("foldable", Expr::var("i")), Expr::u32(0)),
                    vec![Node::let_bind(
                        "fold_old",
                        Expr::atomic_or("fold_bits", Expr::var("word"), Expr::var("bit")),
                    )],
                ),
            ],
        ),
    ];

    Program::wrapped(buffers, [256, 1, 1], body)
}

/// Apply the combined per-Expr deltas to `program`, producing the
/// post-arena-pass Program. DCE is run separately on the result.
pub fn apply_combined_arena_deltas(
    program: &Program,
    swap_mask: &[u32],
    foldable: &[u32],
    value: &[u32],
    rewrite_action: &[u32],
) -> Program {
    let deltas = DenseArenaDeltas {
        swap_mask,
        foldable,
        value,
        rewrite_action,
    };
    apply_combined_arena_deltas_with_lookup(program, &deltas)
}

/// Sparse/dense-agnostic variant of [`apply_combined_arena_deltas`].
pub fn apply_combined_arena_deltas_with_lookup<D: ArenaDeltaLookup + ?Sized>(
    program: &Program,
    deltas: &D,
) -> Program {
    let body: Vec<Node> = match program.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    };

    let mut counter = 0u32;
    let rebuilt = rewrite_scope(&body, deltas, &mut counter);

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

/// Apply compressed bitset arena deltas.

pub fn apply_combined_arena_deltas_bitsets(
    program: &Program,
    swap_bits: &[u32],
    fold_bits: &[u32],
    value: &[u32],
    rewrite_action: &[u32],
) -> Program {
    let deltas = BitsetArenaDeltas::new(swap_bits, fold_bits, value, rewrite_action);
    apply_combined_arena_deltas_with_lookup(program, &deltas)
}

fn rewrite_scope<D: ArenaDeltaLookup + ?Sized>(
    body: &[Node],
    deltas: &D,
    counter: &mut u32,
) -> Vec<Node> {
    let prefix_len = super::encode::reachable_prefix_len(body);
    let mut out = Vec::with_capacity(prefix_len);
    for node in &body[..prefix_len] {
        out.push(rewrite_node(node, deltas, counter));
    }
    out
}

fn rewrite_node<D: ArenaDeltaLookup + ?Sized>(node: &Node, deltas: &D, counter: &mut u32) -> Node {
    match node {
        Node::Let { name, value: e } => {
            Node::let_bind(name.clone(), rewrite_expr(e, deltas, counter))
        }
        Node::Assign { name, value: e } => {
            Node::assign(name.clone(), rewrite_expr(e, deltas, counter))
        }
        Node::Store {
            buffer,
            index,
            value: e,
        } => Node::store(
            buffer.clone(),
            rewrite_expr(index, deltas, counter),
            rewrite_expr(e, deltas, counter),
        ),
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::if_then_else(
            rewrite_expr(cond, deltas, counter),
            rewrite_scope(then, deltas, counter),
            rewrite_scope(otherwise, deltas, counter),
        ),
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::loop_for(
            var.clone(),
            rewrite_expr(from, deltas, counter),
            rewrite_expr(to, deltas, counter),
            rewrite_scope(body, deltas, counter),
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
            offset: Box::new(rewrite_expr(offset, deltas, counter)),
            size: Box::new(rewrite_expr(size, deltas, counter)),
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
            offset: Box::new(rewrite_expr(offset, deltas, counter)),
            size: Box::new(rewrite_expr(size, deltas, counter)),
            tag: tag.clone(),
        },
        Node::Trap { address, tag } => Node::Trap {
            address: Box::new(rewrite_expr(address, deltas, counter)),
            tag: tag.clone(),
        },
        Node::Block(body) => Node::Block(rewrite_scope(body, deltas, counter)),
        Node::Region {
            generator,
            source_region,
            body,
        } => Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(rewrite_scope(body.as_slice(), deltas, counter)),
        },
        Node::Return
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => node.clone(),
        _ => node.clone(),
    }
}

fn rewrite_expr<D: ArenaDeltaLookup + ?Sized>(expr: &Expr, deltas: &D, counter: &mut u32) -> Expr {
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
            let id = *counter as usize;
            *counter += 1;
            decide_leaf(expr, id, deltas)
        }
        Expr::Load { buffer, index } => {
            let new_index = rewrite_expr(index, deltas, counter);
            *counter += 1;
            // Loads are not foldable / not pattern-matched / not
            // canonicalized.
            Expr::Load {
                buffer: buffer.clone(),
                index: Box::new(new_index),
            }
        }
        Expr::BinOp { op, left, right } => {
            let new_left = rewrite_expr(left, deltas, counter);
            let new_right = rewrite_expr(right, deltas, counter);
            let id = *counter as usize;
            *counter += 1;

            // Priority 1: const-fold. The kernel writes the folded
            // u32 result into `value[id]`. For comparison BinOps the
            // result is semantically Bool  -  emit LitBool so dead-
            // branch and downstream type-aware passes see the right
            // shape. For arithmetic BinOps emit LitU32.
            if deltas.foldable(id) == 1 {
                let raw = deltas.value(id);
                use vyre_foundation::ir::BinOp;
                let bool_result = matches!(
                    op,
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
                );
                if bool_result {
                    return Expr::LitBool(raw != 0);
                }
                return Expr::LitU32(raw);
            }
            // Priority 2: pattern-match rewrite.
            match deltas.rewrite_action(id) {
                ra::REPLACE_WITH_LEFT => return new_left,
                ra::REPLACE_WITH_RIGHT => return new_right,
                ra::REPLACE_WITH_LIT_ZERO => return Expr::LitU32(0),
                ra::REPLACE_WITH_LIT_TRUE => return Expr::LitBool(true),
                ra::REPLACE_WITH_LIT_FALSE => return Expr::LitBool(false),
                ra::REPLACE_WITH_LEFT_INNER_LEFT => {
                    if let Expr::BinOp { left: inner_l, .. } = &new_left {
                        return inner_l.as_ref().clone();
                    }
                }
                ra::REPLACE_WITH_LEFT_INNER_RIGHT => {
                    if let Expr::BinOp { right: inner_r, .. } = &new_left {
                        return inner_r.as_ref().clone();
                    }
                }
                _ => {}
            }
            // Priority 3: canonicalize swap.
            if deltas.swap_mask(id) == 1 {
                Expr::BinOp {
                    op: *op,
                    left: Box::new(new_right),
                    right: Box::new(new_left),
                }
            } else {
                Expr::BinOp {
                    op: *op,
                    left: Box::new(new_left),
                    right: Box::new(new_right),
                }
            }
        }
        Expr::UnOp { op, operand } => {
            let new_operand = rewrite_expr(operand, deltas, counter);
            let id = *counter as usize;
            *counter += 1;
            if deltas.foldable(id) == 1 {
                return Expr::LitU32(deltas.value(id));
            }
            // UnOp pattern-match: REPLACE_WITH_GRAND_OPERAND fires
            // for `~~x = x`, `--x = x`, `!!x = x`. The grand-child is
            // `new_operand`'s own operand; we descend one level.
            if deltas.rewrite_action(id) == ra::REPLACE_WITH_GRAND_OPERAND {
                if let Expr::UnOp { operand: inner, .. } = &new_operand {
                    return inner.as_ref().clone();
                }
            }
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
            let nc = rewrite_expr(cond, deltas, counter);
            let nt = rewrite_expr(true_val, deltas, counter);
            let nf = rewrite_expr(false_val, deltas, counter);
            *counter += 1;
            Expr::Select {
                cond: Box::new(nc),
                true_val: Box::new(nt),
                false_val: Box::new(nf),
            }
        }
        Expr::Fma { a, b, c } => {
            let na = rewrite_expr(a, deltas, counter);
            let nb = rewrite_expr(b, deltas, counter);
            let nc = rewrite_expr(c, deltas, counter);
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

fn decide_leaf<D: ArenaDeltaLookup + ?Sized>(expr: &Expr, id: usize, deltas: &D) -> Expr {
    if deltas.foldable(id) == 1 {
        match expr {
            Expr::LitU32(_) | Expr::LitI32(_) | Expr::LitF32(_) | Expr::LitBool(_) => expr.clone(),
            _ => Expr::LitU32(deltas.value(id)),
        }
    } else {
        expr.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resident_delta_compact_program_carries_arena_and_canonical_outputs() {
        let p = build_resident_delta_compact_program(8);
        assert!(p.buffers().iter().any(|b| b.name() == "arena_delta_count"));
        assert!(p
            .buffers()
            .iter()
            .any(|b| b.name() == "arena_delta_records"));
        assert!(p
            .buffers()
            .iter()
            .any(|b| b.name() == "canonical_delta_count"));
        assert!(p
            .buffers()
            .iter()
            .any(|b| b.name() == "canonical_delta_pairs"));
    }

    #[test]
    fn resident_delta_bitset_pack_program_carries_boolean_planes() {
        let p = build_resident_delta_bitset_pack_program(65);
        assert!(p.buffers().iter().any(|b| b.name() == "swap_bits"));
        assert!(p.buffers().iter().any(|b| b.name() == "fold_bits"));
    }

    #[test]
    fn bitset_arena_deltas_decode_boolean_planes() {
        let deltas = BitsetArenaDeltas::new(&[0b10, 0b1], &[0b100], &[0, 7, 9], &[0, 0, 3]);
        assert_eq!(deltas.swap_mask(1), 1);
        assert_eq!(deltas.swap_mask(32), 1);
        assert_eq!(deltas.swap_mask(2), 0);
        assert_eq!(deltas.foldable(2), 1);
        assert_eq!(deltas.value(2), 9);
        assert_eq!(deltas.rewrite_action(2), 3);
    }

    #[test]
    fn sparse_arena_deltas_default_identity_and_override_changed_records() {
        let deltas = SparseArenaDeltas::from_compacted_record_words(
            8,
            2,
            &[3, 1, 0, 0, ra::NONE, 5, 0, 1, 99, ra::REPLACE_WITH_LIT_ZERO],
            "test sparse arena",
        )
        .expect("Fix: valid compact arena records decode");
        assert_eq!(deltas.override_count(), 2);
        assert_eq!(deltas.swap_mask(0), 0);
        assert_eq!(deltas.swap_mask(3), 1);
        assert_eq!(deltas.foldable(5), 1);
        assert_eq!(deltas.value(5), 99);
        assert_eq!(deltas.rewrite_action(5), ra::REPLACE_WITH_LIT_ZERO);
    }

    #[test]
    fn sparse_arena_deltas_reject_malformed_record_count() {
        let err = SparseArenaDeltas::from_compacted_record_words(
            8,
            2,
            &[3, 1, 0, 0, ra::NONE],
            "test sparse arena",
        )
        .expect_err("compact arena record count must match record words exactly");
        assert!(
            matches!(err, super::super::dispatcher::DispatchError::BadInputs(_)),
            "unexpected error: {err:?}"
        );
    }
}
