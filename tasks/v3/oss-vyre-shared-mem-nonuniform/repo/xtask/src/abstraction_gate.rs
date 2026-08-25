//! `cargo_full run --bin xtask -- abstraction-gate`  -  mandatory building-block enforcement.
//!
//! This is the fast local/CI gate that keeps the abstraction thesis
//! mechanical. It verifies that named composition edges point at
//! registered building blocks and that large ops are either small
//! enough to remain leaves or mostly composed from registered children.

use std::collections::BTreeSet;
use std::process;

use vyre::ir::{Node, Program};

const LOOP_BUDGET: usize = 4;
const NODE_BUDGET: usize = 200;
const COMPOSED_FRACTION_THRESHOLD: f64 = 0.6;

/// Entry point for the `abstraction-gate` subcommand.
pub(crate) fn run(_args: &[String]) {
    let ops = collect_ops();
    let ids: BTreeSet<String> = ops.iter().map(|op| op.id.clone()).collect();
    let mut failures = BTreeSet::new();

    for op in &ops {
        let mut state = WalkState::default();
        for node in op.program.entry() {
            walk(node, false, &ids, &mut state, &mut failures, &op.id);
        }

        if !within_budget(&state) {
            failures.insert(format!(
                "ABSTRACTION-BUDGET: `{}` has loops={} nodes={} registered-composed={:.1}%. Fix: extract reusable phases into registered Tier 2.5 primitives and wrap them with `region::wrap_child`.",
                op.id,
                state.loops,
                state.total_nodes,
                state.composed_fraction_pct(),
            ));
        }

        if op.id.starts_with("vyre-primitives::")
            && (op.test_inputs_missing || op.expected_output_missing)
        {
            failures.insert(format!(
                "PRIMITIVE-FIXTURE: `{}` must ship standalone test_inputs and expected_output. Fix: add an inventory fixture so the building block can be tested without its parent pipeline.",
                op.id
            ));
        }
    }

    if failures.is_empty() {
        println!(
            "abstraction-gate: {} registered building blocks checked",
            ops.len()
        );
        return;
    }

    eprintln!("abstraction-gate: {} violation(s)", failures.len());
    for failure in &failures {
        eprintln!("  - {failure}");
    }
    process::exit(1);
}

struct OpInfo {
    id: String,
    program: Program,
    test_inputs_missing: bool,
    expected_output_missing: bool,
}

fn collect_ops() -> Vec<OpInfo> {
    let mut ops = Vec::new();
    for entry in vyre_libs::harness::all_entries() {
        ops.push(OpInfo {
            id: entry.id.to_string(),
            program: (entry.build)(),
            test_inputs_missing: entry.test_inputs.is_none(),
            expected_output_missing: entry.expected_output.is_none(),
        });
    }
    for entry in vyre_intrinsics::harness::all_entries() {
        ops.push(OpInfo {
            id: entry.id.to_string(),
            program: (entry.build)(),
            test_inputs_missing: entry.test_inputs.is_none(),
            expected_output_missing: entry.expected_output.is_none(),
        });
    }
    for entry in vyre_primitives::harness::all_entries() {
        ops.push(OpInfo {
            id: entry.id.to_string(),
            program: (entry.build)(),
            test_inputs_missing: entry.test_inputs.is_none(),
            expected_output_missing: entry.expected_output.is_none(),
        });
    }
    ops
}

#[derive(Default)]
struct WalkState {
    total_nodes: usize,
    loops: usize,
    registered_composed_nodes: usize,
}

impl WalkState {
    fn composed_fraction_pct(&self) -> f64 {
        if self.total_nodes == 0 {
            return 100.0;
        }
        100.0 * self.registered_composed_nodes as f64 / self.total_nodes as f64
    }
}

fn within_budget(state: &WalkState) -> bool {
    if state.loops <= LOOP_BUDGET && state.total_nodes <= NODE_BUDGET {
        return true;
    }
    if state.total_nodes == 0 {
        return true;
    }
    let composed_fraction = state.registered_composed_nodes as f64 / state.total_nodes as f64;
    composed_fraction >= COMPOSED_FRACTION_THRESHOLD
}

fn walk(
    node: &Node,
    inside_registered_child: bool,
    ids: &BTreeSet<String>,
    state: &mut WalkState,
    failures: &mut BTreeSet<String>,
    owner_id: &str,
) {
    state.total_nodes += 1;
    if inside_registered_child {
        state.registered_composed_nodes += 1;
    }

    match node {
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let generator_name = generator.as_str();
            let is_registered_child = source_region.is_some() && ids.contains(generator_name);
            if source_region.is_some() && generator_name.contains("::") && !is_registered_child {
                failures.insert(format!(
                    "UNREGISTERED-CHILD: `{owner_id}` wraps `{generator_name}` as a child region, but no registered OpEntry exists for that building block. Fix: register it in the appropriate Tier 2.5/Tier 3 harness or stop marking it as a child."
                ));
            }
            if let Some(parent) = source_region {
                if parent.name.contains("::") && !ids.contains(parent.name.as_str()) {
                    failures.insert(format!(
                        "UNKNOWN-PARENT: `{owner_id}` child `{generator_name}` cites source_region `{}` which is not a registered op id.",
                        parent.name
                    ));
                }
            }
            for child in body.iter() {
                walk(
                    child,
                    inside_registered_child || is_registered_child,
                    ids,
                    state,
                    failures,
                    owner_id,
                );
            }
        }
        Node::Loop { body, .. } => {
            state.loops += 1;
            for child in body {
                walk(
                    child,
                    inside_registered_child,
                    ids,
                    state,
                    failures,
                    owner_id,
                );
            }
        }
        Node::Block(children) => {
            for child in children {
                walk(
                    child,
                    inside_registered_child,
                    ids,
                    state,
                    failures,
                    owner_id,
                );
            }
        }
        Node::If {
            then, otherwise, ..
        } => {
            for child in then {
                walk(
                    child,
                    inside_registered_child,
                    ids,
                    state,
                    failures,
                    owner_id,
                );
            }
            for child in otherwise {
                walk(
                    child,
                    inside_registered_child,
                    ids,
                    state,
                    failures,
                    owner_id,
                );
            }
        }
        _ => {}
    }
}
