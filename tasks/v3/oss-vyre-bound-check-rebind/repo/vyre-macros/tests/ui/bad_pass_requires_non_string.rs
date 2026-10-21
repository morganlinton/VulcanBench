use vyre_macros::vyre_pass;

#[vyre_pass(
    name = "bad_requires",
    requires = [1],
    invalidates = []
)]
pub struct BadRequires;

fn main() {}
