use vyre_macros::define_op;

define_op! {
    id = "test.missing_program",
    dialect = "test",
    category = A,
    inputs = [],
    outputs = [],
    laws = [],
}

fn main() {}
