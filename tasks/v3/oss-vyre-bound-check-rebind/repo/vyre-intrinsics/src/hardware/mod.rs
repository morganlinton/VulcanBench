//! Cat-C hardware intrinsics  -  each ships a builder, CPU reference
//! (in `vyre-reference`), and dedicated target builder emitter arm. Backends
//! that cannot lower return `UnsupportedByBackend` rather than
//! falling back to slow CPU paths.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

macro_rules! define_unary_u32_hardware_intrinsic {
    (
        $module:ident,
        $function:ident,
        $op_id:literal,
        $expr:path,
        $cpu_map:expr,
        $fixture:expr,
        $seed:expr,
        $one_case:expr,
        $max_case:expr,
        $doc:literal
    ) => {
        #[doc = $doc]
        pub mod $module {
            use vyre_foundation::ir::Program;

            /// Canonical op id.
            pub const OP_ID: &str = $op_id;

            /// Build the canonical u32 unary hardware intrinsic program.
            #[must_use]
            pub fn $function(input: &str, out: &str, n: u32) -> Program {
                crate::hardware::unary_u32_program(OP_ID, input, out, n, $expr)
            }

            fn cpu_ref(input: &[u32]) -> Vec<u8> {
                let map_lane = $cpu_map;
                let output: Vec<u32> = input.iter().copied().map(map_lane).collect();
                crate::hardware::pack_u32(&output)
            }

            fn fixture_input() -> Vec<u32> {
                $fixture.to_vec()
            }

            fn test_inputs() -> Vec<Vec<Vec<u8>>> {
                let input = fixture_input();
                let len = input.len() * 4;
                vec![vec![crate::hardware::pack_u32(&input), vec![0u8; len]]]
            }

            fn expected_output() -> Vec<Vec<Vec<u8>>> {
                let input = fixture_input();
                vec![vec![cpu_ref(&input)]]
            }

            inventory::submit! {
                crate::harness::OpEntry {
                    id: OP_ID,
                    build: || $function("input", "out", 4),
                    test_inputs: Some(test_inputs),
                    expected_output: Some(expected_output),
                    category: Some("hardware"),
                    shape: Some(crate::harness::OpShape::new(
                        1,
                        1,
                        4,
                        crate::harness::HardwareSemantic::UnaryU32Map,
                    )),
                }
            }

            #[cfg(test)]
            mod tests {
                use super::*;
                use crate::hardware::{lcg_u32, pack_u32, run_program};

                fn assert_case(input: &[u32]) {
                    let n = input.len() as u32;
                    let program = $function("input", "out", n.max(1));
                    let outputs = run_program(
                        &program,
                        vec![pack_u32(input), vec![0u8; (n.max(1) * 4) as usize]],
                    );
                    assert_eq!(outputs, vec![cpu_ref(input)]);
                }

                #[test]
                fn one_element() {
                    assert_case($one_case);
                }

                #[test]
                fn max_value() {
                    assert_case($max_case);
                }

                #[test]
                fn random_sixty_four() {
                    let input = lcg_u32($seed, 64);
                    assert_case(&input);
                }
            }
        }

        pub use $module::$function;
    };
}

macro_rules! define_barrier_u32_hardware_intrinsic {
    (
        $module:ident,
        $function:ident,
        $op_id:literal,
        $fixture:expr,
        $seed:expr,
        $one_case:expr,
        $doc:literal
    ) => {
        #[doc = $doc]
        pub mod $module {
            use vyre_foundation::ir::Program;

            const OP_ID: &str = $op_id;

            /// Build a Program that emits this memory barrier after an identity u32 store.
            #[must_use]
            pub fn $function(input: &str, out: &str, n: u32) -> Program {
                crate::hardware::barrier_identity_u32_program(OP_ID, input, out, n)
            }

            fn cpu_ref(input: &[u32]) -> Vec<u8> {
                crate::hardware::pack_u32(input)
            }

            fn fixture_input() -> Vec<u32> {
                $fixture.to_vec()
            }

            fn test_inputs() -> Vec<Vec<Vec<u8>>> {
                let input = fixture_input();
                let len = input.len() * 4;
                vec![vec![crate::hardware::pack_u32(&input), vec![0u8; len]]]
            }

            fn expected_output() -> Vec<Vec<Vec<u8>>> {
                let input = fixture_input();
                vec![vec![cpu_ref(&input)]]
            }

            inventory::submit! {
                crate::harness::OpEntry {
                    id: OP_ID,
                    build: || $function("input", "out", 4),
                    test_inputs: Some(test_inputs),
                    expected_output: Some(expected_output),
                    category: Some("hardware"),
                    shape: Some(crate::harness::OpShape::new(
                        1,
                        1,
                        4,
                        crate::harness::HardwareSemantic::BarrierIdentityU32,
                    )),
                }
            }

            #[cfg(test)]
            mod tests {
                use super::*;
                use crate::hardware::{lcg_u32, pack_u32, run_program};

                fn assert_case(input: &[u32]) {
                    let n = input.len() as u32;
                    let program = $function("input", "out", n.max(1));
                    let outputs = run_program(
                        &program,
                        vec![pack_u32(input), vec![0u8; (n.max(1) * 4) as usize]],
                    );
                    assert_eq!(outputs, vec![cpu_ref(input)]);
                }

                #[test]
                fn one_element() {
                    assert_case($one_case);
                }

                #[test]
                fn random_sixty_four() {
                    let input = lcg_u32($seed, 64);
                    assert_case(&input);
                }
            }
        }

        pub use $module::$function;
    };
}

