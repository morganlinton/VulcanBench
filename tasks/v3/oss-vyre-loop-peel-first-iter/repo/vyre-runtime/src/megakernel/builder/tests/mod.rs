use super::*;

fn async_load_bindings(nodes: &[Node], out: &mut Vec<(String, String, String)>) {
    for node in nodes {
        match node {
            Node::AsyncLoad {
                source,
                destination,
                tag,
                ..
            } => out.push((
                source.as_str().to_string(),
                destination.as_str().to_string(),
                tag.as_str().to_string(),
            )),
            Node::If {
                then, otherwise, ..
            } => {
                async_load_bindings(then, out);
                async_load_bindings(otherwise, out);
            }
            Node::Loop { body, .. } | Node::Block(body) => async_load_bindings(body, out),
            Node::Region { body, .. } => async_load_bindings(body, out),
            _ => {}
        }
    }
}

fn contains_let_named(nodes: &[Node], needle: &str) -> bool {
    for node in nodes {
        match node {
            Node::Let { name, .. } if name.as_str() == needle => return true,
            Node::If {
                then, otherwise, ..
            } => {
                if contains_let_named(then, needle) || contains_let_named(otherwise, needle) {
                    return true;
                }
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                if contains_let_named(body, needle) {
                    return true;
                }
            }
            Node::Region { body, .. } => {
                if contains_let_named(body, needle) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn collect_let_names_preorder<'a>(nodes: &'a [Node], out: &mut Vec<&'a str>) {
    for node in nodes {
        match node {
            Node::Let { name, .. } => out.push(name.as_str()),
            Node::If {
                then, otherwise, ..
            } => {
                collect_let_names_preorder(then, out);
                collect_let_names_preorder(otherwise, out);
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                collect_let_names_preorder(body, out);
            }
            Node::Region { body, .. } => collect_let_names_preorder(body, out),
            _ => {}
        }
    }
}

#[test]
fn io_polling_uses_capability_tables_not_fake_resource_names() {
    let program = build_program_sharded_with_io_polling(64, &[]);
    let mut bindings = Vec::new();
    async_load_bindings(&program.entry, &mut bindings);
    assert_eq!(bindings.len(), 1);
    let (source, destination, tag) = &bindings[0];
    assert_eq!(source, "io_source_capability_table");
    assert_eq!(destination, "io_destination_capability_table");
    assert_eq!(tag, "io_queue_dma");
    assert_ne!(source, "ssd_weights");
    assert_ne!(destination, "vram_cache");
}

#[test]
fn priority_builder_declares_explicit_ring_slots() {
    let program = build_program_priority_slots(64, 512, &[]);
    let ring = program
        .buffer("ring_buffer")
        .expect("Fix: priority megakernel must declare the ring buffer");
    assert_eq!(ring.count, 512 * SLOT_WORDS);
}

#[test]
fn direct_megakernel_defers_tenant_loads_until_status_is_published() {
    let body = persistent_body(64, &[]);
    let top_level_lets = body
        .iter()
        .filter_map(|node| match node {
            Node::Let { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
            top_level_lets,
            vec!["shutdown_flag", "lane_id", "slot_base"],
            "Fix: the persistent megakernel prologue must not load tenant metadata before proving the slot is claimable."
        );

    let mut names = Vec::new();
    collect_let_names_preorder(&body, &mut names);
    let observed = names
        .iter()
        .position(|name| *name == "observed_status")
        .expect("Fix: status load must gate the claim path");
    let tenant_mask = names
        .iter()
        .position(|name| *name == "tenant_mask")
        .expect("Fix: tenant authorization must still exist for published slots");
    assert!(
            observed < tenant_mask,
            "Fix: idle megakernel slots must skip tenant table loads; observed_status appears at {observed}, tenant_mask at {tenant_mask}."
        );
}

#[test]
fn empty_sharded_shared_builder_reuses_cached_program_arc() {
    let first = build_program_sharded_slots_shared(64, 256, &[]);
    let second = build_program_sharded_slots_shared(64, 256, &[]);

    assert!(
            Arc::ptr_eq(&first, &second),
            "Fix: empty megakernel template bootstraps must reuse the cached Arc<Program> instead of cloning the Program before compile."
        );
}

#[test]
fn empty_sharded_once_shared_builder_reuses_cached_program_arc() {
    let first = build_program_sharded_once_slots_shared(64, 256, &[]);
    let second = build_program_sharded_once_slots_shared(64, 256, &[]);

    assert!(
            Arc::ptr_eq(&first, &second),
            "Fix: one-shot megakernel dispatchers must reuse the cached Arc<Program> instead of rebuilding or cloning the Program on the hot path."
        );
}

#[test]
fn custom_opcode_is_optimized_inside_whole_megakernel_program() {
    let handler = OpcodeHandler {
        opcode: 99,
        body: vec![Node::let_bind(
            "__vyre_dead_custom_handler_tmp",
            Expr::add(Expr::u32(1), Expr::u32(2)),
        )],
    };

    let program = build_program_sharded_once_slots(64, 64, &[handler]);

    assert!(
            !contains_let_named(program.entry(), "__vyre_dead_custom_handler_tmp"),
            "Fix: megakernel builders must optimize the assembled Program, including custom opcode handlers, before backend lowering."
        );
}

#[test]
fn self_loading_miss_handler_program_contains_load_miss_bindings() {
    let program = build_program_with_self_loading_miss_handler(64, 256, &[]);
    let mut names = Vec::new();
    collect_let_names_preorder(program.entry(), &mut names);
    assert!(
        names.iter().any(|n| *n == "resource_id"),
        "Fix: self-loading miss handler must bind resource_id (the \
         opaque consumer-defined identifier the IO queue carries)"
    );
    assert!(
        names.iter().any(|n| *n == "found_io_slot"),
        "Fix: self-loading miss handler must scan for an empty IO slot"
    );
    assert!(
        names.iter().any(|n| *n == "poll_done"),
        "Fix: self-loading miss handler must poll for DMA completion"
    );
}

#[test]
fn self_loading_miss_handler_does_not_include_async_load_nodes() {
    let program = build_program_with_self_loading_miss_handler(64, 256, &[]);
    let mut bindings = Vec::new();
    async_load_bindings(program.entry(), &mut bindings);
    assert_eq!(
        bindings.len(),
        0,
        "Fix: self-loading miss handler must not introduce AsyncLoad nodes; it writes to the IO queue and polls instead."
    );
}
