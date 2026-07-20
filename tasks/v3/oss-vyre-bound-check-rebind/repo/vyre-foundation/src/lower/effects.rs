//! Effects-typed lower pipeline (P-1.0-V1.3).
//!
//! [`compute_program_effects`] walks a `Program` and computes the
//! [`ProgramEffects`] row  -  the union of every effect kind any
//! Region in the program produces. The lowering pipeline can route
//! handler discharges (P-1.0-V1.1, P-1.0-V1.2) against the row to
//! prove that a backend's emitted code respects the declared effect
//! discipline.
//!
//! The kinds mirror `vyre_primitives::effects::EffectKind` so a
//! downstream crate can convert this row into the primitives'
//! `EffectRow`. The duplication is deliberate: foundation cannot
//! depend on primitives (cycle), and a row is cheap.

use crate::ir_inner::model::expr::Expr;
use crate::ir_inner::model::node::Node;
use crate::ir_inner::model::program::Program;

/// Set of effect kinds a `Program` produces. Matches the canonical
/// `EffectKind` ordering shipped from `vyre_primitives::effects`.
/// Each backend lowering pass may require / discharge / forbid
/// specific kinds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ProgramEffects(u32);

impl ProgramEffects {
    /// Empty row.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }
    /// Buffer write  -  `Node::Store`, `Node::AsyncStore`.
    pub const BUFFER_WRITE: Self = Self(1 << 0);
    /// Atomic read-modify-write  -  `Expr::Atomic`.
    pub const ATOMIC: Self = Self(1 << 1);
    /// Host-visible I/O effect used by host-bridge extensions.
    pub const HOST_IO: Self = Self(1 << 2);
    /// Nested GPU dispatch  -  `Node::IndirectDispatch`.
    pub const GPU_DISPATCH: Self = Self(1 << 3);
    /// Synchronization  -  `Node::Barrier { ordering: vyre::memory_model::MemoryOrdering::SeqCst }`.
    pub const BARRIER: Self = Self(1 << 4);
    /// Async load fetching from streaming storage  -
    /// `Node::AsyncLoad`.
    pub const ASYNC_LOAD: Self = Self(1 << 5);
    /// Trap or abort  -  `Node::Trap`.
    pub const TRAP: Self = Self(1 << 6);

    /// Whether this row contains every bit set in `other`.
    #[must_use]
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Whether this row has no effects.
    #[must_use]
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether every effect in `self` is also present in `other`.
    #[must_use]
    #[inline]
    pub const fn is_subset_of(self, other: Self) -> bool {
        (self.0 & !other.0) == 0
    }

    /// Effects present in `self` but absent from `previous`.
    #[must_use]
    #[inline]
    pub const fn introduced_since(self, previous: Self) -> Self {
        Self(self.0 & !previous.0)
    }

