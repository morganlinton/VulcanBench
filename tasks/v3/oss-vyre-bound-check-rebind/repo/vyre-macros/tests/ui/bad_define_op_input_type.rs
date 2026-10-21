use vyre_macros::define_op;

define_op! {
    id = "primitive.bad.input",
    dialect = "primitive.bad",
    category = A,
    inputs = [1],
    outputs = ["u32"],
    laws = [],
    program = panic!(),
}

fn main() {}
