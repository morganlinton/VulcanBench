use super::TypeFacts;
use crate::ir::{BinOp, DataType, Expr, Ident, Node, Program};
use rustc_hash::FxHashMap;

pub(super) fn derive(program: &Program) -> TypeFacts {
    let mut ctx = TypeFactCtx {
        facts: TypeFacts::default(),
        buffer_types: program
            .buffers()
            .iter()
            .map(|buffer| (Ident::from(buffer.name()), buffer.element().clone()))
            .collect(),
        expr_key: Vec::with_capacity(64),
    };
    ctx.infer_nodes_types(program.entry());
    ctx.facts
}

struct TypeFactCtx {
    facts: TypeFacts,
    buffer_types: FxHashMap<Ident, DataType>,
    expr_key: Vec<u8>,
}

impl TypeFactCtx {
    fn infer_nodes_types(&mut self, nodes: &[Node]) {
        let mut stack = Vec::with_capacity(nodes.len());
        stack.extend(nodes.iter().rev());
        while let Some(node) = stack.pop() {
            match node {
                Node::Let { name, value } | Node::Assign { name, value } => {
                    if let Some(ty) = self.expr_type(value) {
                        self.facts.var_types.insert(name.clone(), ty);
                    }
                }
                Node::Store { index, value, .. } => {
                    self.record_expr_type(index);
                    self.record_expr_type(value);
                }
                Node::If {
                    cond,
                    then,
                    otherwise,
                } => {
                    self.record_expr_type(cond);
                    stack.extend(otherwise.iter().rev());
                    stack.extend(then.iter().rev());
                }
                Node::Loop { from, to, body, .. } => {
                    self.record_expr_type(from);
                    self.record_expr_type(to);
                    stack.extend(body.iter().rev());
                }
                Node::Block(nodes) => {
                    stack.extend(nodes.iter().rev());
                }
                Node::Region { body, .. } => {
                    stack.extend(body.iter().rev());
                }
                Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
                    self.record_expr_type(offset);
                    self.record_expr_type(size);
                }
                Node::Trap { address, .. } => {
                    self.record_expr_type(address);
                }
                Node::Return
                | Node::Barrier { .. }
                | Node::IndirectDispatch { .. }
                | Node::AllReduce { .. }
                | Node::AllGather { .. }
                | Node::ReduceScatter { .. }
                | Node::Broadcast { .. }
                | Node::AsyncWait { .. }
                | Node::Resume { .. }
                | Node::Opaque(_) => {}
            }
        }
    }

    fn record_expr_type(&mut self, expr: &Expr) {
        drop(self.expr_type(expr));
    }

    fn expr_type(&mut self, expr: &Expr) -> Option<DataType> {
        let ty = match expr {
            Expr::LitI32(_) => Some(DataType::I32),
            Expr::LitF32(_) => Some(DataType::F32),
            Expr::LitBool(_) => Some(DataType::Bool),
            Expr::Var(name) => self.facts.var_types.get(name).cloned(),
            Expr::Load { buffer, index } => {
                self.record_expr_type(index);
                self.buffer_types.get(buffer).cloned()
            }
            Expr::LitU32(_)
            | Expr::BufLen { .. }
            | Expr::InvocationId { .. }
            | Expr::WorkgroupId { .. }
            | Expr::LocalId { .. }
            | Expr::SubgroupLocalId
            | Expr::SubgroupSize => Some(DataType::U32),
            Expr::SubgroupBallot { cond } => {
                self.record_expr_type(cond);
                Some(DataType::U32)
            }
            Expr::Atomic {
                index,
                expected,
                value,
                ..
            } => {
                self.record_expr_type(index);
                if let Some(expected) = expected {
                    self.record_expr_type(expected);
                }
                self.record_expr_type(value);
                Some(DataType::U32)
            }
            Expr::Cast { target, value } => {
                self.record_expr_type(value);
                Some(target.clone())
            }
            Expr::BinOp { op, left, right } => {
                let left_ty = self.expr_type(left);
                let right_ty = self.expr_type(right);
                match op {
                    BinOp::Eq
                    | BinOp::Ne
                    | BinOp::Lt
                    | BinOp::Gt
                    | BinOp::Le
                    | BinOp::Ge
                    | BinOp::And
                    | BinOp::Or => Some(DataType::Bool),
                    BinOp::Mod
                    | BinOp::Shl
                    | BinOp::Shr
                    | BinOp::RotateLeft
                    | BinOp::RotateRight
                    | BinOp::Ballot
                    | BinOp::WaveReduce
                    | BinOp::WaveBroadcast => Some(DataType::U32),
                    _ => left_ty.or(right_ty),
                }
            }
            Expr::UnOp { operand, .. } => self.expr_type(operand),
            Expr::Fma { a, b, c } => {
                self.record_expr_type(a);
                self.record_expr_type(b);
                self.record_expr_type(c);
                Some(DataType::F32)
            }
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                self.record_expr_type(cond);
                let true_ty = self.expr_type(true_val);
                let false_ty = self.expr_type(false_val);
                true_ty.filter(|ty| Some(ty) == false_ty.as_ref())
            }
            Expr::Call { args, .. } => {
                for arg in args {
                    self.record_expr_type(arg);
                }
                None
            }
            Expr::SubgroupShuffle { value, lane } => {
                let value_ty = self.expr_type(value);
                self.record_expr_type(lane);
                value_ty
            }
            Expr::SubgroupReduce { value, .. } => self.expr_type(value),
            Expr::Opaque(extension) => extension.result_type(),
        };
        if let Some(ty) = &ty {
            let key = self.expr_structural_key(expr);
            self.facts.expr_types.insert(key, ty.clone());
        }
        ty
    }

    fn expr_structural_key(&mut self, expr: &Expr) -> u64 {
        self.expr_key.clear();
        if let Err(error) = crate::serial::wire::encode::put_expr(&mut self.expr_key, expr) {
            self.expr_key.clear();
            self.expr_key
                .extend_from_slice(b"VYRE-TYPE-FACT-EXPR-WIRE-ERROR\0");
            self.expr_key.extend_from_slice(error.as_bytes());
        }
        let digest = blake3::hash(&self.expr_key);
        u64::from_le_bytes([
            digest.as_bytes()[0],
            digest.as_bytes()[1],
            digest.as_bytes()[2],
            digest.as_bytes()[3],
            digest.as_bytes()[4],
            digest.as_bytes()[5],
            digest.as_bytes()[6],
            digest.as_bytes()[7],
        ])
    }
}
