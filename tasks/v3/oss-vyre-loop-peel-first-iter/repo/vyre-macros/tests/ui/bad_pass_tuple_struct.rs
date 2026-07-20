use vyre_macros::vyre_pass;

#[vyre_pass(name = "bad_tuple_struct", requires = [], invalidates = [])]
pub struct BadTupleStruct(u32);

fn main() {}