/// `bit_reverse_u32`  -  reverses every bit in each u32 lane via hardware `reverseBits`.
pub mod bit_reverse_u32 {
    define_unary_u32_hardware_intrinsic!(
        bit_reverse_u32,
        bit_reverse_u32,
        "vyre-intrinsics::hardware::bit_reverse_u32",
        vyre_foundation::ir::Expr::reverse_bits,
        |value: u32| value.reverse_bits(),
        &[0u32, 1, 0x8000_0000, 0x1234_5678],
        0x1EA0_7733,
        &[1],
        &[u32::MAX],
        "Cat-C `bit_reverse_u32` - reverse the bit order within each u32 lane."
    );
}
/// `fma_f32`  -  IEEE-754 fused multiply-add (byte-identical to `f32::mul_add`).
pub mod fma_f32;
/// `inverse_sqrt_f32`  -  hardware `inverseSqrt()` approximation.
pub mod inverse_sqrt_f32;
/// `popcount_u32`  -  hardware `countOneBits` on each u32 lane.
pub mod popcount_u32 {
    define_unary_u32_hardware_intrinsic!(
        popcount_u32,
        popcount_u32,
        "vyre-intrinsics::hardware::popcount_u32",
        vyre_foundation::ir::Expr::popcount,
        |value: u32| value.count_ones(),
        &[0u32, 1, 0xFFFF_FFFF, 0x1234_5678],
        0xC0FF_EE11,
        &[1],
        &[u32::MAX],
        "Cat-C `popcount_u32` - count set bits in each u32 lane."
    );
}
/// `storage_barrier`  -  cross-workgroup storage-buffer memory fence.
pub mod storage_barrier {
    define_barrier_u32_hardware_intrinsic!(
        storage_barrier,
        storage_barrier,
        "vyre-intrinsics::hardware::storage_barrier",
        &[10u32, 20, 30, 40],
        0xB200_0022,
        &[7],
        "Cat-C `storage_barrier` - storage-scope memory fence after identity store."
    );
}
/// `subgroup_add`  -  wave-level reduction over the subgroup.
pub mod subgroup_add;
/// `subgroup_ballot`  -  wave-level predicate ballot bitmask.
pub mod subgroup_ballot;
/// `subgroup_shuffle`  -  wave-level lane-to-lane value shuffle.
pub mod subgroup_shuffle;
/// `workgroup_barrier`  -  intra-workgroup shared-memory fence.
pub mod workgroup_barrier {
    define_barrier_u32_hardware_intrinsic!(
        workgroup_barrier,
        workgroup_barrier,
        "vyre-intrinsics::hardware::workgroup_barrier",
        &[1u32, 2, 3, 4],
        0xB100_0011,
        &[42],
        "Cat-C `workgroup_barrier` - workgroup-scope memory fence after identity store."
    );
}

pub(crate) const MAP_WORKGROUP: [u32; 3] = [64, 1, 1];

pub(crate) fn unary_u32_program<F>(
    op_id: &'static str,
    input: &str,
    out: &str,
    n: u32,
    expr: F,
) -> Program
where
    F: Fn(Expr) -> Expr,
{
    let body = vec![crate::region::wrap_anonymous(
        op_id,
        vec![
            Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::buf_len(out)),
                vec![Node::store(
                    out,
                    Expr::var("idx"),
                    expr(Expr::load(input, Expr::var("idx"))),
                )],
            ),
        ],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::output(out, 1, DataType::U32).with_count(n),
        ],
        MAP_WORKGROUP,
        body,
    )
}

pub(crate) fn barrier_identity_u32_program(
    op_id: &'static str,
    input: &str,
    out: &str,
    n: u32,
) -> Program {
    let body = vec![crate::region::wrap_anonymous(
        op_id,
        vec![
            Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::buf_len(out)),
                vec![Node::store(
                    out,
                    Expr::var("idx"),
                    Expr::load(input, Expr::var("idx")),
                )],
            ),
            Node::barrier(),
        ],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::output(out, 1, DataType::U32).with_count(n),
        ],
        MAP_WORKGROUP,
        body,
    )
}

pub(crate) fn ternary_f32_program(a: &str, b: &str, c: &str, out: &str, n: u32) -> Program {
    let body = vec![crate::region::wrap_anonymous(
        "vyre-intrinsics::hardware::ternary_f32_map",
        vec![
            Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::buf_len(out)),
                vec![Node::store(
                    out,
                    Expr::var("idx"),
                    Expr::Fma {
                        a: Box::new(Expr::load(a, Expr::var("idx"))),
                        b: Box::new(Expr::load(b, Expr::var("idx"))),
                        c: Box::new(Expr::load(c, Expr::var("idx"))),
                    },
                )],
            ),
        ],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(b, 1, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::storage(c, 2, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(out, 3, DataType::F32).with_count(n),
        ],
        MAP_WORKGROUP,
        body,
    )
}

pub(crate) fn pack_u32(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

pub(crate) fn pack_f32(values: &[f32]) -> Vec<u8> {
    vyre_primitives::wire::pack_f32_slice(values)
}

#[cfg(test)]
pub(crate) fn run_program(program: &Program, inputs: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    use vyre_reference::value::Value;
    let values: Vec<Value> = inputs.into_iter().map(|b| Value::Bytes(b.into())).collect();
    vyre_reference::reference_eval(program, &values)
        .expect("Fix: intrinsic must execute; restore this invariant before continuing.")
        .into_iter()
        .map(|v| v.to_bytes())
        .collect()
}

#[cfg(test)]
pub(crate) fn lcg_u32(seed: u32, len: usize) -> Vec<u32> {
    let mut s = seed;
    (0..len)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            s
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn lcg_f32(seed: u32, len: usize) -> Vec<f32> {
    lcg_u32(seed, len)
        .into_iter()
        .map(|w| f32::from_bits((w >> 9) | 0x3F00_0000) - 1.0)
        .collect()
}
