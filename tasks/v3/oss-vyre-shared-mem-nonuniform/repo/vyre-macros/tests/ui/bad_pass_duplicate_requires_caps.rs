use vyre_macros::vyre_pass;

#[vyre_pass(
    name = "bad.duplicate.requires_caps",
    requires = [],
    invalidates = [],
    requires_caps = ["cuda", "cuda"]
)]
pub struct BadDuplicateRequiresCaps;

fn main() {}
