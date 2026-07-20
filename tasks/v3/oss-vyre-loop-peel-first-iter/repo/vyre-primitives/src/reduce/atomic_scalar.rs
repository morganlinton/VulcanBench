use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

pub(crate) const WORKGROUP_SIZE: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicReduceKind {
    Sum,
    Min,
    Max,
    CountNonZero,
    PopcountSum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicBoolReduceKind {
    AnyNonZero,
    AllNonZero,
}

impl AtomicBoolReduceKind {
    fn identity(self) -> u32 {
        match self {
            Self::AnyNonZero => 0,
            Self::AllNonZero => 1,
        }
    }

    fn atomic(self, out: &str, value: Expr) -> Expr {
        match self {
            Self::AnyNonZero => Expr::atomic_or(out, Expr::u32(0), value),
            Self::AllNonZero => Expr::atomic_and(out, Expr::u32(0), value),
        }
    }
}

impl AtomicReduceKind {
    fn identity(self) -> u32 {
        match self {
            Self::Sum | Self::Max | Self::CountNonZero | Self::PopcountSum => 0,
            Self::Min => u32::MAX,
        }
    }

    fn value(self, input: &str, index: Expr) -> Expr {
        let loaded = Expr::load(input, index);
        match self {
            Self::CountNonZero => {
                Expr::select(Expr::ne(loaded, Expr::u32(0)), Expr::u32(1), Expr::u32(0))
            }
            Self::PopcountSum => Expr::UnOp {
                op: UnOp::Popcount,
                operand: Box::new(loaded),
            },
            Self::Sum | Self::Min | Self::Max => loaded,
        }
    }

    fn atomic(self, out: &str, value: Expr) -> Expr {
        match self {
            Self::Sum | Self::CountNonZero | Self::PopcountSum => {
                Expr::atomic_add(out, Expr::u32(0), value)
            }
            Self::Min => Expr::atomic_min(out, Expr::u32(0), value),
            Self::Max => Expr::atomic_max(out, Expr::u32(0), value),
        }
    }
}

pub(crate) fn atomic_reduce_u32(
    input: &str,
    out: &str,
    count: u32,
    kind: AtomicReduceKind,
    op_id: &'static str,
) -> Program {
    atomic_grid_stride_u32(
        input,
        out,
        count,
        kind.identity(),
        |input, index| kind.value(input, index),
        |out, value| kind.atomic(out, value),
        op_id,
    )
}

pub(crate) fn atomic_nonzero_bool_reduce_u32(
    input: &str,
    out: &str,
    count: u32,
    kind: AtomicBoolReduceKind,
    op_id: &'static str,
) -> Program {
    atomic_grid_stride_u32(
        input,
        out,
        count,
        kind.identity(),
        |input, index| {
            Expr::select(
                Expr::ne(Expr::load(input, index), Expr::u32(0)),
                Expr::u32(1),
                Expr::u32(0),
            )
        },
        |out, value| kind.atomic(out, value),
        op_id,
    )
}

#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn cpu_ref_nonzero_bool_reduce(values: &[u32], kind: AtomicBoolReduceKind) -> u32 {
    let matched = match kind {
        AtomicBoolReduceKind::AnyNonZero => values.iter().any(|&value| value != 0),
        AtomicBoolReduceKind::AllNonZero => values.iter().all(|&value| value != 0),
    };
    u32::from(matched)
}

macro_rules! define_bool_reduce_op {
    (
        op_id: $op_id:expr,
        fn_name: $fn_name:ident,
        kind: $kind:ident,
        true_case: $true_case:expr,
        false_case: $false_case:expr,
        inventory_expected: $inventory_expected:expr
    ) => {
        /// Canonical op id.
        pub const OP_ID: &str = $op_id;

        /// Build a non-zero boolean reduction Program over a u32 ValueSet.
        #[must_use]
        pub fn $fn_name(values: &str, out: &str, count: u32) -> vyre_foundation::ir::Program {
            crate::reduce::atomic_scalar::atomic_nonzero_bool_reduce_u32(
                values,
                out,
                count,
                crate::reduce::atomic_scalar::AtomicBoolReduceKind::$kind,
                OP_ID,
            )
        }

        /// CPU reference.
        #[must_use]
        #[cfg(any(test, feature = "cpu-parity"))]
        pub fn cpu_ref(values: &[u32]) -> u32 {
            crate::reduce::atomic_scalar::cpu_ref_nonzero_bool_reduce(
                values,
                crate::reduce::atomic_scalar::AtomicBoolReduceKind::$kind,
            )
        }

        #[cfg(feature = "inventory-registry")]
        inventory::submit! {
            crate::harness::OpEntry::new(
                OP_ID,
                || $fn_name("values", "out", 4),
                Some(|| {
                    let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
                    vec![vec![
                        to_bytes(&[1, 0, 1, 1]),
                        to_bytes(&[0]),
                    ]]
                }),
                Some(|| {
                    let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
                    vec![vec![to_bytes(&$inventory_expected)]]
                }),
            )
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn true_case_reduces_to_one() {
                assert_eq!(cpu_ref(&$true_case), 1);
            }

            #[test]
            fn false_case_reduces_to_zero() {
                assert_eq!(cpu_ref(&$false_case), 0);
            }

            #[test]
            fn program_uses_parallel_grid_stride() {
                let program = $fn_name("values", "out", 513);
                assert_eq!(
                    program.workgroup_size(),
                    [crate::reduce::atomic_scalar::WORKGROUP_SIZE, 1, 1]
                );
            }
        }
    };
}

