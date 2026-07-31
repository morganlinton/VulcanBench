//! Sweep oracle matrix for VIR0 wire encode/decode round-trips.
//!
//! Builds hostile `Program` shapes whose literals and casts exercise the
//! frozen `vyre_spec` wire tag surface, then pins byte-identical canonical
//! round-trip idempotence: encode → decode → re-encode must match bytes.

#![forbid(unsafe_code)]

use smallvec::smallvec;
use vyre_foundation::ir::{BufferDecl, Expr, Node, Program};
use vyre_spec::extension::{
    ExtensionAtomicOpId, ExtensionBinOpId, ExtensionDataTypeId, ExtensionUnOpId,
};
use vyre_spec::{
    AtomicOp, BinOp, DataType, QuantizationScale, QuantizationZeroPoint, TypeId, UnOp,
};

const CASES: usize = 1024;

#[derive(Clone, Copy)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 7;
        x ^= x >> 9;
        x ^= x << 8;
        self.0 = x;
        (x >> 16) as u32
    }

    fn range(&mut self, upper: u32) -> u32 {
        if upper == 0 {
            0
        } else {
            self.next_u32() % upper
        }
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.range(items.len() as u32) as usize]
    }

    fn pick_str<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.range(items.len() as u32) as usize]
    }
}

#[test]
fn sweep_wire_roundtrip_oracle_matrix_preserves_canonical_bytes() {
    let mut assertions = 0usize;
    for case in 0..CASES {
        let program = hostile_program(case as u64);
        let encoded = program
            .to_wire()
            .unwrap_or_else(|error| panic!("Fix: hostile wire case {case} must encode: {error}"));
        let decoded = Program::from_wire(&encoded)
            .unwrap_or_else(|error| panic!("Fix: hostile wire case {case} must decode: {error}"));

        let reencoded = decoded.to_wire().unwrap_or_else(|error| {
            panic!("Fix: hostile wire case {case} must re-encode canonically: {error}")
        });
        assert_eq!(
            reencoded, encoded,
            "Fix: hostile wire case {case} canonical bytes drifted after round-trip"
        );
        assertions += 1;

        let redecoded = Program::from_wire(&reencoded).unwrap_or_else(|error| {
            panic!("Fix: hostile wire case {case} must decode canonical bytes: {error}")
        });
        let roundtrip_again = redecoded.to_wire().unwrap_or_else(|error| {
            panic!("Fix: hostile wire case {case} must triple-encode: {error}")
        });
        assert_eq!(
            roundtrip_again, encoded,
            "Fix: hostile wire case {case} lost byte identity on second canonical encode"
        );
        assertions += 1;

        assert_ne!(
            encoded.len(),
            0,
            "Fix: hostile wire case {case} must emit non-empty bytes"
        );
        assertions += 1;
    }
    assert_eq!(assertions, CASES * 3);
}

fn hostile_program(case: u64) -> Program {
    let mut rng = Rng::new(0xC0DE_CAFE_0000_0001 ^ case.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let dtype_a = hostile_buffer_datatype(&mut rng);
    let dtype_b = hostile_buffer_datatype(&mut rng);
    let entry = hostile_entry(&mut rng);
    let non_composable = rng.next_u32() & 7 == 0;
    Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32)
                .with_count(8)
                .with_output_byte_range(0..16),
            BufferDecl::read("input", 1, DataType::U32).with_count(8),
            BufferDecl::read_write("rw", 2, DataType::U32).with_count(8),
            BufferDecl::read("bytes_in", 3, DataType::Bytes).with_count(16),
            BufferDecl::read_write("bytes_out", 4, DataType::Bytes).with_count(16),
            BufferDecl::read("counts", 5, DataType::U32).with_count(8),
            BufferDecl::workgroup("scratch", 4, DataType::U32),
            BufferDecl::read("extra_a", 6, dtype_a).with_count(1),
            BufferDecl::read("extra_b", 7, dtype_b).with_count(1),
        ],
        [1 + rng.range(3), 1, 1],
        entry,
    )
    .with_non_composable_with_self(non_composable)
}

