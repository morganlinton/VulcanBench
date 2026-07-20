use vyre_macros::vyre_pass;

#[vyre_pass(
    name = "bad.duplicate.requires",
    requires = ["domtree", "domtree"],
    invalidates = []
)]
pub struct BadDuplicateRequires;

fn main() {}
