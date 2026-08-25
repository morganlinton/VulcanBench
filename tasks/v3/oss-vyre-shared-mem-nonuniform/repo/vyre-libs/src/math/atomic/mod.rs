//! Cat-B atomic-read-modify-write compositions over a 1-slot state buffer.
//!
//! These builders live in `vyre-libs` because they are still compositions over
//! `Expr::Atomic`, but they are NOT Category A: correctness depends on the
//! backend owning the matching target builder atomic emission path. If a backend cannot
//! lower the atomic op, dispatch must fail loudly instead of silently treating
//! these as pure library sugar.
//!
//! Each op emits a single-invocation serial walk. For every i in
//! 0..n: write the pre-op state into `trace[i]`, apply the atomic op
//! to `state[0]`. The serial walk gives a byte-identical CPU reference
//! that matches `wrapping_{add,and,or,xor}`, `min`, `max`, `exchange`,
//! or `compare_exchange` semantics under single-lane contention.

use vyre::ir::{AtomicOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre::memory_model::MemoryOrdering;

// --- Macros must be defined before `pub mod` declarations so child modules
// can name them. `macro_rules!` is textual and scoped to what appears
// lexically earlier in the file; submodules declared above the macros
// cannot see them and fail to compile with "cannot find macro …" errors.
// F-IR-35 reclassified atomics to Category::Intrinsic through these.

/// Helper macro to register a Cat-B atomic serial op in the dialect registry.
/// Every atomic op that emits `Expr::Atomic` must carry `Category::Intrinsic`
/// so the validator knows the backend must own the corresponding target builder arm.
macro_rules! register_atomic_serial_op {
    ($op_id:expr, $compose:expr) => {
        ::inventory::submit! {
            ::vyre_driver::registry::dialect::OpDefRegistration::new(|| ::vyre_driver::registry::OpDef {
                id: $op_id,
                dialect: "vyre-libs.math.atomic",
                category: ::vyre_driver::registry::Category::Intrinsic,
                signature: ::vyre_driver::registry::Signature {
                    inputs: &[
                        ::vyre_driver::registry::TypedParam { name: "values", ty: "buffer<u32>" },
                        ::vyre_driver::registry::TypedParam { name: "state", ty: "buffer<u32>" },
                        ::vyre_driver::registry::TypedParam { name: "trace", ty: "buffer<u32>" },
                    ],
                    outputs: &[],
                    attrs: &[],
                    bytes_extraction: false,
                },
                lowerings: ::vyre_foundation::dialect_lookup::LoweringTable::empty(),
                laws: &[],
                compose: Some($compose),
            })
        }
    };
}

/// Helper macro for `atomic_compare_exchange_u32` which has a different
/// input schema (`expected` + `desired` buffers).
macro_rules! register_atomic_cas_op {
    ($op_id:expr, $compose:expr) => {
        ::inventory::submit! {
            ::vyre_driver::registry::dialect::OpDefRegistration::new(|| ::vyre_driver::registry::OpDef {
                id: $op_id,
                dialect: "vyre-libs.math.atomic",
                category: ::vyre_driver::registry::Category::Intrinsic,
                signature: ::vyre_driver::registry::Signature {
                    inputs: &[
                        ::vyre_driver::registry::TypedParam { name: "expected", ty: "buffer<u32>" },
                        ::vyre_driver::registry::TypedParam { name: "desired", ty: "buffer<u32>" },
                        ::vyre_driver::registry::TypedParam { name: "state", ty: "buffer<u32>" },
                        ::vyre_driver::registry::TypedParam { name: "trace", ty: "buffer<u32>" },
                    ],
                    outputs: &[],
                    attrs: &[],
                    bytes_extraction: false,
                },
                lowerings: ::vyre_foundation::dialect_lookup::LoweringTable::empty(),
                laws: &[],
                compose: Some($compose),
            })
        }
    };
}

macro_rules! define_atomic_serial_module {
    (
        $fn_name:ident,
        $op_id:literal,
        $atomic_op:ident,
        $oracle:ident,
        $values:expr,
        $initial:expr,
        $final_state:expr,
        $trace:expr
    ) => {
        use vyre::ir::Program;

        const OP_ID: &str = $op_id;

        /// Sequential atomic operation over `values[0..n]` into one-slot `state`.
        #[must_use]
        pub fn $fn_name(values: &str, state: &str, trace: &str, n: u32) -> Program {
            super::build_atomic_serial(
                OP_ID,
                vyre::ir::AtomicOp::$atomic_op,
                values,
                state,
                trace,
                n,
            )
        }

        inventory::submit! {
            crate::harness::OpEntry {
                id: OP_ID,
                build: || $fn_name("values", "state", "trace", 4),
                test_inputs: Some(|| {
                    let to_bytes = vyre_primitives::wire::pack_u32_slice;
                    let values: &[u32] = &$values;
                    vec![vec![to_bytes(values), to_bytes(&[$initial])]]
                }),
                expected_output: Some(|| {
                    let to_bytes = vyre_primitives::wire::pack_u32_slice;
                    let trace: &[u32] = &$trace;
                    vec![vec![to_bytes(&[$final_state]), to_bytes(trace)]]
                }),
                category: Some("math"),
            }
        }

        register_atomic_serial_op!(OP_ID, || $fn_name("values", "state", "trace", 4));

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::math::atomic::testutil::{assert_serial_matches, SerialAtomicOracle};

            #[test]
            fn fixture_matches_serial_oracle() {
                let values: &[u32] = &$values;
                let program = $fn_name("values", "state", "trace", values.len() as u32);
                assert_serial_matches(&program, SerialAtomicOracle::$oracle, values, $initial);
            }

            #[test]
            fn single_value_matches_serial_oracle() {
                let values: &[u32] = &$values;
                let single = [values[0]];
                let program = $fn_name("values", "state", "trace", 1);
                assert_serial_matches(&program, SerialAtomicOracle::$oracle, &single, $initial);
            }
        }
    };
}

