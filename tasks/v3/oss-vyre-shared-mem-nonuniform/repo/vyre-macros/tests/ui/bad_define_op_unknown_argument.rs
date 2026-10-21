use vyre_macros::define_op;

define_op! {
    id = "primitive.bad.unknown",
    dialect = "primitive.bad",
    category = A,
    inputs = ["u32"],
    outputs = ["u32"],
    laws = [],
    program = panic!(),
    mystery = true,
}

fn main() {}
