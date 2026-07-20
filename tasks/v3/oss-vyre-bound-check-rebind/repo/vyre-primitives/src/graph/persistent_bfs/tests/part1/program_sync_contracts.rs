use super::*;

#[test]
fn large_persistent_bfs_program_uses_grid_sync_parallel_steps() {
    let program = persistent_bfs(
        ProgramGraphShape::new(257, 256),
        "frontier_in",
        "frontier_out",
        0xFFFF_FFFF,
        3,
    );

    assert_eq!(program.workgroup_size, PERSISTENT_BFS_WORKGROUP_SIZE);
    assert!(
        contains_grid_sync(program.entry()),
        "Fix: large persistent_bfs must use grid synchronization between parallel expansion passes."
    );
    assert_eq!(
        count_grid_sync(program.entry()),
        6,
        "Fix: three large persistent-BFS iterations require one seed fence, one snapshot fence per parallel expansion, and one inter-iteration fence between expansion passes."
    );
    let changed = program
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "changed")
        .expect("Fix: large persistent_bfs must declare the changed buffer.");
    assert_eq!(
        changed.count(),
        3,
        "Fix: large persistent_bfs needs changed[1..=2] as ping-pong active scratch to skip converged edge scans."
    );
    assert_eq!(
        changed.output_byte_range(),
        None,
        "Fix: grid-sync split must carry all changed scratch words between segments; callers decode changed[0]."
    );
    let entry_debug = format!("{:?}", program.entry());
    assert!(
        entry_debug.contains("grid_iter_0_extra_changed_old")
            && entry_debug.contains("grid_iter_1_extra_changed_old")
            && entry_debug.contains("grid_iter_2_extra_changed_old"),
        "Fix: every large persistent-BFS wave must set the next active scratch word when it discovers new nodes."
    );
    assert!(
        !contains_loop_named(program.entry(), "src"),
        "Fix: large persistent_bfs must not scan every source node from one lane."
    );
}

fn contains_grid_sync(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Barrier {
            ordering: MemoryOrdering::GridSync,
        } => true,
        Node::If {
            then, otherwise, ..
        } => contains_grid_sync(then) || contains_grid_sync(otherwise),
        Node::Loop { body, .. } | Node::Block(body) => contains_grid_sync(body),
        Node::Region { body, .. } => contains_grid_sync(body),
        _ => false,
    })
}

fn count_grid_sync(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            Node::Barrier {
                ordering: MemoryOrdering::GridSync,
            } => 1,
            Node::If {
                then, otherwise, ..
            } => count_grid_sync(then) + count_grid_sync(otherwise),
            Node::Loop { body, .. } | Node::Block(body) => count_grid_sync(body),
            Node::Region { body, .. } => count_grid_sync(body),
            _ => 0,
        })
        .sum()
}

fn contains_loop_named(nodes: &[Node], needle: &str) -> bool {
    nodes.iter().any(|node| match node {
        Node::Loop { var, body, .. } => var.as_str() == needle || contains_loop_named(body, needle),
        Node::If {
            then, otherwise, ..
        } => contains_loop_named(then, needle) || contains_loop_named(otherwise, needle),
        Node::Block(body) => contains_loop_named(body, needle),
        Node::Region { body, .. } => contains_loop_named(body, needle),
        _ => false,
    })
}
