pub(crate) fn law_violation(
    op: &crate::quick::quick_op::QuickOp,
    law: crate::quick::quick_law::QuickLaw,
) -> Option<String> {
    const VALUES: &[u32] = &[0, 1, 2, 3, 0x7FFF_FFFF, 0x8000_0000, u32::MAX];
    match law {
        crate::quick::quick_law::QuickLaw::Commutative => {
            crate::quick::binary_law::binary_law(op, VALUES, |op, a, b, _| {
                (op.eval)(&[a, b]) == (op.eval)(&[b, a])
            })
        }
        crate::quick::quick_law::QuickLaw::Associative => {
            crate::quick::binary_law::binary_law(op, VALUES, |op, a, b, c| {
                let left = (op.eval)(&[(op.eval)(&[a, b]), c]);
                let right = (op.eval)(&[a, (op.eval)(&[b, c])]);
                left == right
            })
        }
        crate::quick::quick_law::QuickLaw::Identity(element) => {
            crate::quick::binary_law::binary_law(op, VALUES, |op, a, _, _| {
                (op.eval)(&[a, element]) == a && (op.eval)(&[element, a]) == a
            })
        }
        crate::quick::quick_law::QuickLaw::SelfInverse(result) => {
            crate::quick::binary_law::binary_law(op, VALUES, |op, a, _, _| {
                (op.eval)(&[a, a]) == result
            })
        }
        crate::quick::quick_law::QuickLaw::Idempotent => {
            crate::quick::binary_law::binary_law(op, VALUES, |op, a, _, _| (op.eval)(&[a, a]) == a)
        }
        crate::quick::quick_law::QuickLaw::Involution => {
            if op.arity != 1 {
                return None;
            }
            for a in VALUES {
                if (op.eval)(&[(op.eval)(&[*a])]) != *a {
                    return Some(format!("({a})"));
                }
            }
            None
        }
    }
}
