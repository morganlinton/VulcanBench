use crate::ir::{BinOp, Expr, UnOp};
use crate::optimizer::passes::fusion_cse::cse::expr_key::{ExprId, ExprKey};
use crate::optimizer::passes::fusion_cse::cse::{is_commutative, CseCtx, TypeKey};
use smallvec::SmallVec;

impl CseCtx {
    #[inline]
    pub(crate) fn intern_expr(&mut self, expr: &Expr) -> ExprId {
        // Soundness (S19): pointer-keyed cache removed. See the
        // matching comment in `impl_csectx.rs::expr`  -  `Box<Expr>`
        // addresses are reused as Cow::Owned rewrites churn through
        // them, so caching by raw pointer returned stale ExprIds and
        // CSE merged semantically distinct expressions. The
        // `deduplication` map below still amortises intern cost
        // through structural key lookup.
        self.intern_calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let key = match expr {
            Expr::LitU32(value) => ExprKey::LitU32(*value),
            Expr::LitI32(value) => ExprKey::LitI32(*value),
            Expr::LitF32(value) => ExprKey::LitF32(value.to_bits()),
            Expr::LitBool(value) => ExprKey::LitBool(*value),
            Expr::Var(name) => ExprKey::Var(name.clone()),
            Expr::Load { buffer, index } => ExprKey::Load(buffer.clone(), self.intern_expr(index)),
            Expr::BufLen { buffer } => ExprKey::BufLen(buffer.clone()),
            Expr::InvocationId { axis } => ExprKey::InvocationId(*axis),
            Expr::WorkgroupId { axis } => ExprKey::WorkgroupId(*axis),
            Expr::LocalId { axis } => ExprKey::LocalId(*axis),
            Expr::BinOp { op, left, right } => {
                let mut l = self.intern_expr(left);
                let mut r = self.intern_expr(right);
                if is_commutative(op) && r < l {
                    std::mem::swap(&mut l, &mut r);
                }
                match op {
                    BinOp::Opaque(id) => ExprKey::BinOpOpaque(id.as_u32(), l, r),
                    _ => match bin_op_key(*op) {
                        Some(key) => ExprKey::BinOp(key, l, r),
                        None => self.unique_uncached_key(),
                    },
                }
            }
            Expr::UnOp { op, operand } => {
                let operand_id = self.intern_expr(operand);
                match op {
                    UnOp::Opaque(id) => ExprKey::UnOpOpaque(id.as_u32(), operand_id),
                    _ => match un_op_key(op) {
                        Some(key) => ExprKey::UnOp(key, operand_id),
                        None => self.unique_uncached_key(),
                    },
                }
            }
            Expr::Call { op_id, args } => ExprKey::Call(
                op_id.clone(),
                args.iter()
                    .map(|arg| self.intern_expr(arg))
                    .collect::<SmallVec<[ExprId; 4]>>(),
            ),
            Expr::Fma { a, b, c } => ExprKey::Fma(
                self.intern_expr(a),
                self.intern_expr(b),
                self.intern_expr(c),
            ),
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => ExprKey::Select(
                self.intern_expr(cond),
                self.intern_expr(true_val),
                self.intern_expr(false_val),
            ),
            Expr::Cast { target, value } => {
                ExprKey::Cast(TypeKey::from(target), self.intern_expr(value))
            }
            Expr::Atomic { .. } => ExprKey::Atomic,
            &Expr::SubgroupBallot { .. }
            | &Expr::SubgroupShuffle { .. }
            | &Expr::SubgroupReduce { .. } => {
                let id = self.subgroup_counter;
                self.subgroup_counter = self.subgroup_counter.wrapping_add(1);
                ExprKey::Subgroup(id)
            }
            Expr::SubgroupLocalId => ExprKey::SubgroupLocalId,
            Expr::SubgroupSize => ExprKey::SubgroupSize,
            Expr::Opaque(extension) => {
                ExprKey::Opaque(extension.extension_kind(), extension.stable_fingerprint())
            }
        };

        if let Some(&id) = self.deduplication.get(&key) {
            id
        } else {
            let id = ExprId(u32::try_from(self.arena.len()).map_or(u32::MAX, |value| value));
            self.arena.push(key.clone());
            self.deduplication.insert(key, id);
            id
        }
    }

