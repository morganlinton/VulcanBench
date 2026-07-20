use vyre_foundation::ir::UnOp;

/// Sanitize a binding name into a valid PTX identifier suffix. Empty
/// names fall back to `slot{N}` so every binding still gets a unique
/// suffix.
pub(super) fn sanitize_param_name(name: &str, slot: u32) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        format!("slot{slot}")
    } else {
        cleaned
    }
}

pub(super) fn unop_name(op: &UnOp) -> &'static str {
    match op {
        UnOp::Negate => "negate",
        UnOp::LogicalNot => "logical_not",
        UnOp::BitNot => "bit_not",
        UnOp::Abs => "abs",
        UnOp::Sqrt => "sqrt",
        UnOp::InverseSqrt => "inverse_sqrt",
        UnOp::Reciprocal => "reciprocal",
        UnOp::Exp => "exp",
        UnOp::Log => "log",
        UnOp::Exp2 => "exp2",
        UnOp::Log2 => "log2",
        UnOp::Sin => "sin",
        UnOp::Cos => "cos",
        UnOp::Tanh => "tanh",
        UnOp::Floor => "floor",
        UnOp::Ceil => "ceil",
        UnOp::Round => "round",
        UnOp::Trunc => "trunc",
        UnOp::Popcount => "popcount",
        UnOp::Clz => "clz",
        UnOp::Ctz => "ctz",
        UnOp::ReverseBits => "reverse_bits",
        UnOp::IsNan => "is_nan",
        UnOp::IsInf => "is_inf",
        UnOp::IsFinite => "is_finite",
        _ => "unknown",
    }
}
