#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]
use crate::quick::QuickLaw;

#[derive(Clone, Copy)]
pub(crate) struct QuickMutation {
    pub(crate) id: &'static str,
    pub(crate) from: &'static str,
    pub(crate) eval: Option<fn(&[u32]) -> u32>,
    pub(crate) laws: Option<&'static [QuickLaw]>,
}
