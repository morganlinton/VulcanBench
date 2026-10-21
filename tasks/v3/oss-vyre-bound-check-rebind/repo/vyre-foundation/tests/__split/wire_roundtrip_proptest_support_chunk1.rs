// Wire-format round-trip property tests.
// (allow(dead_code) moved to parent wire_roundtrip_proptest.rs)

use proptest::collection::vec as prop_vec;
use proptest::prelude::*;
use smallvec::smallvec;
use std::sync::Arc;
use vyre_foundation::extension::{OpaqueExprResolver, OpaqueNodeResolver};
use vyre_foundation::ir::{
    AtomicOp, BinOp, BufferDecl, DataType, Expr, ExprNode, Node, NodeExtension, Program, UnOp,
};
use vyre_foundation::MemoryOrdering;
use vyre_spec::data_type::TypeId;
use vyre_spec::extension::{
    ExtensionAtomicOpId, ExtensionBinOpId, ExtensionDataTypeId, ExtensionUnOpId,
};

const VAR_NAMES: &[&str] = &["", "x", "alpha", "snow_雪", "nul\0name"];
const CALL_IDS: &[&str] = &["", "call", "筛选", "op::雪"];
const TAG_NAMES: &[&str] = &["", "tag", "stream-雪", "wait\0tag"];
const BUFFER_NAMES: &[&str] = &[
    "out",
    "input",
    "rw",
    "bytes_in",
    "bytes_out",
    "counts",
    "scratch",
];

const EXPR_OPAQUE_KIND: &str = "test.wire.expr";
const NODE_OPAQUE_KIND: &str = "test.wire.node";

#[derive(Debug)]
struct TestOpaqueExpr {
    payload: Vec<u8>,
}

impl ExprNode for TestOpaqueExpr {
    fn extension_kind(&self) -> &'static str {
        EXPR_OPAQUE_KIND
    }

    fn debug_identity(&self) -> &str {
        "wire-expr"
    }

    fn result_type(&self) -> Option<DataType> {
        Some(DataType::U32)
    }

    fn cse_safe(&self) -> bool {
        true
    }

    fn stable_fingerprint(&self) -> [u8; 32] {
        *blake3::hash(&self.payload).as_bytes()
    }

    fn validate_extension(&self) -> Result<(), String> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn wire_payload(&self) -> Vec<u8> {
        self.payload.clone()
    }
}

fn deserialize_test_opaque_expr(bytes: &[u8]) -> Result<Arc<dyn ExprNode>, String> {
    Ok(Arc::new(TestOpaqueExpr {
        payload: bytes.to_vec(),
    }))
}

inventory::submit! {
    OpaqueExprResolver {
        kind: EXPR_OPAQUE_KIND,
        deserialize: deserialize_test_opaque_expr,
    }
}

#[derive(Debug)]
struct TestOpaqueNode {
    payload: Vec<u8>,
}

impl NodeExtension for TestOpaqueNode {
    fn extension_kind(&self) -> &'static str {
        NODE_OPAQUE_KIND
    }

    fn debug_identity(&self) -> &str {
        "wire-node"
    }

    fn stable_fingerprint(&self) -> [u8; 32] {
        *blake3::hash(&self.payload).as_bytes()
    }

    fn validate_extension(&self) -> Result<(), String> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn wire_payload(&self) -> Vec<u8> {
        self.payload.clone()
    }
}

fn deserialize_test_opaque_node(bytes: &[u8]) -> Result<Arc<dyn NodeExtension>, String> {
    Ok(Arc::new(TestOpaqueNode {
        payload: bytes.to_vec(),
    }))
}

inventory::submit! {
    OpaqueNodeResolver {
        kind: NODE_OPAQUE_KIND,
        deserialize: deserialize_test_opaque_node,
    }
}

fn arb_ident() -> BoxedStrategy<String> {
    prop::sample::select(VAR_NAMES.to_vec())
        .prop_map(str::to_string)
        .boxed()
}

fn arb_call_id() -> BoxedStrategy<String> {
    prop::sample::select(CALL_IDS.to_vec())
        .prop_map(str::to_string)
        .boxed()
}

fn arb_tag() -> BoxedStrategy<String> {
    prop::sample::select(TAG_NAMES.to_vec())
        .prop_map(str::to_string)
        .boxed()
}

fn arb_axis() -> BoxedStrategy<u8> {
    prop_oneof![Just(0), Just(1), Just(2), Just(255)].boxed()
}

fn arb_opaque_bytes() -> BoxedStrategy<Vec<u8>> {
    prop_vec(
        prop_oneof![
            Just(0x00u8),
            Just(0xffu8),
            Just(0xc0u8),
            Just(0xafu8),
            any::<u8>(),
        ],
        0..=24,
    )
    .boxed()
}

