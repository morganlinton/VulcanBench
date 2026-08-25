use vyre_macros::vyre_pass;

#[vyre_pass(name = "bad_named_struct", requires = [], invalidates = [])]
pub struct BadNamedStruct {
    state: u32,
}

fn main() {}
