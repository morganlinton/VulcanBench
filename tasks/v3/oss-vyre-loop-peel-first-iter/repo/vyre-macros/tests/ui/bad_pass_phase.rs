use vyre_macros::vyre_pass;

#[vyre_pass(
    name = "bad_phase",
    requires = [],
    invalidates = [],
    phase = "definitely_not_a_phase"
)]
pub struct BadPhase;

fn main() {}
