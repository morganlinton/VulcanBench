use vyre_macros::vyre_pass;

#[vyre_pass(name = "bad_duplicate_requires", requires = ["domtree", "domtree"], invalidates = [])]
pub struct BadDuplicateRequires;

fn main() {}
