use super::*;

#[test]
fn store_u32_body_produces_single_atomic_store() {
    let nodes = store_u32_body();
    assert_eq!(nodes.len(), 1, "STORE_U32 must emit exactly one IR node");
}

#[test]
fn atomic_add_body_produces_single_let_bind() {
    let nodes = atomic_add_body();
    assert_eq!(nodes.len(), 1, "ATOMIC_ADD must emit exactly one IR node");
}

#[test]
fn shutdown_body_produces_single_atomic_exchange() {
    let nodes = shutdown_body();
    assert_eq!(nodes.len(), 1, "SHUTDOWN must emit exactly one IR node");
}

#[test]
fn printf_body_produces_reservation_plus_guarded_writes() {
    let nodes = printf_body();
    assert_eq!(
        nodes.len(),
        2,
        "PRINTF must emit reservation (let_bind) + guarded write block (if_then)"
    );
}

#[test]
fn load_u32_body_produces_single_store() {
    let nodes = load_u32_body();
    assert_eq!(nodes.len(), 1, "LOAD_U32 must emit exactly one IR node");
}

#[test]
fn compare_swap_body_produces_cas_plus_observable_write() {
    let nodes = compare_swap_body();
    assert_eq!(
        nodes.len(),
        2,
        "COMPARE_SWAP must emit CAS let_bind + observable store"
    );
}

#[test]
fn memcpy_body_produces_loop() {
    let nodes = memcpy_body();
    assert_eq!(nodes.len(), 1, "MEMCPY must emit exactly one loop node");
}

#[test]
fn batch_fence_body_produces_epoch_bump_plus_observable() {
    let nodes = batch_fence_body();
    assert_eq!(
        nodes.len(),
        2,
        "BATCH_FENCE must emit epoch atomic_add + observable store"
    );
}

#[test]
fn opcode_if_wraps_body_in_conditional() {
    let body = vec![Node::let_bind("x", Expr::u32(42))];
    let node = opcode_if(99, body);
    // The result must be an If node (not the raw body).
    // We can't destructure Node directly, but we can confirm it's
    // a single node wrapping our body.
    assert!(
        format!("{node:?}").contains("99"),
        "opcode_if must embed the discriminant"
    );
}

#[test]
fn claimed_slot_bindings_loads_four_variables() {
    let bindings = claimed_slot_bindings();
    assert_eq!(
        bindings.len(),
        4,
        "claimed_slot_bindings must bind opcode, arg0, arg1, arg2"
    );
}

#[test]
fn claimed_slot_body_includes_done_counter_and_status_write() {
    let body = claimed_slot_body(&[]);
    // Must contain at least: 4 bindings + dispatch block +
    // packed_slot handler + done_count atomic_add + DONE store
    assert!(
            body.len() >= 7,
            "claimed_slot_body must include bindings + dispatch + packed + done_count + DONE store, got {}",
            body.len()
        );
}

#[test]
fn claimed_slot_body_includes_custom_handlers() {
    let custom = OpcodeHandler {
        opcode: 200,
        body: vec![Node::let_bind("custom_var_unique", Expr::u32(0))],
    };
    let body = claimed_slot_body(&[custom]);
    let debug = format!("{body:?}");
    assert!(
        debug.contains("200"),
        "claimed_slot_body must embed the custom opcode discriminant in the dispatch tree"
    );
    assert!(
        debug.contains("custom_var_unique"),
        "claimed_slot_body must embed the custom handler body in the dispatch tree"
    );
}

#[test]
fn packed_slot_body_is_nonempty() {
    let body = packed_slot_body(&[]);
    assert!(
        !body.is_empty(),
        "packed_slot_body must emit the packed opcode dispatch root even with no custom opcodes"
    );
    let debug = format!("{body:?}");
    assert!(
        debug.contains("packed_opcode_count"),
        "packed_slot_body must emit the packed opcode-count decode root even with no custom opcodes"
    );
}

#[test]
fn dispatch_opcode_body_includes_all_builtin_opcodes() {
    let body = dispatch_opcode_body(&[]);
    // 1 metrics guard + 8 builtin opcode handlers = 9
    assert_eq!(
        body.len(),
        9,
        "dispatch_opcode_body must include metrics guard + 8 builtin handlers"
    );
}

#[test]
fn dispatch_opcode_body_extends_with_custom_handlers() {
    let custom = OpcodeHandler {
        opcode: 201,
        body: vec![Node::let_bind("ext", Expr::u32(1))],
    };
    let body = dispatch_opcode_body(&[custom]);
    assert_eq!(
        body.len(),
        10,
        "dispatch_opcode_body must include 9 builtins + 1 custom"
    );
}
