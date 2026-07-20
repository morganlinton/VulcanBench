use vyre_macros::vyre_ast_registry;

// Just testing if the macro compiles
vyre_ast_registry! {
    Let { name: Ident, value: Expr },
    Assign { name: Ident, value: Expr }
}
fn main() {}
