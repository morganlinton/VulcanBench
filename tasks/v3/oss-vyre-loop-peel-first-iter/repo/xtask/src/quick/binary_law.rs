pub(crate) fn binary_law(
    op: &crate::quick::quick_op::QuickOp,
    values: &[u32],
    predicate: impl Fn(&crate::quick::quick_op::QuickOp, u32, u32, u32) -> bool,
) -> Option<String> {
    if op.arity != 2 {
        return None;
    }
    for a in values {
        for b in values {
            for c in values {
                if !predicate(op, *a, *b, *c) {
                    return Some(format!("({a}, {b}, {c})"));
                }
            }
        }
    }
    None
}
