use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// ABI type-kind tag for C `char`.
pub const C_ABI_CHAR: u32 = 1;
/// ABI type-kind tag for C pointer-sized objects.
pub const C_ABI_POINTER: u32 = 2;
/// ABI type-kind tag for C `long` / 64-bit integer-sized objects.
pub const C_ABI_LONG: u32 = 3;
/// ABI type-kind tag for C `double`.
pub const C_ABI_DOUBLE: u32 = 4;

/// GPU System V ABI Alignment & Sizeof Evaluator
///
/// Ensures strict CPU cache-line compliance by aligning struct members.
/// A parallel scan computes inclusive offsets across struct blueprints, accounting
/// for byte padding natively across the GPU topology.
#[must_use]
pub fn c11_compute_alignments(
    type_definitions: &str,
    out_sizes: &str,
    out_alignments: &str,
    num_types: Expr,
) -> Program {
    c11_compute_alignments_for_abi(
        type_definitions,
        out_sizes,
        out_alignments,
        num_types,
        8,
        8,
        8,
    )
}

/// GPU C ABI Alignment & Sizeof Evaluator for an explicit C data model.
///
/// The production frontend uses this for target-sensitive semantic evidence:
/// `-m64` maps to LP64 (`pointer=8`, `long=8`) and `-m32` maps to ILP32
/// (`pointer=4`, `long=4`, `double_align=4`). Invalid sizes are rejected at builder time so a
/// miswired target ABI cannot silently emit host-shaped layout evidence.
#[must_use]
pub fn c11_compute_alignments_for_abi(
    type_definitions: &str,
    out_sizes: &str,
    out_alignments: &str,
    num_types: Expr,
    pointer_size_bytes: u32,
    long_size_bytes: u32,
    double_alignment_bytes: u32,
) -> Program {
    assert!(
        matches!(pointer_size_bytes, 4 | 8),
        "c11_compute_alignments_for_abi pointer size must be 4 or 8 bytes"
    );
    assert!(
        matches!(long_size_bytes, 4 | 8),
        "c11_compute_alignments_for_abi long size must be 4 or 8 bytes"
    );
    assert!(
        matches!(double_alignment_bytes, 4 | 8),
        "c11_compute_alignments_for_abi double alignment must be 4 or 8 bytes"
    );
    let t = Expr::InvocationId { axis: 0 };

    let loop_body = vec![
        Node::let_bind("type_kind", Expr::load(type_definitions, t.clone())),
        Node::let_bind(
            "size_bytes",
            Expr::select(
                Expr::eq(Expr::var("type_kind"), Expr::u32(C_ABI_CHAR)),
                Expr::u32(1),
                Expr::select(
                    Expr::or(
                        Expr::eq(Expr::var("type_kind"), Expr::u32(C_ABI_POINTER)),
                        Expr::eq(Expr::var("type_kind"), Expr::u32(C_ABI_LONG)),
                    ),
                    Expr::select(
                        Expr::eq(Expr::var("type_kind"), Expr::u32(C_ABI_POINTER)),
                        Expr::u32(pointer_size_bytes),
                        Expr::u32(long_size_bytes),
                    ),
                    Expr::select(
                        Expr::eq(Expr::var("type_kind"), Expr::u32(C_ABI_DOUBLE)),
                        Expr::u32(8),
                        Expr::u32(4),
                    ),
                ),
            ),
        ),
        Node::let_bind(
            "align_bytes",
            Expr::select(
                Expr::eq(Expr::var("type_kind"), Expr::u32(C_ABI_DOUBLE)),
                Expr::u32(double_alignment_bytes),
                Expr::var("size_bytes"),
            ),
        ),
        Node::store(out_sizes, t.clone(), Expr::var("size_bytes")),
        Node::store(out_alignments, t.clone(), Expr::var("align_bytes")),
    ];

    let type_count = match &num_types {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(type_definitions, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(type_count),
            BufferDecl::storage(out_sizes, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(type_count),
            BufferDecl::storage(out_alignments, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(type_count),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::c11_compute_alignments",
            vec![Node::if_then(Expr::lt(t.clone(), num_types), loop_body)],
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::c11_compute_alignments",
        build: || c11_compute_alignments("types", "sizes", "aligns", Expr::u32(5)),
        test_inputs: Some(|| vec![vec![
            vyre_primitives::wire::pack_u32_slice(&[
                C_ABI_CHAR,
                C_ABI_POINTER,
                C_ABI_LONG,
                C_ABI_DOUBLE,
                0,
            ]),
            vec![0u8; 5 * 4],
            vec![0u8; 5 * 4],
        ]]),
        expected_output: Some(|| {
            let sizes = vyre_primitives::wire::pack_u32_slice(&[1u32, 8, 8, 8, 4]);
            let aligns = vyre_primitives::wire::pack_u32_slice(&[1u32, 8, 8, 8, 4]);
            vec![vec![sizes, aligns]]
        }),
        category: Some("compiler"),
    }
}
