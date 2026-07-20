use vyre_macros::vyre_pass;

#[vyre_pass(
    name = "bad_unknown",
    requires = [],
    invalidates = [],
    surprise = "not allowed"
)]
pub struct BadUnknown;

fn main() {}
