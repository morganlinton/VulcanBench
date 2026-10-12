#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuickLaw {
    Commutative,
    Associative,
    Identity(u32),
    SelfInverse(u32),
    Idempotent,
    Involution,
}
