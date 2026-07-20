use vyre_macros::vyre_pass;

#[vyre_pass(
    name = "bad.duplicate.phase",
    requires = [],
    invalidates = [],
    phase = "dataflow",
    phase = "cleanup"
)]
pub struct BadDuplicatePhase;

fn main() {}
