use vyre_macros::vyre_pass;

#[vyre_pass(requires = [], invalidates = [])]
pub struct BadMissingName;

fn main() {}
