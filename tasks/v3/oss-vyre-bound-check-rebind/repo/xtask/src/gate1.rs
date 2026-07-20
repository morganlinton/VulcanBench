//! `cargo_full run --bin xtask -- gate1`  -  Gate 1 complexity-budget enforcement.
//!
//! Spec: `docs/primitives-tier.md` (the LEGO substrate enforcement loop).
//!
//! For every registered op (vyre-libs + vyre-intrinsics inventories):
//!
//! 1. Build the op's `Program`.
//! 2. Walk the entry-body Node tree:
//!    - `total_nodes`  -  recursive node count.
//!    - `loops`  -  count of `Node::Loop`.
//!    - `composed_nodes`  -  count of nodes that live inside a
//!      `Node::Region { source_region: Some(_), .. }` (i.e. the Region
//!      was constructed by composing another registered op rather than
//!      being an anonymous local wrapper).
//! 3. Pass if EITHER:
//!    - Under raw budget: `loops <= 4 AND total_nodes <= 200`, OR
//!    - Adequate composition: `composed_nodes / total_nodes >= 0.6`.
//!
//! On fail, the diagnostic lists the inline sub-blocks (the loops /
//! large Block / If branches that aren't wrapped in a child Region)
//! so an author can see exactly what should have been a primitive
//! call.
//!
//! Exit code 0 = all ops pass. Exit code 1 = ≥ 1 op fails (CI signal).

use std::process;

use vyre::ir::{Node, Program};

const LOOP_BUDGET: usize = 4;
const NODE_BUDGET: usize = 200;
const COMPOSED_FRACTION_THRESHOLD: f64 = 0.6;

/// Per-op gate-1 verdict.
#[derive(Debug)]
struct Verdict {
    op_id: String,
    total_nodes: usize,
    loops: usize,
    composed_nodes: usize,
    inline_hot_spots: Vec<String>,
}

impl Verdict {
    fn passes(&self) -> bool {
        if self.loops <= LOOP_BUDGET && self.total_nodes <= NODE_BUDGET {
            return true;
        }
        if self.total_nodes == 0 {
            return true;
        }
        let composed_fraction = self.composed_nodes as f64 / self.total_nodes as f64;
        composed_fraction >= COMPOSED_FRACTION_THRESHOLD
    }

    fn composed_fraction_pct(&self) -> f64 {
        if self.total_nodes == 0 {
            return 100.0;
        }
        100.0 * self.composed_nodes as f64 / self.total_nodes as f64
    }
}

/// Entry point for the `gate1` subcommand.
pub(crate) fn run(_args: &[String]) {
    let mut verdicts: Vec<Verdict> = Vec::new();
    for entry in vyre_libs::harness::all_entries() {
        verdicts.push(verdict_for(entry.id, &(entry.build)()));
    }
    for entry in vyre_intrinsics::harness::all_entries() {
        verdicts.push(verdict_for(entry.id, &(entry.build)()));
    }
    for entry in vyre_primitives::harness::all_entries() {
        verdicts.push(verdict_for(entry.id, &(entry.build)()));
    }

    verdicts.sort_by(|a, b| a.op_id.cmp(&b.op_id));

    let total = verdicts.len();
    let failures: Vec<&Verdict> = verdicts.iter().filter(|v| !v.passes()).collect();

    println!("=== Gate 1  -  complexity budget ===");
    println!(
        "Budget: loops <= {LOOP_BUDGET}  AND  nodes <= {NODE_BUDGET}, \
         OR composed_fraction >= {:.0}%",
        COMPOSED_FRACTION_THRESHOLD * 100.0
    );
    println!("Ops audited: {total}");
    println!("Failures:    {}", failures.len());
    println!();

    for v in &verdicts {
        let mark = if v.passes() { "✓" } else { "✗" };
        println!(
            "{mark}  {:<60}  loops={:<3} nodes={:<5} composed={:>5.1}%",
            v.op_id,
            v.loops,
            v.total_nodes,
            v.composed_fraction_pct()
        );
    }

    if !failures.is_empty() {
        println!();
        println!("=== Failure detail ===");
        for v in &failures {
            println!();
            println!("✗ {}", v.op_id);
            println!(
                "  loops={} (budget {LOOP_BUDGET}), nodes={} (budget {NODE_BUDGET}), composed={:.1}% (need {:.0}%)",
                v.loops,
                v.total_nodes,
                v.composed_fraction_pct(),
                COMPOSED_FRACTION_THRESHOLD * 100.0
            );
            if v.inline_hot_spots.is_empty() {
                println!("  Fix: factor inline work into a Tier 2.5 primitive call (region::wrap_child).");
            } else {
                println!("  Inline hot spots that should be Tier 2.5 primitive calls:");
                for spot in &v.inline_hot_spots {
                    println!("    - {spot}");
                }
                println!(
                    "  Fix: extract each hot spot into a `vyre-primitives::{{math,nn,hash,matching,parsing,text,graph}}::<op>` primitive (one crate, feature-gated per domain), \
                     then call it from this op via `region::wrap_child(<primitive_op_id>, ...)`. \
                     See docs/primitives-tier.md."
                );
            }
        }
        process::exit(1);
    }

    println!();
    println!("All {total} ops within budget. Gate 1 ✓");
}

