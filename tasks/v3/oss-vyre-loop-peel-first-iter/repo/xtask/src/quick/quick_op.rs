#[derive(Clone)]
pub(crate) struct QuickOp {
    pub(crate) id: &'static str,
    pub(crate) arity: usize,
    pub(crate) laws: &'static [crate::quick::QuickLaw],
    pub(crate) eval: fn(&[u32]) -> u32,
}
