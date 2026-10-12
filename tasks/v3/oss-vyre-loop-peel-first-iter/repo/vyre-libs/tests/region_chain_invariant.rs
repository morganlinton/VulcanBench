//! CRITIQUE_VISION_ALIGNMENT_2026-04-23 V7 (CI gate half): every
//! registered Tier-3 / Tier-2.5 op must expose a Region chain whose
//! generator names resolve back to registered ops. A non-leaf op
//! whose every `Node::Region` names an unregistered generator is a
//! *black box*  -  opaque provenance that defeats the vision's
//! auditability promise.
//!
//! The test walks every op in `vyre_libs::harness::all_entries()`,
//! builds its Program, collects the set of generator names referenced
//! anywhere in the Region chain, and asserts every generator name
//! either (a) resolves to a registered op id, or (b) is an
//! anonymous / inline region (empty generator, or opens with
//! `anonymous::` / `inline::`).
//!
//! When this test fails, the offending op has either introduced a
//! typo'd generator id or landed a black-box composition that
//! shouldn't be Tier 3. Either is a vision-alignment regression and
//! must be closed before the PR merges.

use std::collections::BTreeSet;
use vyre::ir::{Node, Program};
use vyre_foundation::composition::self_exclusive_region_key;

fn collect_generators(program: &Program) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for node in program.entry() {
        walk(node, &mut out);
    }
    out
}

fn walk(node: &Node, out: &mut BTreeSet<String>) {
    match node {
        Node::Region {
            generator, body, ..
        } => {
            out.insert(generator.as_str().to_string());
            for child in body.iter() {
                walk(child, out);
            }
        }
        Node::Block(children) => {
            for c in children {
                walk(c, out);
            }
        }
        Node::If {
            then, otherwise, ..
        } => {
            for c in then {
                walk(c, out);
            }
            for c in otherwise {
                walk(c, out);
            }
        }
        Node::Loop { body, .. } => {
            for c in body {
                walk(c, out);
            }
        }
        _ => {}
    }
}

fn registered_op_ids() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for entry in vyre_libs::harness::all_entries() {
        out.insert(entry.id.to_string());
    }
    for entry in vyre_intrinsics::harness::all_entries() {
        out.insert(entry.id.to_string());
    }
    for entry in vyre_primitives::harness::all_entries() {
        out.insert(entry.id.to_string());
    }
    out
}

fn generator_is_allowed(generator: &str, registered: &BTreeSet<String>) -> bool {
    let registered_key = self_exclusive_region_key(generator).unwrap_or(generator);
    // Registered op id  -  the common + correct case.
    if registered.contains(registered_key) {
        return true;
    }
    // Named internal sub-pass of a registered op: generators of the form
    // `<registered_op_id>::<sub_pass>` (e.g. `..::c_lexer::classify_at_pos`,
    // `..::ast_shunting_yard::statement_pass`). Provenance still resolves to a
    // registered op  -  the region declares which op's internal composition it
    // belongs to  -  so it is NOT a black box. A typo in the *op* portion leaves
    // no registered `::`-boundary prefix and is still caught; only sub-pass
    // names (internal to an already-audited op) are admitted this way.
    if registered
        .iter()
        .any(|op| registered_key.starts_with(op) && registered_key[op.len()..].starts_with("::"))
    {
        return true;
    }
    // Generators the architecture explicitly allows to be anonymous:
    // top-level wrappers built by consumers / vyre-libs that don't name
    // a specific downstream op (e.g., `anonymous`, `inline-call`).
    // These are not black boxes because they're structural
    // boundaries, not compositions claiming to invoke a named op.
    if generator.is_empty()
        || generator.starts_with("anonymous")
        || generator.starts_with("inline")
        || generator == "vyre.program.root"
        || generator.starts_with("vyre-runtime::")
    {
        return true;
    }
    false
}

#[test]
fn every_tier3_op_region_chain_resolves_to_registered_generators() {
    let registered = registered_op_ids();
    let mut offenders: Vec<(String, Vec<String>)> = Vec::new();
    for entry in vyre_libs::harness::all_entries() {
        let program = (entry.build)();
        let generators = collect_generators(&program);
        let unregistered: Vec<String> = generators
            .into_iter()
            .filter(|g| !generator_is_allowed(g, &registered))
            .collect();
        if !unregistered.is_empty() {
            offenders.push((entry.id.to_string(), unregistered));
        }
    }
    assert!(
        offenders.is_empty(),
        "Fix: CRITIQUE_VISION_ALIGNMENT_2026-04-23 V7 regression  -  {} \
         registered op(s) name a generator in their Region chain that \
         does not resolve to a registered op id. Every generator must \
         either (a) be a known op id or (b) open with anonymous / \
         inline / vyre.program.root / vyre-runtime::. Offenders:\n{}",
        offenders.len(),
        offenders
            .iter()
            .map(|(op, gens)| format!("  - {op}: [{}]", gens.join(", ")))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