/// Cat-B atomic-add composition.
pub mod atomic_add {
    define_atomic_serial_module!(
        atomic_add_u32,
        "vyre-libs::math::atomic::atomic_add_u32",
        Add,
        Add,
        [1u32, 5, u32::MAX, 3],
        7u32,
        15u32,
        [7u32, 8, 13, 12]
    );
}

/// Cat-B atomic-AND composition.
pub mod atomic_and {
    define_atomic_serial_module!(
        atomic_and_u32,
        "vyre-libs::math::atomic::atomic_and_u32",
        And,
        And,
        [0xFFu32, 0xF0, 0x0F, 0x33],
        u32::MAX,
        0x00u32,
        [u32::MAX, 0xFF, 0xF0, 0x00]
    );
}

pub mod atomic_compare_exchange;
/// Cat-B atomic-exchange composition.
pub mod atomic_exchange {
    define_atomic_serial_module!(
        atomic_exchange_u32,
        "vyre-libs::math::atomic::atomic_exchange_u32",
        Exchange,
        Exchange,
        [100u32, 200, 300, 400],
        42u32,
        400u32,
        [42u32, 100, 200, 300]
    );
}

pub mod atomic_lru_update;
/// Cat-B atomic-max composition.
pub mod atomic_max {
    define_atomic_serial_module!(
        atomic_max_u32,
        "vyre-libs::math::atomic::atomic_max_u32",
        Max,
        Max,
        [50u32, 20, 80, 10],
        0u32,
        80u32,
        [0u32, 50, 50, 80]
    );
}

/// Cat-B atomic-min composition.
pub mod atomic_min {
    define_atomic_serial_module!(
        atomic_min_u32,
        "vyre-libs::math::atomic::atomic_min_u32",
        Min,
        Min,
        [50u32, 20, 80, 10],
        100u32,
        10u32,
        [100u32, 50, 20, 20]
    );
}

/// Cat-B atomic-OR composition.
pub mod atomic_or {
    define_atomic_serial_module!(
        atomic_or_u32,
        "vyre-libs::math::atomic::atomic_or_u32",
        Or,
        Or,
        [0x01u32, 0x02, 0x04, 0x08],
        0u32,
        0x0Fu32,
        [0u32, 1, 3, 7]
    );
}

/// Cat-B atomic-XOR composition.
pub mod atomic_xor {
    define_atomic_serial_module!(
        atomic_xor_u32,
        "vyre-libs::math::atomic::atomic_xor_u32",
        Xor,
        Xor,
        [0xF0u32, 0x0F, 0xFF, 0x55],
        0u32,
        0x55u32,
        [0u32, 0xF0, 0xFF, 0x00]
    );
}

