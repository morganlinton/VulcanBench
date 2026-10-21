use vyre_macros::vyre_pass;

#[vyre_pass(
    name = "bad_preserves_abi",
    requires = [],
    invalidates = [],
    preserves_abi = "yes"
)]
pub struct BadPreservesAbi;

fn main() {}