fn verdict_for(op_id: &'static str, program: &Program) -> Verdict {
    let mut state = WalkState::default();
    for node in program.entry() {
        walk(node, false, &mut state);
    }
    Verdict {
        op_id: op_id.to_string(),
        total_nodes: state.total_nodes,
        loops: state.loops,
        composed_nodes: state.composed_nodes,
        inline_hot_spots: state.inline_hot_spots,
    }
}

#[derive(Default)]
struct WalkState {
    total_nodes: usize,
    loops: usize,
    composed_nodes: usize,
    inline_hot_spots: Vec<String>,
}

/// Walk a node, counting it and recursing.
///
/// `inside_composed_region` propagates downward: once we enter a
/// `Region { source_region: Some(_), .. }`, every node beneath counts
/// toward `composed_nodes`. Anonymous regions (`source_region: None`)
/// do NOT promote their children to composed  -  they're local wrappers,
/// not composition.
fn walk(node: &Node, inside_composed_region: bool, state: &mut WalkState) {
    state.total_nodes += 1;
    if inside_composed_region {
        state.composed_nodes += 1;
    }

    match node {
        Node::Region {
            source_region,
            body,
            generator,
        } => {
            let now_composed = inside_composed_region || source_region.is_some();
            for child in body.iter() {
                walk(child, now_composed, state);
            }
            // Hot spot: an anonymous Region with > 50 inline nodes  -
            // either factor the body into a registered primitive or
            // mark the source_region.
            if !inside_composed_region && source_region.is_none() && body.len() > 50 {
                state.inline_hot_spots.push(format!(
                    "anonymous Region `{}` with {} top-level body nodes",
                    generator.as_str(),
                    body.len()
                ));
            }
        }
        Node::Loop { body, .. } => {
            state.loops += 1;
            for child in body {
                walk(child, inside_composed_region, state);
            }
            if !inside_composed_region {
                state.inline_hot_spots.push(format!(
                    "inline `Node::Loop` with {} body nodes",
                    body.len()
                ));
            }
        }
        Node::Block(children) => {
            for child in children {
                walk(child, inside_composed_region, state);
            }
        }
        Node::If {
            then, otherwise, ..
        } => {
            for child in then {
                walk(child, inside_composed_region, state);
            }
            for child in otherwise {
                walk(child, inside_composed_region, state);
            }
        }
        // Leaves  -  Let, Assign, Store, Return, Barrier, IndirectDispatch,
        // AsyncLoad, AsyncWait, Opaque  -  count themselves and stop.
        _ => {}
    }
}
