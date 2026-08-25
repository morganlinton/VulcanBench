use vyre_macros::define_op;

define_op! {
    id = "primitive.bad.duplicate",
    id = "primitive.bad.late_override",
    dialect = "primitive.bad",
    category = A,
    inputs = ["u32"],
    outputs = ["u32"],
    laws = [],
    program = panic!(),
}

fn main() {}
