use vyre_macros::vyre_pass;

#[vyre_pass(
    name = "bad_boundary",
    requires = [],
    invalidates = [],
    boundary_class = "leaky"
)]
pub struct BadBoundary;

fn main() {}