fn hostile_entry(rng: &mut Rng) -> Vec<Node> {
    let names = ["", "x", "alpha", "snow_雪", "nul\0name"];
    let buffers = ["out", "input", "rw", "bytes_out"];
    let mut nodes = Vec::new();
    let node_count = 1 + rng.range(6) as usize;
    for index in 0..node_count {
        let name = rng.pick_str(&names);
        let value = hostile_value_expr(rng, index);
        match index % 5 {
            0 => nodes.push(Node::let_bind(name, value)),
            1 => nodes.push(Node::Assign {
                name: name.into(),
                value,
            }),
            2 => {
                let buffer = rng.pick_str(&buffers);
                nodes.push(Node::Store {
                    buffer: buffer.into(),
                    index: hostile_value_expr(rng, index + 3),
                    value: hostile_value_expr(rng, index + 5),
                });
            }
            3 if index + 1 < node_count => {
                nodes.push(Node::If {
                    cond: hostile_value_expr(rng, index + 7),
                    then: vec![Node::let_bind("t", hostile_value_expr(rng, index + 11))],
                    otherwise: vec![Node::let_bind("f", hostile_value_expr(rng, index + 13))],
                });
            }
            _ => nodes.push(Node::let_bind(name, value)),
        }
    }
    nodes.push(Node::Return);
    nodes
}

fn hostile_value_expr(rng: &mut Rng, salt: usize) -> Expr {
    match (rng.next_u32() as usize + salt) % 12 {
        0 => Expr::LitU32(rng.next_u32()),
        1 => Expr::LitI32(rng.next_u32() as i32),
        2 => Expr::LitBool(rng.next_u32() & 1 == 0),
        3 => Expr::LitF32(hostile_f32_bits(rng)),
        4 => Expr::var(rng.pick_str(&["", "x", "alpha", "snow_雪", "nul\0name"])),
        5 => Expr::buf_len("input"),
        6 => Expr::gid_x(),
        7 => Expr::Cast {
            target: hostile_datatype(rng),
            value: Box::new(Expr::LitU32(rng.next_u32())),
        },
        8 => {
            let op = hostile_bin_op(rng);
            Expr::BinOp {
                op,
                left: Box::new(Expr::LitU32(rng.next_u32())),
                right: Box::new(Expr::LitU32(rng.next_u32())),
            }
        }
        9 => {
            let op = hostile_un_op(rng);
            Expr::UnOp {
                op,
                operand: Box::new(Expr::LitU32(rng.next_u32())),
            }
        }
        10 => Expr::Select {
            cond: Box::new(Expr::LitBool(rng.next_u32() & 1 == 0)),
            true_val: Box::new(Expr::LitU32(rng.next_u32())),
            false_val: Box::new(Expr::LitU32(rng.next_u32())),
        },
        _ => Expr::Fma {
            a: Box::new(Expr::LitF32(hostile_f32_bits(rng))),
            b: Box::new(Expr::LitF32(hostile_f32_bits(rng))),
            c: Box::new(Expr::LitF32(hostile_f32_bits(rng))),
        },
    }
}

fn hostile_f32_bits(rng: &mut Rng) -> f32 {
    let bits = match rng.range(10) {
        0 => 0x0000_0000,
        1 => 0x0000_0001,
        2 => 0x007f_ffff,
        3 => f32::MIN_POSITIVE.to_bits(),
        4 => f32::MIN.to_bits(),
        5 => f32::MAX.to_bits(),
        6 => 0x7fc0_0000,
        7 => 0.0f32.to_bits(),
        8 => 1.0f32.to_bits(),
        _ => {
            let raw = rng.next_u32();
            if f32::from_bits(raw).is_nan() || raw == (-0.0f32).to_bits() {
                2.0f32.to_bits()
            } else {
                raw
            }
        }
    };
    f32::from_bits(bits)
}

fn hostile_bin_op(rng: &mut Rng) -> BinOp {
    let opaque = BinOp::Opaque(ExtensionBinOpId(rng.next_u32() | 0x8000_0000));
    let ops = [
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
        BinOp::WrappingAdd,
        BinOp::WrappingSub,
        BinOp::MulHigh,
        opaque,
    ];
    ops[rng.range(ops.len() as u32) as usize].clone()
}

fn hostile_un_op(rng: &mut Rng) -> UnOp {
    let opaque = UnOp::Opaque(ExtensionUnOpId(rng.next_u32() | 0x8000_0000));
    let ops = [
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
        UnOp::InverseSqrt,
        UnOp::Reciprocal,
        opaque,
    ];
    ops[rng.range(ops.len() as u32) as usize].clone()
}