fn arb_datatype() -> BoxedStrategy<DataType> {
    let leaf = prop_oneof![
        Just(DataType::U8),
        Just(DataType::U16),
        Just(DataType::U32),
        Just(DataType::I8),
        Just(DataType::I16),
        Just(DataType::I32),
        Just(DataType::I64),
        Just(DataType::U64),
        Just(DataType::Vec2U32),
        Just(DataType::Vec4U32),
        Just(DataType::Bool),
        Just(DataType::Bytes),
        (0usize..=64).prop_map(|element_size| DataType::Array { element_size }),
        Just(DataType::F16),
        Just(DataType::BF16),
        Just(DataType::F32),
        Just(DataType::F64),
        Just(DataType::Tensor),
        any::<u32>().prop_map(|id| DataType::Handle(TypeId(id))),
        any::<u32>().prop_map(|id| DataType::Opaque(ExtensionDataTypeId(id | 0x8000_0000))),
    ];

    leaf.prop_recursive(3, 24, 3, |inner| {
        prop_oneof![
            (inner.clone(), 0u8..=4).prop_map(|(element, count)| DataType::Vec {
                element: Box::new(element),
                count,
            }),
            (inner.clone(), prop_vec(any::<u32>(), 0..=4),).prop_map(|(element, shape)| {
                DataType::TensorShaped {
                    element: Box::new(element),
                    shape: shape.into_iter().collect(),
                }
            }),
        ]
    })
    .boxed()
}

fn arb_buffer_datatype() -> BoxedStrategy<DataType> {
    prop_oneof![
        Just(DataType::U8),
        Just(DataType::U16),
        Just(DataType::U32),
        Just(DataType::I8),
        Just(DataType::I16),
        Just(DataType::I32),
        Just(DataType::I64),
        Just(DataType::U64),
        Just(DataType::Vec2U32),
        Just(DataType::Vec4U32),
        Just(DataType::Bool),
        Just(DataType::Bytes),
        (0usize..=64).prop_map(|element_size| DataType::Array { element_size }),
        Just(DataType::F16),
        Just(DataType::BF16),
        Just(DataType::F32),
        Just(DataType::F64),
        Just(DataType::Tensor),
    ]
    .boxed()
}

fn arb_literal() -> BoxedStrategy<Expr> {
    let adversarial_f32_bits = prop_oneof![
        Just(0x0000_0000u32),
        Just(0x0000_0001u32),
        Just(0x007f_ffffu32),
        Just(f32::MIN_POSITIVE.to_bits()),
        Just(f32::MIN.to_bits()),
        Just(f32::MAX.to_bits()),
        any::<u32>().prop_filter(
            "general proptest literals exclude NaN and -0.0; targeted tests cover payload preservation and canonical zero encoding",
            |bits| !f32::from_bits(*bits).is_nan() && *bits != (-0.0f32).to_bits()
        ),
    ];

    prop_oneof![
        any::<u32>().prop_map(Expr::LitU32),
        any::<i32>().prop_map(Expr::LitI32),
        any::<bool>().prop_map(Expr::LitBool),
        adversarial_f32_bits.prop_map(|bits| Expr::LitF32(f32::from_bits(bits))),
    ]
    .boxed()
}