    /// Raw bitmask.
    #[must_use]
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl core::ops::BitOr for ProgramEffects {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for ProgramEffects {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Compute the union of every effect kind reachable from
/// `program.entry()`.
#[must_use]
pub fn compute_program_effects(program: &Program) -> ProgramEffects {
    let mut effects = ProgramEffects::empty();
    for node in program.entry() {
        walk_node(node, &mut effects);
    }
    effects
}

fn walk_node(node: &Node, effects: &mut ProgramEffects) {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => walk_expr(value, effects),
        Node::Store { index, value, .. } => {
            *effects |= ProgramEffects::BUFFER_WRITE;
            walk_expr(index, effects);
            walk_expr(value, effects);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            walk_expr(cond, effects);
            for n in then {
                walk_node(n, effects);
            }
            for n in otherwise {
                walk_node(n, effects);
            }
        }
        Node::Loop { from, to, body, .. } => {
            walk_expr(from, effects);
            walk_expr(to, effects);
            for n in body {
                walk_node(n, effects);
            }
        }
        Node::IndirectDispatch { .. } => {
            *effects |= ProgramEffects::GPU_DISPATCH;
        }
        Node::AsyncLoad { offset, size, .. } => {
            *effects |= ProgramEffects::ASYNC_LOAD;
            walk_expr(offset, effects);
            walk_expr(size, effects);
        }
        Node::AsyncStore { offset, size, .. } => {
            *effects |= ProgramEffects::BUFFER_WRITE;
            walk_expr(offset, effects);
            walk_expr(size, effects);
        }
        Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. } => {
            *effects |= ProgramEffects::BUFFER_WRITE;
            *effects |= ProgramEffects::BARRIER;
        }
        Node::AsyncWait { .. } | Node::Resume { .. } | Node::Return | Node::Opaque(_) => {}
        Node::Trap { address, .. } => {
            *effects |= ProgramEffects::TRAP;
            walk_expr(address, effects);
        }
        Node::Barrier { .. } => {
            *effects |= ProgramEffects::BARRIER;
        }
        Node::Block(nodes) => {
            for n in nodes {
                walk_node(n, effects);
            }
        }
        Node::Region { body, .. } => {
            for n in body.iter() {
                walk_node(n, effects);
            }
        }
    }
}

fn walk_expr(expr: &Expr, effects: &mut ProgramEffects) {
    match expr {
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::BufLen { .. }
        | Expr::Opaque(_) => {}
        Expr::Load { index, .. } => walk_expr(index, effects),
        Expr::BinOp { left, right, .. } => {
            walk_expr(left, effects);
            walk_expr(right, effects);
        }
        Expr::UnOp { operand, .. } => walk_expr(operand, effects),
        Expr::Call { args, .. } => {
            for a in args {
                walk_expr(a, effects);
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            walk_expr(cond, effects);
            walk_expr(true_val, effects);
            walk_expr(false_val, effects);
        }
        Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => walk_expr(value, effects),
        Expr::Fma { a, b, c } => {
            walk_expr(a, effects);
            walk_expr(b, effects);
            walk_expr(c, effects);
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            *effects |= ProgramEffects::ATOMIC;
            walk_expr(index, effects);
            if let Some(expected) = expected {
                walk_expr(expected, effects);
            }
            walk_expr(value, effects);
        }
        Expr::SubgroupBallot { cond } => walk_expr(cond, effects),
        Expr::SubgroupShuffle { value, lane } => {
            walk_expr(value, effects);
            walk_expr(lane, effects);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr as IrExpr, Node as IrNode, Program};

    fn program_with(body: Vec<IrNode>, buffers: Vec<BufferDecl>) -> Program {
        Program::wrapped(buffers, [1, 1, 1], body)
    }

    #[test]
    fn empty_program_has_no_effects() {
        let prog = program_with(vec![IrNode::Return], vec![]);
        assert_eq!(compute_program_effects(&prog), ProgramEffects::empty());
    }

    #[test]
    fn store_records_buffer_write() {
        let prog = program_with(
            vec![IrNode::store("out", IrExpr::u32(0), IrExpr::u32(7))],
            vec![
                BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            ],
        );
        let e = compute_program_effects(&prog);
        assert!(e.contains(ProgramEffects::BUFFER_WRITE));
        assert!(!e.contains(ProgramEffects::ATOMIC));
        assert!(!e.contains(ProgramEffects::BARRIER));
    }

    #[test]
    fn barrier_records_barrier() {
        let prog = program_with(vec![IrNode::barrier(), IrNode::Return], vec![]);
        let e = compute_program_effects(&prog);
        assert!(e.contains(ProgramEffects::BARRIER));
    }

    #[test]
    fn atomic_records_atomic() {
        let prog = program_with(
            vec![IrNode::store(
                "out",
                IrExpr::u32(0),
                IrExpr::atomic_add("out", IrExpr::u32(0), IrExpr::u32(1)),
            )],
            vec![
                BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            ],
        );
        let e = compute_program_effects(&prog);
        assert!(e.contains(ProgramEffects::ATOMIC));
        assert!(e.contains(ProgramEffects::BUFFER_WRITE));
    }

    #[test]
    fn nested_in_if_branches_collects_effects() {
        let prog = program_with(
            vec![IrNode::If {
                cond: IrExpr::bool(true),
                then: vec![IrNode::barrier()],
                otherwise: vec![IrNode::store("o", IrExpr::u32(0), IrExpr::u32(1))],
            }],
            vec![BufferDecl::storage("o", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        );
        let e = compute_program_effects(&prog);
        assert!(e.contains(ProgramEffects::BARRIER));
        assert!(e.contains(ProgramEffects::BUFFER_WRITE));
    }

    #[test]
    fn pure_arithmetic_program_has_no_effects() {
        // Var + Lit binop with no Store still has zero effects.
        let prog = program_with(
            vec![IrNode::let_bind("x", IrExpr::u32(7)), IrNode::Return],
            vec![],
        );
        let e = compute_program_effects(&prog);
        assert_eq!(e, ProgramEffects::empty());
    }

    #[test]
    fn region_traversal_descends_into_body() {
        let prog = program_with(
            vec![IrNode::Region {
                generator: "test.r".into(),
                source_region: None,
                body: std::sync::Arc::new(vec![IrNode::barrier()]),
            }],
            vec![],
        );
        assert!(compute_program_effects(&prog).contains(ProgramEffects::BARRIER));
    }

    #[test]
    fn effects_form_a_stable_set() {
        // Order of nodes does not change the row.
        let buffer =
            BufferDecl::storage("o", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1);
        let p1 = program_with(
            vec![
                IrNode::barrier(),
                IrNode::store("o", IrExpr::u32(0), IrExpr::u32(1)),
            ],
            vec![buffer.clone()],
        );
        let p2 = program_with(
            vec![
                IrNode::store("o", IrExpr::u32(0), IrExpr::u32(1)),
                IrNode::barrier(),
            ],
            vec![buffer],
        );
        assert_eq!(compute_program_effects(&p1), compute_program_effects(&p2));
    }

    #[test]
    fn introduced_since_reports_only_new_effects() {
        let before = ProgramEffects::BUFFER_WRITE | ProgramEffects::ATOMIC;
        let after = before | ProgramEffects::BARRIER;
        let introduced = after.introduced_since(before);

        assert!(introduced.contains(ProgramEffects::BARRIER));
        assert!(!introduced.contains(ProgramEffects::BUFFER_WRITE));
        assert!(introduced.is_subset_of(ProgramEffects::BARRIER));
        assert!(ProgramEffects::empty().is_empty());
    }
}