pub(crate) use define_bool_reduce_op;

macro_rules! define_u32_reduce_op {
    (
        op_id: $op_id:expr,
        fn_name: $fn_name:ident,
        kind: $kind:ident,
        identity: $identity:expr,
        fold: $fold:expr,
        sample: $sample:expr,
        expected: $expected:expr
    ) => {
        /// Canonical op id.
        pub const OP_ID: &str = $op_id;

        /// Build an atomic grid-stride u32 reduction Program.
        #[must_use]
        pub fn $fn_name(values: &str, out: &str, count: u32) -> vyre_foundation::ir::Program {
            crate::reduce::atomic_scalar::atomic_reduce_u32(
                values,
                out,
                count,
                crate::reduce::atomic_scalar::AtomicReduceKind::$kind,
                OP_ID,
            )
        }

        /// CPU reference.
        #[must_use]
        #[cfg(any(test, feature = "cpu-parity"))]
        pub fn cpu_ref(values: &[u32]) -> u32 {
            let fold = $fold;
            values.iter().copied().fold($identity, fold)
        }

        #[cfg(feature = "inventory-registry")]
        inventory::submit! {
            crate::harness::OpEntry::new(
                OP_ID,
                || $fn_name("values", "out", 4),
                Some(|| {
                    let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
                    vec![vec![
                        to_bytes(&$sample),
                        to_bytes(&[0]),
                    ]]
                }),
                Some(|| {
                    let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
                    vec![vec![to_bytes(&[$expected])]]
                }),
            )
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn sample_matches_cpu_reference() {
                assert_eq!(cpu_ref(&$sample), $expected);
            }

            #[test]
            fn empty_returns_identity() {
                assert_eq!(cpu_ref(&[]), $identity);
            }

            #[test]
            fn singleton_returns_value_or_identity_fold() {
                assert_eq!(cpu_ref(&[$expected]), $expected);
            }

            #[test]
            fn program_uses_parallel_grid_stride() {
                let program = $fn_name("values", "out", 513);
                assert_eq!(
                    program.workgroup_size(),
                    [crate::reduce::atomic_scalar::WORKGROUP_SIZE, 1, 1]
                );
            }
        }
    };
}

pub(crate) use define_u32_reduce_op;

fn atomic_grid_stride_u32<V, A>(
    input: &str,
    out: &str,
    count: u32,
    identity: u32,
    value: V,
    atomic: A,
    op_id: &'static str,
) -> Program
where
    V: Fn(&str, Expr) -> Expr,
    A: Fn(&str, Expr) -> Expr,
{
    let lane = Expr::InvocationId { axis: 0 };
    let chunk_count = Expr::div(
        Expr::add(Expr::u32(count), Expr::u32(WORKGROUP_SIZE - 1)),
        Expr::u32(WORKGROUP_SIZE),
    );

    let body = vec![
        Node::if_then(
            Expr::eq(lane.clone(), Expr::u32(0)),
            vec![Node::store(out, Expr::u32(0), Expr::u32(identity))],
        ),
        Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        },
        Node::loop_for(
            "chunk",
            Expr::u32(0),
            chunk_count,
            vec![
                Node::let_bind(
                    "i",
                    Expr::add(
                        Expr::mul(Expr::var("chunk"), Expr::u32(WORKGROUP_SIZE)),
                        lane.clone(),
                    ),
                ),
                Node::if_then(
                    Expr::lt(Expr::var("i"), Expr::u32(count)),
                    vec![Node::let_bind(
                        "_acc_prev",
                        atomic(out, value(input, Expr::var("i"))),
                    )],
                ),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(out, 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [WORKGROUP_SIZE, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}