    #[inline]
    fn unique_uncached_key(&mut self) -> ExprKey {
        let id = self.subgroup_counter;
        self.subgroup_counter = self.subgroup_counter.wrapping_add(1);
        ExprKey::Subgroup(id)
    }
}

#[inline]
fn bin_op_key(op: BinOp) -> Option<u8> {
    // Soundness: every concrete BinOp variant gets a distinct tag so
    // CSE never merges semantically distinct ops. The previous
    // `_ => 255` fallback collapsed WrappingSub / RotateLeft /
    // RotateRight / MulHigh onto a single tag  -  silent CSE soundness
    // gap waiting on an adversarial input. `BinOp::Opaque` is keyed
    // separately via `ExprKey::BinOpOpaque` (carries the extension u32
    // id) so the integer table below covers only built-in variants.
    match op {
        BinOp::Add => Some(0),
        BinOp::Sub => Some(1),
        BinOp::Mul => Some(2),
        BinOp::Div => Some(3),
        BinOp::Mod => Some(4),
        BinOp::BitAnd => Some(5),
        BinOp::BitOr => Some(6),
        BinOp::BitXor => Some(7),
        BinOp::Shl => Some(8),
        BinOp::Shr => Some(9),
        BinOp::Eq => Some(10),
        BinOp::Ne => Some(11),
        BinOp::Lt => Some(12),
        BinOp::Gt => Some(13),
        BinOp::Le => Some(14),
        BinOp::Ge => Some(15),
        BinOp::And => Some(16),
        BinOp::Or => Some(17),
        BinOp::AbsDiff => Some(18),
        BinOp::Min => Some(19),
        BinOp::Max => Some(20),
        BinOp::SaturatingAdd => Some(21),
        BinOp::SaturatingSub => Some(22),
        BinOp::SaturatingMul => Some(23),
        BinOp::Shuffle => Some(24),
        BinOp::Ballot => Some(25),
        BinOp::WaveReduce => Some(26),
        BinOp::WaveBroadcast => Some(27),
        BinOp::WrappingAdd => Some(28),
        BinOp::WrappingSub => Some(29),
        BinOp::RotateLeft => Some(30),
        BinOp::RotateRight => Some(31),
        BinOp::MulHigh => Some(32),
        // Opaque is handled via ExprKey::BinOpOpaque before this
        // function is called; reaching this arm is a soundness bug.
        _ => None,
    }
}

#[inline]
fn un_op_key(op: &UnOp) -> Option<u8> {
    // Same soundness contract as bin_op_key: every concrete UnOp
    // variant gets a distinct tag. `UnOp::Opaque` is keyed separately
    // via `ExprKey::UnOpOpaque`, so the table covers only built-ins.
    match op {
        UnOp::Negate => Some(0),
        UnOp::BitNot => Some(1),
        UnOp::LogicalNot => Some(2),
        UnOp::Popcount => Some(3),
        UnOp::Clz => Some(4),
        UnOp::Ctz => Some(5),
        UnOp::ReverseBits => Some(6),
        UnOp::Sin => Some(7),
        UnOp::Cos => Some(8),
        UnOp::Abs => Some(9),
        UnOp::Sqrt => Some(10),
        UnOp::InverseSqrt => Some(11),
        UnOp::Reciprocal => Some(12),
        UnOp::Floor => Some(13),
        UnOp::Ceil => Some(14),
        UnOp::Round => Some(15),
        UnOp::Trunc => Some(16),
        UnOp::Sign => Some(17),
        UnOp::IsNan => Some(18),
        UnOp::IsInf => Some(19),
        UnOp::IsFinite => Some(20),
        UnOp::Exp => Some(21),
        UnOp::Log => Some(22),
        UnOp::Log2 => Some(23),
        UnOp::Exp2 => Some(24),
        UnOp::Tan => Some(25),
        UnOp::Acos => Some(26),
        UnOp::Asin => Some(27),
        UnOp::Atan => Some(28),
        UnOp::Tanh => Some(29),
        UnOp::Sinh => Some(30),
        UnOp::Cosh => Some(31),
        UnOp::Unpack4Low => Some(32),
        UnOp::Unpack4High => Some(33),
        UnOp::Unpack8Low => Some(34),
        UnOp::Unpack8High => Some(35),
        _ => None,
    }
}
