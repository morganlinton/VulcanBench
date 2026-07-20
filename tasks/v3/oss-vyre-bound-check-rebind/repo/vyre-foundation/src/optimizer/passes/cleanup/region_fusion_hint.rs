//! ROADMAP H5 (foundation_optimizer half)  -  region-fusion hint pass.
//!
//! Detects adjacent `Node::Region` pairs whose generator names
//! match a fusion rule in the built-in table (e.g.
//! `vyre-libs::nn::linear` followed by `vyre-libs::nn::relu`),
//! and rewrites them to a single Region whose generator is the
//! fused-primitive id (`vyre-libs::nn::linear_relu`). The fused
//! Region's body is the concatenation of the two arms; downstream
//! lowering reads the generator id and dispatches the existing
//! fused libs primitive instead of two separate dispatches.
//!
//! Soundness: `Approximate`  -  the rewrite is correct only if the
//! fused primitive is observably equivalent to the two-stage
//! sequence, which is true for the entries in the built-in fusion
//! table (linear+relu, linear+silu) by construction. The table is
//! the contract; new fusion candidates land alongside their fused
//! libs primitive.
//!
//! Cost direction: monotone-down on dispatch count (one less
//! kernel launch per fired fusion) and on global memory traffic
//! (the intermediate buffer between the two stages stays in
//! registers / scratch instead of round-tripping through global).
//!
//! Preserves: every analysis. Invalidates: nothing  -  the fused
//! Region produces the same observable output the two-stage
//! sequence did.

use crate::ir::{Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use std::sync::Arc;

/// Fuse adjacent compatible Regions per the built-in fusion table.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "region_fusion_hint",
    requires = [],
    invalidates = [],
    phase = "cleanup",
    boundary_class = "abi_preserving",
    cost_model_family = "fusion"
)]
pub struct RegionFusionHintPass;

impl RegionFusionHintPass {
    /// Skip programs without a candidate Region pair. Checks both the
    /// top-level entry vec (transform fuses adjacent siblings there too)
    /// and every nested If/Loop/Block/Region body.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Pair-fusion needs at least two Regions; even one Region is
        // necessary. Bit-test cached stats first.
        if !program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_REGION)
        {
            return PassAnalysis::SKIP;
        }
        if entry_has_top_level_candidate_pair(program.entry())
            || program.entry().iter().any(has_candidate_pair)
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree and fuse every matching Region pair.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| fuse_in_body(entry, &mut changed));
        PassResult { program, changed }
    }
}

/// Built-in fusion table. Adding a new fusion candidate requires
/// (1) shipping the fused libs primitive, (2) adding the (left,
/// right, fused) triple here. Order matters: `left` is the upstream
/// region producing the intermediate, `right` consumes it.
const FUSION_RULES: &[(&str, &str, &str)] = &[
    (
        "vyre-libs::nn::linear",
        "vyre-libs::nn::relu",
        "vyre-libs::nn::linear_relu",
    ),
    (
        "vyre-libs::nn::linear",
        "vyre-libs::nn::silu",
        "vyre-libs::nn::linear_silu",
    ),
];

fn lookup_fused(left_gen: &str, right_gen: &str) -> Option<&'static str> {
    FUSION_RULES
        .iter()
        .find(|(l, r, _)| *l == left_gen && *r == right_gen)
        .map(|(_, _, f)| *f)
}

fn fuse_in_body(body: Vec<Node>, changed: &mut bool) -> Vec<Node> {
    let body: Vec<Node> = body.into_iter().map(|n| recurse(n, changed)).collect();
    let mut out: Vec<Node> = Vec::with_capacity(body.len());
    let mut iter = body.into_iter().peekable();
    while let Some(node) = iter.next() {
        let Node::Region {
            generator,
            source_region,
            body,
        } = node
        else {
            out.push(node);
            continue;
        };
        let next_match = matches!(
            iter.peek(),
            Some(Node::Region {
                generator: g, ..
            }) if lookup_fused(generator.as_str(), g.as_str()).is_some()
        );
        if !next_match {
            out.push(Node::Region {
                generator,
                source_region,
                body,
            });
            continue;
        }
        let Some(Node::Region {
            generator: gen_b,
            source_region: _src_b,
            body: body_b,
        }) = iter.next()
        else {
            unreachable!("peek confirmed Region above");
        };
        let fused_gen = lookup_fused(generator.as_str(), gen_b.as_str())
            .unwrap_or_else(|| unreachable!("has_candidate_pair confirmed a fusable pair above"));
        let mut fused_body: Vec<Node> = match Arc::try_unwrap(body) {
            Ok(v) => v,
            Err(arc) => (*arc).clone(),
        };
        let body_b_vec: Vec<Node> = match Arc::try_unwrap(body_b) {
            Ok(v) => v,
            Err(arc) => (*arc).clone(),
        };
        fused_body.extend(body_b_vec);
        *changed = true;
        out.push(Node::Region {
            generator: crate::ir::Ident::from(fused_gen),
            source_region,
            body: Arc::new(fused_body),
        });
    }
    out
}

