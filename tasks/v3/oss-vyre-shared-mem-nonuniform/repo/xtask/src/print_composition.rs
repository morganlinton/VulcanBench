//! `cargo_full run --bin xtask -- print-composition <op_id>`  -  walk an op's Program
//! body and print its Region decomposition chain.
//!
//! Spec: `docs/region-chain.md` (Phase J).
//!
//! Walks the registered op inventories (`vyre_libs::harness`,
//! `vyre_primitives::harness`, and `vyre_intrinsics::harness`), finds the
//! matching OpEntry, calls `build()`, and recurses into every
//! `Node::Region` in the Program's entry body extracting the generator
//! name. Output is an indented tree showing how a public op
//! decomposes through its composition chain down to the leaves.

use std::process;

use vyre::ir::{Node, Program};

/// Entry point for the `print-composition` subcommand.
pub(crate) fn run(args: &[String]) {
    let op_id = match args.get(2) {
        Some(s) => s.as_str(),
        None => {
            eprintln!("Fix: usage: cargo_full run --bin xtask -- print-composition <op_id>");
            process::exit(1);
        }
    };

    let program = match resolve_program(op_id) {
        Some(p) => p,
        None => {
            eprintln!(
                "Fix: op id '{op_id}' not found in any inventory registry. \
                 Known id prefixes: vyre-intrinsics::hardware::*, \
                 vyre-primitives::*, \
                 vyre-libs::math::*, vyre-libs::math::atomic::*, \
                 vyre-libs::hash::*, vyre-libs::logical::*, vyre-libs::nn::*, \
                 vyre-libs::matching::*."
            );
            process::exit(1);
        }
    };

    println!("{op_id}  [{} top-level Nodes]", program.entry().len());
    for node in program.entry() {
        print_node(node, 1);
    }
}

fn resolve_program(op_id: &str) -> Option<Program> {
    for entry in vyre_libs::harness::all_entries() {
        if entry.id == op_id {
            return Some((entry.build)());
        }
    }
    for entry in vyre_primitives::harness::all_entries() {
        if entry.id == op_id {
            return Some((entry.build)());
        }
    }
    for entry in vyre_intrinsics::harness::all_entries() {
        if entry.id == op_id {
            return Some((entry.build)());
        }
    }
    None
}

fn print_node(node: &Node, depth: usize) {
    let indent = "  ".repeat(depth);
    match node {
        Node::Region {
            generator, body, ..
        } => {
            println!("{indent}├─ {}  [{} Nodes]", generator.as_str(), body.len());
            for child in body.iter() {
                print_node(child, depth + 1);
            }
        }
        Node::Block(children) => {
            for child in children {
                print_node(child, depth);
            }
        }
        Node::If {
            then, otherwise, ..
        } => {
            for child in then {
                print_node(child, depth);
            }
            for child in otherwise {
                print_node(child, depth);
            }
        }
        Node::Loop { body, .. } => {
            for child in body {
                print_node(child, depth);
            }
        }
        _ => {
            // Leaf node (Let, Assign, Store, Barrier, Return, …)  -  no
            // composition below. We only print at Region boundaries and
            // composite control-flow structures.
        }
    }
}
