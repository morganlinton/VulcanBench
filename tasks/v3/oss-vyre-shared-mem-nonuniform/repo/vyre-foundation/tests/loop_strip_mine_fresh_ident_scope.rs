//! Oracle-differential probe: loop_strip_mine synthesizes tile/lane loop
//! variables via `fresh_ident`, which checked only names appearing in the LOOP
//! BODY, not names bound in the ENCLOSING scope. So an outer binding named
//! exactly like the generated tile var (`<loopvar>_tile`), unused inside the
//! body, is invisible to the freshness check -- and the generated tile loop var
//! shadows it ("duplicate local binding shadows an outer scope", V008), turning
//! a well-scoped program into one the validator rejects.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::loops::loop_strip_mine::LoopStripMine;
use vyre_reference::value::Value;

/// ```text
/// let i_tile = 5;                       // outer binding, unused in the loop body
/// loop i in 0..16 { store(out, i, 1); } // strip-mineable (trip 16 >= 2*TILE)
/// ```
/// The loop is over `i`, so strip-mine wants a tile var `i_tile` -- colliding
/// with the outer binding. out ends as [1; 16].
fn program_with_outer_name_matching_generated_tile_var() -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(16)],
        [1, 1, 1],
        vec![
            Node::let_bind("i_tile", Expr::u32(5)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(16),
                vec![Node::store("out", Expr::var("i"), Expr::u32(1))],
            ),
        ],
    )
}

#[test]
fn loop_strip_mine_generated_var_does_not_shadow_outer_binding() {
    let program = program_with_outer_name_matching_generated_tile_var();
    let inputs: [Value; 0] = []; // `out` is the only buffer and it is an output.

    let original = vyre_reference::reference_eval(&program, &inputs)
        .expect("original program is well-scoped and must run");
    assert_eq!(
        original,
        vec![Value::from(
            (0..16).flat_map(|_| 1u32.to_le_bytes()).collect::<Vec<u8>>()
        )],
        "out == [1; 16]"
    );

    let transformed = LoopStripMine::transform(program).program;

    // The transformed program must STILL validate. If strip-mine named its tile
    // loop var `i_tile`, it shadows the outer binding and reference_eval errors.
    let after = vyre_reference::reference_eval(&transformed, &inputs).expect(
        "strip-mine must not synthesize a tile/lane var that shadows an outer \
         binding (V008)",
    );
    assert_eq!(
        after, original,
        "strip-mine must preserve observable semantics and scoping"
    );
}