fn recurse(node: Node, changed: &mut bool) -> Node {
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::If {
            cond,
            then: fuse_in_body(then, changed),
            otherwise: fuse_in_body(otherwise, changed),
        },
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::Loop {
            var,
            from,
            to,
            body: fuse_in_body(body, changed),
        },
        Node::Block(body) => Node::Block(fuse_in_body(body, changed)),
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let body_vec: Vec<Node> = match Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            Node::Region {
                generator,
                source_region,
                body: Arc::new(fuse_in_body(body_vec, changed)),
            }
        }
        other => other,
    }
}

fn entry_has_top_level_candidate_pair(entry: &[Node]) -> bool {
    for window in entry.windows(2) {
        if let (Node::Region { generator: a, .. }, Node::Region { generator: b, .. }) =
            (&window[0], &window[1])
        {
            if lookup_fused(a.as_str(), b.as_str()).is_some() {
                return true;
            }
        }
    }
    false
}

fn has_candidate_pair(node: &Node) -> bool {
    match node {
        Node::Region { body, .. } => {
            let body = body.as_ref();
            for window in body.windows(2) {
                if let (Node::Region { generator: a, .. }, Node::Region { generator: b, .. }) =
                    (&window[0], &window[1])
                {
                    if lookup_fused(a.as_str(), b.as_str()).is_some() {
                        return true;
                    }
                }
            }
            body.iter().any(has_candidate_pair)
        }
        Node::If {
            then, otherwise, ..
        } => {
            for window in then.windows(2) {
                if let (Node::Region { generator: a, .. }, Node::Region { generator: b, .. }) =
                    (&window[0], &window[1])
                {
                    if lookup_fused(a.as_str(), b.as_str()).is_some() {
                        return true;
                    }
                }
            }
            for window in otherwise.windows(2) {
                if let (Node::Region { generator: a, .. }, Node::Region { generator: b, .. }) =
                    (&window[0], &window[1])
                {
                    if lookup_fused(a.as_str(), b.as_str()).is_some() {
                        return true;
                    }
                }
            }
            then.iter().any(has_candidate_pair) || otherwise.iter().any(has_candidate_pair)
        }
        Node::Loop { body, .. } => {
            for window in body.windows(2) {
                if let (Node::Region { generator: a, .. }, Node::Region { generator: b, .. }) =
                    (&window[0], &window[1])
                {
                    if lookup_fused(a.as_str(), b.as_str()).is_some() {
                        return true;
                    }
                }
            }
            body.iter().any(has_candidate_pair)
        }
        Node::Block(body) => {
            for window in body.windows(2) {
                if let (Node::Region { generator: a, .. }, Node::Region { generator: b, .. }) =
                    (&window[0], &window[1])
                {
                    if lookup_fused(a.as_str(), b.as_str()).is_some() {
                        return true;
                    }
                }
            }
            body.iter().any(has_candidate_pair)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Ident, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn region(generator_name: &str, body: Vec<Node>) -> Node {
        Node::Region {
            generator: Ident::from(generator_name),
            source_region: None,
            body: Arc::new(body),
        }
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn region_generators(nodes: &[Node]) -> Vec<String> {
        let mut out = Vec::new();
        fn walk(nodes: &[Node], out: &mut Vec<String>) {
            for n in nodes {
                if let Node::Region {
                    generator, body, ..
                } = n
                {
                    out.push(generator.as_str().to_owned());
                    walk(body.as_ref(), out);
                }
                match n {
                    Node::If {
                        then, otherwise, ..
                    } => {
                        walk(then, out);
                        walk(otherwise, out);
                    }
                    Node::Loop { body, .. } => walk(body, out),
                    Node::Block(body) => walk(body, out),
                    _ => {}
                }
            }
        }
        walk(nodes, &mut out);
        out
    }

    /// Adjacent linear + relu Regions fuse into linear_relu.
    #[test]
    fn fuses_linear_then_relu() {
        let entry = vec![
            region("vyre-libs::nn::linear", vec![Node::Return]),
            region("vyre-libs::nn::relu", vec![Node::Return]),
        ];
        let result = RegionFusionHintPass::transform(program(entry));
        assert!(result.changed, "linear+relu must fuse");
        let gens = region_generators(result.program.entry());
        assert!(
            gens.iter().any(|g| g == "vyre-libs::nn::linear_relu"),
            "generators after fuse: {gens:?}"
        );
    }

    /// Adjacent linear + silu Regions fuse into linear_silu.
    #[test]
    fn fuses_linear_then_silu() {
        let entry = vec![
            region("vyre-libs::nn::linear", vec![Node::Return]),
            region("vyre-libs::nn::silu", vec![Node::Return]),
        ];
        let result = RegionFusionHintPass::transform(program(entry));
        assert!(result.changed, "linear+silu must fuse");
        let gens = region_generators(result.program.entry());
        assert!(
            gens.iter().any(|g| g == "vyre-libs::nn::linear_silu"),
            "generators after fuse: {gens:?}"
        );
    }

    /// relu + linear (wrong order) does not fuse.
    #[test]
    fn keeps_when_order_reversed() {
        let entry = vec![
            region("vyre-libs::nn::relu", vec![Node::Return]),
            region("vyre-libs::nn::linear", vec![Node::Return]),
        ];
        let result = RegionFusionHintPass::transform(program(entry));
        assert!(!result.changed, "wrong order must not fuse");
    }

    /// Two unrelated Regions do not fuse.
    #[test]
    fn keeps_when_no_rule_matches() {
        let entry = vec![
            region("foo::bar", vec![Node::Return]),
            region("baz::qux", vec![Node::Return]),
        ];
        let result = RegionFusionHintPass::transform(program(entry));
        assert!(!result.changed);
    }

    /// `analyze` short-circuits when no candidate pair exists.
    #[test]
    fn analyze_skips_when_no_candidate() {
        let entry = vec![region("foo::bar", vec![Node::Return])];
        let prog = program(entry);
        match crate::optimizer::ProgramPass::analyze(&RegionFusionHintPass, &prog) {
            PassAnalysis::SKIP => {}
            other => panic!("expected SKIP, got {other:?}"),
        }
    }

    /// Nested fusion: linear+relu inside a wrapping Region also fuses.
    #[test]
    fn fuses_inside_wrapping_region() {
        let inner = vec![
            region("vyre-libs::nn::linear", vec![Node::Return]),
            region("vyre-libs::nn::relu", vec![Node::Return]),
        ];
        let entry = vec![region("wrapper", inner)];
        let result = RegionFusionHintPass::transform(program(entry));
        assert!(result.changed);
        let gens = region_generators(result.program.entry());
        assert!(
            gens.iter().any(|g| g == "vyre-libs::nn::linear_relu"),
            "generators: {gens:?}"
        );
    }

    /// Two adjacent fusable Regions sitting at the TOP LEVEL of the
    /// entry Vec must trip analyze. The previous shallow check ran
    /// has_candidate_pair on each entry node individually, never
    /// looking at sibling pairs in the entry vec, so analyze SKIPPed
    /// while transform would happily fuse them.
    #[test]
    fn analyze_runs_for_top_level_adjacent_fusable_pair() {
        let entry = vec![
            region("vyre-libs::nn::linear", vec![Node::Return]),
            region("vyre-libs::nn::relu", vec![Node::Return]),
        ];
        let prog = program(entry);
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&RegionFusionHintPass, &prog),
            PassAnalysis::RUN,
            "top-level adjacent fusable Region pair must trigger analyze"
        );
    }
}