pub use atomic_add::atomic_add_u32;
pub use atomic_and::atomic_and_u32;
pub use atomic_compare_exchange::atomic_compare_exchange_u32;
pub use atomic_exchange::atomic_exchange_u32;
pub use atomic_lru_update::atomic_lru_update_u32;
pub use atomic_max::atomic_max_u32;
pub use atomic_min::atomic_min_u32;
pub use atomic_or::atomic_or_u32;
pub use atomic_xor::atomic_xor_u32;

/// Shared builder for the 7 single-value atomic variants
/// (add/and/or/xor/min/max/exchange). Constructs:
///
/// ```text
/// if idx == 0 {
///   for i in 0..buf_len(values) {
///     trace[i] = Atomic { op: <op>, buffer: state, index: 0, value: values[i] };
///   }
/// }
/// ```
///
/// Wrapped in `Node::Region` with `op_id` per the Region chain
/// invariant.
pub(crate) fn build_atomic_serial(
    op_id: &'static str,
    op: AtomicOp,
    values: &str,
    state: &str,
    trace: &str,
    n: u32,
) -> Program {
    let body = vec![crate::region::wrap_anonymous(
        op_id,
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::buf_len(values),
                vec![
                    Node::let_bind(
                        "old",
                        Expr::Atomic {
                            op,
                            buffer: state.into(),
                            index: Box::new(Expr::u32(0)),
                            expected: None,
                            value: Box::new(Expr::load(values, Expr::var("i"))),
                            ordering: MemoryOrdering::SeqCst,
                        },
                    ),
                    Node::store(trace, Expr::var("i"), Expr::var("old")),
                ],
            )],
        )],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(values, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::read_write(state, 1, DataType::U32).with_count(1),
            BufferDecl::output(trace, 2, DataType::U32).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

/// Shared builder for `atomic_compare_exchange_u32`. Walks two input
/// buffers (expected[i], desired[i]) against a 1-slot state.
pub(crate) fn build_atomic_compare_exchange(
    op_id: &'static str,
    expected: &str,
    desired: &str,
    state: &str,
    trace: &str,
    n: u32,
) -> Program {
    let body = vec![crate::region::wrap_anonymous(
        op_id,
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::buf_len(expected),
                vec![
                    Node::let_bind(
                        "old",
                        Expr::Atomic {
                            op: AtomicOp::CompareExchange,
                            buffer: state.into(),
                            index: Box::new(Expr::u32(0)),
                            expected: Some(Box::new(Expr::load(expected, Expr::var("i")))),
                            value: Box::new(Expr::load(desired, Expr::var("i"))),
                            ordering: MemoryOrdering::SeqCst,
                        },
                    ),
                    Node::store(trace, Expr::var("i"), Expr::var("old")),
                ],
            )],
        )],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(expected, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(desired, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::read_write(state, 2, DataType::U32).with_count(1),
            BufferDecl::output(trace, 3, DataType::U32).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

// Test helpers shared across atomic op unit tests.
#[cfg(test)]
pub(crate) mod testutil {
    use vyre_reference::value::Value;

    pub(crate) use crate::scan::dispatch_io::pack_u32_slice as pack_u32;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum SerialAtomicOracle {
        Add,
        And,
        Exchange,
        Max,
        Min,
        Or,
        Xor,
    }

    impl SerialAtomicOracle {
        fn apply(self, state: u32, value: u32) -> u32 {
            match self {
                Self::Add => state.wrapping_add(value),
                Self::And => state & value,
                Self::Exchange => value,
                Self::Max => state.max(value),
                Self::Min => state.min(value),
                Self::Or => state | value,
                Self::Xor => state ^ value,
            }
        }
    }

    pub(crate) fn cpu_serial(
        kind: SerialAtomicOracle,
        values: &[u32],
        initial_state: u32,
    ) -> (u32, Vec<u32>) {
        let mut state = initial_state;
        let mut trace = Vec::with_capacity(values.len());
        for &value in values {
            trace.push(state);
            state = kind.apply(state, value);
        }
        (state, trace)
    }

    pub(crate) fn assert_serial_matches(
        program: &vyre::ir::Program,
        kind: SerialAtomicOracle,
        values: &[u32],
        initial_state: u32,
    ) {
        let gpu_like = run_serial(program, values, initial_state);
        let expected = cpu_serial(kind, values, initial_state);
        assert_eq!(gpu_like, expected);
    }

    pub(crate) fn run_serial(
        program: &vyre::ir::Program,
        values: &[u32],
        initial_state: u32,
    ) -> (u32, Vec<u32>) {
        let n = values.len().max(1);
        let inputs = vec![
            Value::Bytes(pack_u32(values).into()),
            Value::Bytes(pack_u32(&[initial_state]).into()),
            Value::Bytes(vec![0u8; n * 4].into()),
        ];
        let outputs = vyre_reference::reference_eval(program, &inputs)
            .expect("Fix: atomic op must run; restore this invariant before continuing.");
        let state_bytes = outputs[0].to_bytes();
        let state = vyre_primitives::wire::read_u32_le_word(&state_bytes, 0, "atomic state")
            .expect("Fix: atomic state output must contain one u32.");
        let trace_bytes = outputs[1].to_bytes();
        let trace = vyre_primitives::wire::decode_u32_le_bytes_all(&trace_bytes);
        (state, trace)
    }

    pub(crate) fn run_cas(
        program: &vyre::ir::Program,
        expected: &[u32],
        desired: &[u32],
        initial_state: u32,
    ) -> (u32, Vec<u32>) {
        let n = expected.len().max(1);
        let inputs = vec![
            Value::Bytes(pack_u32(expected).into()),
            Value::Bytes(pack_u32(desired).into()),
            Value::Bytes(pack_u32(&[initial_state]).into()),
            Value::Bytes(vec![0u8; n * 4].into()),
        ];
        let outputs = vyre_reference::reference_eval(program, &inputs)
            .expect("Fix: cas op must run; restore this invariant before continuing.");
        let state_bytes = outputs[0].to_bytes();
        let state = vyre_primitives::wire::read_u32_le_word(&state_bytes, 0, "cas state")
            .expect("Fix: CAS state output must contain one u32.");
        let trace_bytes = outputs[1].to_bytes();
        let trace = vyre_primitives::wire::decode_u32_le_bytes_all(&trace_bytes);
        (state, trace)
    }
}

#[cfg(test)]

mod tests {
    use super::*;
    use testutil::{assert_serial_matches, SerialAtomicOracle};

    struct GeneratedSerialCase {
        name: &'static str,
        kind: SerialAtomicOracle,
        build: fn(&str, &str, &str, u32) -> Program,
        seed: u32,
    }

    #[test]
    fn generated_atomic_serial_family_matches_cpu_oracle() {
        let cases = [
            GeneratedSerialCase {
                name: "add",
                kind: SerialAtomicOracle::Add,
                build: atomic_add_u32,
                seed: 0xA11C_EE01,
            },
            GeneratedSerialCase {
                name: "and",
                kind: SerialAtomicOracle::And,
                build: atomic_and_u32,
                seed: 0xA11C_EE02,
            },
            GeneratedSerialCase {
                name: "exchange",
                kind: SerialAtomicOracle::Exchange,
                build: atomic_exchange_u32,
                seed: 0xA11C_EE03,
            },
            GeneratedSerialCase {
                name: "max",
                kind: SerialAtomicOracle::Max,
                build: atomic_max_u32,
                seed: 0xA11C_EE04,
            },
            GeneratedSerialCase {
                name: "min",
                kind: SerialAtomicOracle::Min,
                build: atomic_min_u32,
                seed: 0xA11C_EE05,
            },
            GeneratedSerialCase {
                name: "or",
                kind: SerialAtomicOracle::Or,
                build: atomic_or_u32,
                seed: 0xA11C_EE06,
            },
            GeneratedSerialCase {
                name: "xor",
                kind: SerialAtomicOracle::Xor,
                build: atomic_xor_u32,
                seed: 0xA11C_EE07,
            },
        ];

        for case in cases {
            let mut state = case.seed;
            for iteration in 0..512_u32 {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let len = ((state >> 27) as usize % 9) + 1;
                let initial = state.rotate_left(iteration & 31) ^ 0x5EED_5EED;
                let mut values = Vec::with_capacity(len);
                for word in 0..len {
                    state = state.rotate_left(7)
                        ^ (iteration.wrapping_mul(0x9E37_79B9))
                        ^ (word as u32).wrapping_mul(0x85EB_CA6B);
                    values.push(match word % 4 {
                        0 => state,
                        1 => !state,
                        2 => state.wrapping_add(u32::MAX),
                        _ => state ^ (1_u32 << ((iteration as usize + word) & 31)),
                    });
                }
                let program = (case.build)("values", "state", "trace", values.len() as u32);
                assert_serial_matches(&program, case.kind, &values, initial);
            }
        }
    }
}
