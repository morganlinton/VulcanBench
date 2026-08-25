//! Tier 10 - End-to-end CLI surface for `dominator_tree`.
//!
//! Reads a tiny edge-list from stdin (or defaults to a diamond fixture) and
//! prints the immediate-dominator array computed by the exact CPU oracle.
//!
//! Example:
//!   echo -e "0 1\n0 2\n1 3\n2 3" | cargo run --example dominator_tree_e2e --features graph,cpu-parity

use std::io::{self, BufRead};

fn main() {
    println!("vyre-primitives dominator_tree e2e example");

    let stdin = io::stdin();
    let mut edges: Vec<(u32, u32)> = Vec::new();
    let mut max_node: u32 = 0;

    for line in stdin.lock().lines() {
        let line = line.expect("Fix: provide readable UTF-8 edge-list lines on stdin.");
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() == 2 {
            let u: u32 = parts[0]
                .parse()
                .expect("Fix: use a decimal u32 source node id.");
            let v: u32 = parts[1]
                .parse()
                .expect("Fix: use a decimal u32 target node id.");
            edges.push((u, v));
            max_node = max_node.max(u).max(v);
        }
    }

    let node_count = if edges.is_empty() {
        // Default diamond fixture.
        edges = vec![(0, 1), (0, 2), (1, 3), (2, 3)];
        4
    } else {
        max_node + 1
    };

    println!("Nodes: {node_count}, Edges: {}", edges.len());

    #[cfg(feature = "cpu-parity")]
    {
        use vyre_primitives::graph::dominator_tree::cpu_ref;
        let idoms = cpu_ref(node_count, 0, &edges);
        for (v, idom) in idoms.iter().enumerate() {
            match idom {
                Some(p) => println!("idom[{v}] = {p}"),
                None => println!("idom[{v}] = NONE (unreachable)"),
            }
        }
    }
    #[cfg(not(feature = "cpu-parity"))]
    {
        println!("Enable feature 'cpu-parity' to run the CPU oracle.");
    }
}
