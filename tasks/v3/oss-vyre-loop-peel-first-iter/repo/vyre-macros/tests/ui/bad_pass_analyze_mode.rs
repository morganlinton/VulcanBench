use vyre_macros::vyre_pass;

#[vyre_pass(
    name = "bad_analyze",
    requires = [],
    invalidates = [],
    analyze = "sometimes"
)]
pub struct BadAnalyze;

fn main() {}
