use vyre_macros::vyre_pass;

#[vyre_pass(
    name = "bad.duplicate.invalidates",
    requires = [],
    invalidates = ["alias", "alias"]
)]
pub struct BadDuplicateInvalidates;

fn main() {}
