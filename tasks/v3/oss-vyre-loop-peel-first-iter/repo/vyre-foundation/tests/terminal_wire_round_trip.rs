//! Terminal enum variant wire-format round trips.

use smallvec::smallvec;
use vyre_foundation::ir::{AtomicOp, BinOp, BufferDecl, DataType, Expr, Node, Program, UnOp};
use vyre_foundation::MemoryOrdering;
use vyre_spec::data_type::TypeId;
use vyre_spec::extension::{
    ExtensionAtomicOpId, ExtensionBinOpId, ExtensionDataTypeId, ExtensionUnOpId,
};

fn round_trip(program: Program) {
    let encoded = program
        .to_wire()
        .unwrap_or_else(|error| panic!("Fix: terminal variant program must encode: {error}"));
    let decoded = Program::from_wire(&encoded)
        .unwrap_or_else(|error| panic!("Fix: terminal variant program must decode: {error}"));
    assert_eq!(decoded, program);
}

#[test]
fn every_terminal_data_type_round_trips_in_cast_targets() {
    let variants = vec![
        DataType::U32,
        DataType::I32,
        DataType::U64,
        DataType::Vec2U32,
        DataType::Vec4U32,
        DataType::Bool,
        DataType::Bytes,
        DataType::Array { element_size: 16 },
        DataType::F16,
        DataType::BF16,
        DataType::F32,
        DataType::F64,
        DataType::Tensor,
        DataType::U8,
        DataType::U16,
        DataType::I8,
        DataType::I16,
        DataType::I64,
        DataType::Handle(TypeId(0x1357_2468)),
        DataType::Vec {
            element: Box::new(DataType::U16),
            count: 3,
        },
        DataType::TensorShaped {
            element: Box::new(DataType::F32),
            shape: smallvec![2, 3, 5, 7],
        },
        DataType::Opaque(ExtensionDataTypeId(0x8000_0001)),
    ];

    let nodes = variants
        .into_iter()
        .enumerate()
        .map(|(idx, target)| {
            Node::let_bind(
                format!("cast_{idx}"),
                Expr::Cast {
                    target,
                    value: Box::new(Expr::u32(idx as u32)),
                },
            )
        })
        .chain([Node::Return])
        .collect();

    round_trip(Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        nodes,
    ));
}

#[test]
fn every_terminal_binop_round_trips() {
    let variants = vec![
        BinOp::Add,
        BinOp::Sub,
        BinOp::Mul,
        BinOp::Div,
        BinOp::Mod,
        BinOp::BitAnd,
        BinOp::BitOr,
        BinOp::BitXor,
        BinOp::Shl,
        BinOp::Shr,
        BinOp::Eq,
        BinOp::Ne,
        BinOp::Lt,
        BinOp::Gt,
        BinOp::Le,
        BinOp::Ge,
        BinOp::And,
        BinOp::Or,
        BinOp::AbsDiff,
        BinOp::Min,
        BinOp::Max,
        BinOp::SaturatingAdd,
        BinOp::SaturatingSub,
        BinOp::SaturatingMul,
        BinOp::Shuffle,
        BinOp::Ballot,
        BinOp::WaveReduce,
        BinOp::WaveBroadcast,
        BinOp::Opaque(ExtensionBinOpId(0x8000_0101)),
    ];

    let nodes = variants
        .into_iter()
        .enumerate()
        .map(|(idx, op)| {
            Node::let_bind(
                format!("bin_{idx}"),
                Expr::BinOp {
                    op,
                    left: Box::new(Expr::u32(1)),
                    right: Box::new(Expr::u32(2)),
                },
            )
        })
        .chain([Node::Return])
        .collect();

    round_trip(Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        nodes,
    ));
}

#[test]
fn every_terminal_unop_round_trips() {
    let variants = vec![
        UnOp::Negate,
        UnOp::BitNot,
        UnOp::LogicalNot,
        UnOp::Popcount,
        UnOp::Clz,
        UnOp::Ctz,
        UnOp::ReverseBits,
        UnOp::Cos,
        UnOp::Sin,
        UnOp::Abs,
        UnOp::Sqrt,
        UnOp::Floor,
        UnOp::Ceil,
        UnOp::Round,
        UnOp::Trunc,
        UnOp::Sign,
        UnOp::IsNan,
        UnOp::IsInf,
        UnOp::IsFinite,
        UnOp::Exp,
        UnOp::Log,
        UnOp::Log2,
        UnOp::Exp2,
        UnOp::Tan,
        UnOp::Acos,
        UnOp::Asin,
        UnOp::Atan,
        UnOp::Tanh,
        UnOp::Sinh,
        UnOp::Cosh,
        UnOp::InverseSqrt,
        UnOp::Reciprocal,
        UnOp::Unpack4Low,
        UnOp::Unpack4High,
        UnOp::Unpack8Low,
        UnOp::Unpack8High,
        UnOp::Opaque(ExtensionUnOpId(0x8000_0202)),
    ];

    let nodes = variants
        .into_iter()
        .enumerate()
        .map(|(idx, op)| {
            Node::let_bind(
                format!("un_{idx}"),
                Expr::UnOp {
                    op,
                    operand: Box::new(Expr::u32(1)),
                },
            )
        })
        .chain([Node::Return])
        .collect();

    round_trip(Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        nodes,
    ));
}

#[test]
fn every_terminal_atomic_op_round_trips() {
    let variants = vec![
        AtomicOp::Add,
        AtomicOp::Or,
        AtomicOp::And,
        AtomicOp::Xor,
        AtomicOp::Min,
        AtomicOp::Max,
        AtomicOp::Exchange,
        AtomicOp::CompareExchange,
        AtomicOp::CompareExchangeWeak,
        AtomicOp::FetchNand,
        AtomicOp::LruUpdate,
        AtomicOp::Opaque(ExtensionAtomicOpId(0x8000_0303)),
    ];

    let nodes = variants
        .into_iter()
        .enumerate()
        .map(|(idx, op)| {
            Node::let_bind(
                format!("atomic_{idx}"),
                Expr::Atomic {
                    op,
                    buffer: "out".into(),
                    index: Box::new(Expr::u32(0)),
                    expected: matches!(
                        op,
                        AtomicOp::CompareExchange | AtomicOp::CompareExchangeWeak
                    )
                    .then(|| Box::new(Expr::u32(1))),
                    value: Box::new(Expr::u32(2)),
                    ordering: MemoryOrdering::SeqCst,
                },
            )
        })
        .chain([Node::Return])
        .collect();

    round_trip(Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        nodes,
    ));
}
