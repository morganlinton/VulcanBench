use vyre_macros::vyre_pass;

#[vyre_pass(name = "bad_stateful_pass", requires = [], invalidates = [])]
pub struct BadStatefulPass {
    state: u32,
}

fn main() {}
