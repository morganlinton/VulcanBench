use vyre_macros::vyre_ast_registry;

vyre_ast_registry! {
    Expr {
        Literal(u32),
        Literal { value: u32 },
    }
}

fn main() {}
