pub(crate) fn infer_quick_laws(
    op: &crate::quick::quick_op::QuickOp,
) -> Vec<crate::quick::quick_law::QuickLaw> {
    let candidates: &[crate::quick::quick_law::QuickLaw] = if op.arity == 2 {
        &[
            crate::quick::quick_law::QuickLaw::Commutative,
            crate::quick::quick_law::QuickLaw::Associative,
            crate::quick::quick_law::QuickLaw::Idempotent,
            crate::quick::quick_law::QuickLaw::Identity(0),
            crate::quick::quick_law::QuickLaw::Identity(1),
            crate::quick::quick_law::QuickLaw::Identity(u32::MAX),
            crate::quick::quick_law::QuickLaw::SelfInverse(0),
            crate::quick::quick_law::QuickLaw::SelfInverse(1),
        ]
    } else {
        &[crate::quick::quick_law::QuickLaw::Involution]
    };

    candidates
        .iter()
        .copied()
        .filter(|law| crate::quick::law_violation::law_violation(op, *law).is_none())
        .collect()
}
