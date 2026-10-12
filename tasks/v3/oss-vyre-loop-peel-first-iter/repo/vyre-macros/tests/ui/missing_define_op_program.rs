use vyre_macros::define_op;

define_op! {
    id = "primitive.bad.missing_program",
    dialect = "primitive.bad",
    category = A,
    inputs = ["u32"],
    outputs = ["u32"],
    laws = [],
}

fn main() {}