fn hostile_buffer_datatype(rng: &mut Rng) -> DataType {
    let ops = [
        DataType::U8,
        DataType::U16,
        DataType::U32,
        DataType::I8,
        DataType::I16,
        DataType::I32,
        DataType::I64,
        DataType::U64,
        DataType::Vec2U32,
        DataType::Vec4U32,
        DataType::Bool,
        DataType::Bytes,
        DataType::Array {
            element_size: 1 + rng.range(64) as usize,
        },
        DataType::F16,
        DataType::BF16,
        DataType::F32,
        DataType::F64,
        DataType::Tensor,
    ];
    ops[rng.range(ops.len() as u32) as usize].clone()
}

fn hostile_datatype(rng: &mut Rng) -> DataType {
    let leaf = match rng.range(16) {
        0 => DataType::U8,
        1 => DataType::U16,
        2 => DataType::U32,
        3 => DataType::I8,
        4 => DataType::I16,
        5 => DataType::I32,
        6 => DataType::I64,
        7 => DataType::U64,
        8 => DataType::Vec2U32,
        9 => DataType::Vec4U32,
        10 => DataType::Bool,
        11 => DataType::Bytes,
        12 => DataType::Array {
            element_size: 1 + rng.range(64) as usize,
        },
        13 => DataType::F16,
        14 => DataType::BF16,
        _ => DataType::F32,
    };
    match rng.range(8) {
        0 => leaf,
        1 => DataType::F64,
        2 => DataType::Tensor,
        3 => DataType::Handle(TypeId(rng.next_u32())),
        4 => DataType::Vec {
            element: Box::new(leaf),
            count: rng.range(5) as u8,
        },
        5 => DataType::TensorShaped {
            element: Box::new(leaf),
            shape: smallvec![1 + rng.range(8), 1 + rng.range(8)],
        },
        6 => DataType::SparseBsr {
            element: Box::new(DataType::I4),
            block_rows: 1 + rng.range(16),
            block_cols: 1 + rng.range(16),
        },
        7 => DataType::Quantized {
            storage: Box::new(DataType::I8),
            scale: QuantizationScale::PerGroup {
                group_size: 32 + rng.range(96),
            },
            zero_point: QuantizationZeroPoint::PerChannel { axis: rng.range(4) },
        },
        _ => DataType::Opaque(ExtensionDataTypeId(rng.next_u32() | 0x8000_0000)),
    }
}

#[test]
fn sweep_wire_roundtrip_exercises_spec_atomic_tags_in_programs() {
    let mut assertions = 0usize;
    for case in 0..128usize {
        let mut rng = Rng::new(0xA71C_0000 ^ case as u64);
        let opaque = AtomicOp::Opaque(ExtensionAtomicOpId(rng.next_u32() | 0x8000_0000));
        let ops = [
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
            opaque,
        ];
        let op = ops[rng.range(ops.len() as u32) as usize];
        let program = Program::wrapped(
            vec![BufferDecl::read_write("rw", 0, DataType::U32).with_count(4)],
            [1, 1, 1],
            vec![
                Node::let_bind(
                    "atomic",
                    Expr::Atomic {
                        op,
                        buffer: "rw".into(),
                        index: Box::new(Expr::LitU32(rng.next_u32())),
                        expected: (case & 1 == 0).then(|| Box::new(Expr::LitU32(rng.next_u32()))),
                        value: Box::new(Expr::LitU32(rng.next_u32())),
                        ordering: vyre_foundation::ir::MemoryOrdering::SeqCst,
                    },
                ),
                Node::Return,
            ],
        );
        let encoded = program
            .to_wire()
            .unwrap_or_else(|error| panic!("Fix: atomic wire case {case} must encode: {error}"));
        let decoded = Program::from_wire(&encoded)
            .unwrap_or_else(|error| panic!("Fix: atomic wire case {case} must decode: {error}"));
        let reencoded = decoded
            .to_wire()
            .unwrap_or_else(|error| panic!("Fix: atomic wire case {case} must re-encode: {error}"));
        assert_eq!(
            reencoded, encoded,
            "Fix: atomic wire case {case} byte drift"
        );
        let roundtrip_again = Program::from_wire(&reencoded)
            .unwrap_or_else(|error| panic!("Fix: atomic wire case {case} must re-decode: {error}"))
            .to_wire()
            .unwrap_or_else(|error| {
                panic!("Fix: atomic wire case {case} must triple-encode: {error}")
            });
        assert_eq!(
            roundtrip_again, encoded,
            "Fix: atomic wire case {case} triple-encode drift"
        );
        assertions += 2;
    }
    assert_eq!(assertions, 128 * 2);
}