fn arb_expr() -> BoxedStrategy<Expr> {
    let leaf = prop_oneof![
        arb_literal(),
        arb_ident().prop_map(Expr::var),
        prop::sample::select(BUFFER_NAMES.to_vec()).prop_map(Expr::buf_len),
        arb_axis().prop_map(|axis| Expr::InvocationId { axis }),
        arb_axis().prop_map(|axis| Expr::WorkgroupId { axis }),
        arb_axis().prop_map(|axis| Expr::LocalId { axis }),
        arb_opaque_bytes().prop_map(|payload| Expr::Opaque(Arc::new(TestOpaqueExpr { payload }))),
    ];

    leaf.prop_recursive(4, 128, 4, |inner| {
        prop_oneof![
            (prop::sample::select(BUFFER_NAMES.to_vec()), inner.clone(),).prop_map(
                |(buffer, index)| Expr::Load {
                    buffer: buffer.into(),
                    index: Box::new(index),
                }
            ),
            (
                prop_oneof![
                    Just(BinOp::Add),
                    Just(BinOp::Sub),
                    Just(BinOp::Mul),
                    Just(BinOp::Div),
                    Just(BinOp::Mod),
                    Just(BinOp::BitAnd),
                    Just(BinOp::BitOr),
                    Just(BinOp::BitXor),
                    Just(BinOp::Shl),
                    Just(BinOp::Shr),
                    Just(BinOp::Eq),
                    Just(BinOp::Ne),
                    Just(BinOp::Lt),
                    Just(BinOp::Gt),
                    Just(BinOp::Le),
                    Just(BinOp::Ge),
                    Just(BinOp::And),
                    Just(BinOp::Or),
                    Just(BinOp::AbsDiff),
                    Just(BinOp::Min),
                    Just(BinOp::Max),
                    Just(BinOp::SaturatingAdd),
                    Just(BinOp::SaturatingSub),
                    Just(BinOp::SaturatingMul),
                    Just(BinOp::Shuffle),
                    Just(BinOp::Ballot),
                    Just(BinOp::WaveReduce),
                    Just(BinOp::WaveBroadcast),
                    Just(BinOp::RotateLeft),
                    Just(BinOp::RotateRight),
                    any::<u32>().prop_map(|id| BinOp::Opaque(ExtensionBinOpId(id | 0x8000_0000))),
                ],
                inner.clone(),
                inner.clone(),
            )
                .prop_map(|(op, left, right)| Expr::BinOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                }),
            (
                prop_oneof![
                    Just(UnOp::Negate),
                    Just(UnOp::BitNot),
                    Just(UnOp::LogicalNot),
                    Just(UnOp::Popcount),
                    Just(UnOp::Clz),
                    Just(UnOp::Ctz),
                    Just(UnOp::ReverseBits),
                    Just(UnOp::Cos),
                    Just(UnOp::Sin),
                    Just(UnOp::Abs),
                    Just(UnOp::Sqrt),
                    Just(UnOp::Floor),
                    Just(UnOp::Ceil),
                    Just(UnOp::Round),
                    Just(UnOp::Trunc),
                    Just(UnOp::Sign),
                    Just(UnOp::IsNan),
                    Just(UnOp::IsInf),
                    Just(UnOp::IsFinite),
                    Just(UnOp::Exp),
                    Just(UnOp::Log),
                    Just(UnOp::Log2),
                    Just(UnOp::Exp2),
                    Just(UnOp::Tan),
                    Just(UnOp::Acos),
                    Just(UnOp::Asin),
                    Just(UnOp::Atan),
                    Just(UnOp::Tanh),
                    Just(UnOp::Sinh),
                    Just(UnOp::Cosh),
                    Just(UnOp::InverseSqrt),
                    Just(UnOp::Reciprocal),
                    Just(UnOp::Unpack4Low),
                    Just(UnOp::Unpack4High),
                    Just(UnOp::Unpack8Low),
                    Just(UnOp::Unpack8High),
                    any::<u32>().prop_map(|id| UnOp::Opaque(ExtensionUnOpId(id | 0x8000_0000))),
                ],
                inner.clone(),
            )
                .prop_map(|(op, operand)| Expr::UnOp {
                    op,
                    operand: Box::new(operand),
                }),
            (arb_call_id(), prop_vec(inner.clone(), 0..=4),).prop_map(|(op_id, args)| Expr::Call {
                op_id: op_id.into(),
                args,
            }),
            (inner.clone(), inner.clone(), inner.clone(),).prop_map(
                |(cond, true_val, false_val)| Expr::Select {
                    cond: Box::new(cond),
                    true_val: Box::new(true_val),
                    false_val: Box::new(false_val),
                }
            ),
            (arb_buffer_datatype(), inner.clone()).prop_map(|(target, value)| Expr::Cast {
                target,
                value: Box::new(value),
            }),
            (inner.clone(), inner.clone(), inner.clone(),).prop_map(|(a, b, c)| Expr::Fma {
                a: Box::new(a),
                b: Box::new(b),
                c: Box::new(c),
            }),
            (
                prop_oneof![
                    Just(AtomicOp::Add),
                    Just(AtomicOp::Or),
                    Just(AtomicOp::And),
                    Just(AtomicOp::Xor),
                    Just(AtomicOp::Min),
                    Just(AtomicOp::Max),
                    Just(AtomicOp::Exchange),
                    Just(AtomicOp::CompareExchange),
                    Just(AtomicOp::CompareExchangeWeak),
                    Just(AtomicOp::FetchNand),
                    Just(AtomicOp::LruUpdate),
                    any::<u32>()
                        .prop_map(|id| AtomicOp::Opaque(ExtensionAtomicOpId(id | 0x8000_0000))),
                ],
                prop::sample::select(vec!["rw", "out", "counts", "bytes_out"]),
                inner.clone(),
                proptest::option::of(inner.clone()),
                inner.clone(),
            )
                .prop_map(|(op, buffer, index, expected, value)| Expr::Atomic {
                    op,
                    buffer: buffer.into(),
                    index: Box::new(index),
                    expected: expected.map(Box::new),
                    value: Box::new(value),
                    ordering: MemoryOrdering::SeqCst,
                }),
            inner.clone().prop_map(|value| Expr::subgroup_add(value)),
            (inner.clone(), inner.clone()).prop_map(|(value, lane)| Expr::SubgroupShuffle {
                value: Box::new(value),
                lane: Box::new(lane),
            }),
            inner.prop_map(|cond| Expr::SubgroupBallot {
                cond: Box::new(cond),
            }),
        ]
    })
    .boxed()
}

