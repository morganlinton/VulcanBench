//! Per-arm alpha-renaming for cross-program fusion.
//!
//! Two arm strategies share this walker (see `super::fuse::ArmNamespace`):
//!
//! * **Isolated** arms are independent programs whose temp counters overlap.
//!   Every arm-local name is prefixed `__vyre_fuse_a{arm}_…` so reused names
//!   cannot collide. Built with [`ArmRenamer::isolated`] (empty shared set).
//!
//! * **Shared** arms are sub-programs of one rule that share a global temp
//!   namespace. A value declared in one arm and consumed in another
//!   (`let __cmp_N = load(__quant_flag_…)` produced by the quantifier,
//!   `Var(__cmp_N)` in the consumer) must keep ONE name in ONE scope. Built
//!   with [`ArmRenamer::shared`]: the *cross-arm* names (free in some arm)
//!   are left intact so the decl→use link survives, while the genuinely
//!   arm-local names (e.g. a primitive's internal `let acc`, reused verbatim
//!   across unrelated primitives) are still prefixed so they cannot collide
//!   once the arms are spliced into one flat scope.

use std::sync::Arc;

use rustc_hash::FxHashSet;

use crate::ir::{Expr, Ident, Node, Program};

/// Prefix used to mark a name as arm-qualified by fusion.
const FUSION_ARM_PREFIX: &str = "__vyre_fuse_a";

/// Which arm-local identifiers an [`ArmRenamer`] rewrites with its arm prefix.
#[derive(Clone, Copy)]
pub(super) enum RenameScope<'a> {
    /// Isolated fusion: every name is arm-local; rename all of them.
    All,
    /// Shared-namespace merge: rename ONLY names that are declared in ≥2 arms
    /// of the batch (a genuine collision, e.g. a primitive's internal `acc`
    /// reused verbatim in two primitives). A name declared in exactly one arm
    /// — including a quantifier flag readback `let __cmp_N = …` consumed by a
    /// different arm — is globally unique and must keep ONE name so the
    /// decl→use link survives. Declaration multiplicity is STABLE across the
    /// pairwise merge chain (a name declared once stays declared once), unlike
    /// a free-variable test, which mislabels `__cmp_N` as arm-local at an
    /// inner merge where its consumer is not yet in the batch.
    MultiplyDeclared(&'a FxHashSet<Ident>),
}

/// Renames an arm's local identifiers with its arm index, under a policy that
/// decides which names are arm-local (see [`RenameScope`]).
pub(super) struct ArmRenamer<'a> {
    arm_idx: usize,
    scope: RenameScope<'a>,
}

impl<'a> ArmRenamer<'a> {
    /// Isolated fusion: rename every arm-local name.
    pub(super) fn isolated(arm_idx: usize) -> Self {
        Self {
            arm_idx,
            scope: RenameScope::All,
        }
    }

    /// Shared-namespace merge: rename only the `multiply_declared` names so the
    /// single-declaration cross-arm values stay linked.
    pub(super) fn shared(arm_idx: usize, multiply_declared: &'a FxHashSet<Ident>) -> Self {
        Self {
            arm_idx,
            scope: RenameScope::MultiplyDeclared(multiply_declared),
        }
    }

    /// Splice one entry node into `out`, renaming arm-local names.
    ///
    /// Shared mode unwraps EXACTLY the arm's synthetic root-region wrapper
    /// (`Node::Region` whose generator is [`Program::ROOT_REGION_GENERATOR`],
    /// which [`Program::wrapped`] auto-adds around any raw top-level body) and
    /// splices its body flat into the one shared rule scope. This is required
    /// because the validator treats EVERY `Region` as a scope frame: a `let`
    /// declared inside a region is restored away when that region exits
    /// (`validate.rs` `PopScope` → `restore_scope`). If each arm kept its own
    /// root-region wrapper, a value declared in one arm (`let __cmp_N = …`)
    /// could not reach its consumer (`Var(__cmp_N)`) in another arm — the
    /// `csrf_missing_token` "undeclared variable" miscompile. Unwrapping the
    /// synthetic wrapper is exact: it is the inverse of `wrap_entry`'s auto-
    /// wrap, carries no provenance, and lands the arm body in the shared scope.
    ///
    /// A *provenance* `Region` (any other generator, e.g. a primitive's
    /// `vyre-primitives::label::resolve_family` group) is preserved verbatim:
    /// its breadcrumb/label semantics are asserted on downstream
    /// (`null_check_sanitized_by_uses_pg_node_tags_not_frontier`), and its
    /// bindings are genuinely arm-local. The fused program re-acquires one
    /// fresh root region via `Program::wrapped`, so the preserved provenance
    /// regions simply nest one level deeper and survive the merge chain.
    ///
    /// Isolated fusion ([`push_alpha_renamed_arm_entry_node`]) does NOT unwrap:
    /// it re-wraps each arm in its own `Block` scope, where reused arm-local
    /// names must stay isolated, not linked.
    pub(super) fn push_entry_node(&self, out: &mut Vec<Node>, node: &Node) {
        if let RenameScope::MultiplyDeclared(_) = self.scope {
            if let Node::Region {
                generator, body, ..
            } = node
            {
                if generator.as_str() == Program::ROOT_REGION_GENERATOR {
                    for child in body.iter() {
                        out.push(self.node(child));
                    }
                    return;
                }
            }
        }
        out.push(self.node(node));
    }

    /// Rename one identifier unless the policy leaves it alone or it is already
    /// arm-qualified (idempotent: re-prefixing a temp from a prior fusion level
    /// would desync it from its matching decl/use — the historical
    /// `__vyre_fuse_a1___vyre_fuse_a0___cmp_5` miscompile).
    fn ident(&self, name: &Ident) -> Ident {
        let rename = match self.scope {
            RenameScope::All => true,
            RenameScope::MultiplyDeclared(set) => set.contains(name),
        };
        if !rename || name.as_str().starts_with(FUSION_ARM_PREFIX) {
            return name.clone();
        }
        Ident::from(format!("{FUSION_ARM_PREFIX}{}_{}", self.arm_idx, name.as_str()))
    }

    fn nodes(&self, nodes: &[Node]) -> Vec<Node> {
        nodes.iter().map(|node| self.node(node)).collect()
    }

    fn node(&self, node: &Node) -> Node {
        match node {
            Node::Let { name, value } => Node::Let {
                name: self.ident(name),
                value: self.expr(value),
            },
            Node::Assign { name, value } => Node::Assign {
                name: self.ident(name),
                value: self.expr(value),
            },
            Node::Store {
                buffer,
                index,
                value,
            } => Node::Store {
                buffer: buffer.clone(),
                index: self.expr(index),
                value: self.expr(value),
            },
            Node::If {
                cond,
                then,
                otherwise,
            } => Node::If {
                cond: self.expr(cond),
                then: self.nodes(then),
                otherwise: self.nodes(otherwise),
            },
            Node::Loop {
                var,
                from,
                to,
                body,
            } => Node::Loop {
                var: self.ident(var),
                from: self.expr(from),
                to: self.expr(to),
                body: self.nodes(body),
            },
            Node::Block(body) => Node::Block(self.nodes(body)),
            Node::Region {
                generator,
                source_region,
                body,
            } => Node::Region {
                generator: generator.clone(),
                source_region: source_region.clone(),
                body: Arc::new(self.nodes(body)),
            },
            Node::AsyncLoad {
                source,
                destination,
                offset,
                size,
                tag,
            } => Node::AsyncLoad {
                source: source.clone(),
                destination: destination.clone(),
                offset: Box::new(self.expr(offset)),
                size: Box::new(self.expr(size)),
                tag: self.ident(tag),
            },
            Node::AsyncStore {
                source,
                destination,
                offset,
                size,
                tag,
            } => Node::AsyncStore {
                source: source.clone(),
                destination: destination.clone(),
                offset: Box::new(self.expr(offset)),
                size: Box::new(self.expr(size)),
                tag: self.ident(tag),
            },
            Node::AsyncWait { tag } => Node::AsyncWait {
                tag: self.ident(tag),
            },
            Node::Trap { address, tag } => Node::Trap {
                address: Box::new(self.expr(address)),
                tag: self.ident(tag),
            },
            Node::Resume { tag } => Node::Resume {
                tag: self.ident(tag),
            },
            Node::IndirectDispatch {
                count_buffer,
                count_offset,
            } => Node::IndirectDispatch {
                count_buffer: count_buffer.clone(),
                count_offset: *count_offset,
            },
            Node::AllReduce { buffer, op, group } => Node::AllReduce {
                buffer: buffer.clone(),
                op: *op,
                group: *group,
            },
            Node::AllGather {
                input,
                output,
                group,
            } => Node::AllGather {
                input: input.clone(),
                output: output.clone(),
                group: *group,
            },
            Node::ReduceScatter {
                input,
                output,
                op,
                group,
            } => Node::ReduceScatter {
                input: input.clone(),
                output: output.clone(),
                op: *op,
                group: *group,
            },
            Node::Broadcast {
                buffer,
                root,
                group,
            } => Node::Broadcast {
                buffer: buffer.clone(),
                root: *root,
                group: *group,
            },
            Node::Return => Node::Return,
            Node::Barrier { ordering } => Node::barrier_with_ordering(*ordering),
            Node::Opaque(extension) => Node::Opaque(Arc::clone(extension)),
        }
    }

    fn expr(&self, expr: &Expr) -> Expr {
        match expr {
            Expr::Var(name) => Expr::Var(self.ident(name)),
            Expr::Load { buffer, index } => Expr::Load {
                buffer: buffer.clone(),
                index: Box::new(self.expr(index)),
            },
            Expr::BufLen { buffer } => Expr::BufLen {
                buffer: buffer.clone(),
            },
            Expr::BinOp { op, left, right } => Expr::BinOp {
                op: *op,
                left: Box::new(self.expr(left)),
                right: Box::new(self.expr(right)),
            },
            Expr::UnOp { op, operand } => Expr::UnOp {
                op: op.clone(),
                operand: Box::new(self.expr(operand)),
            },
            Expr::Call { op_id, args } => Expr::Call {
                op_id: op_id.clone(),
                args: args.iter().map(|arg| self.expr(arg)).collect(),
            },
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => Expr::Select {
                cond: Box::new(self.expr(cond)),
                true_val: Box::new(self.expr(true_val)),
                false_val: Box::new(self.expr(false_val)),
            },
            Expr::Cast { target, value } => Expr::Cast {
                target: target.clone(),
                value: Box::new(self.expr(value)),
            },
            Expr::Fma { a, b, c } => Expr::Fma {
                a: Box::new(self.expr(a)),
                b: Box::new(self.expr(b)),
                c: Box::new(self.expr(c)),
            },
            Expr::Atomic {
                op,
                buffer,
                index,
                expected,
                value,
                ordering,
            } => Expr::Atomic {
                op: *op,
                buffer: buffer.clone(),
                index: Box::new(self.expr(index)),
                expected: expected.as_ref().map(|expr| Box::new(self.expr(expr))),
                value: Box::new(self.expr(value)),
                ordering: *ordering,
            },
            Expr::SubgroupBallot { cond } => Expr::SubgroupBallot {
                cond: Box::new(self.expr(cond)),
            },
            Expr::SubgroupShuffle { value, lane } => Expr::SubgroupShuffle {
                value: Box::new(self.expr(value)),
                lane: Box::new(self.expr(lane)),
            },
            Expr::SubgroupReduce { op, value } => Expr::SubgroupReduce { op: *op,
                value: Box::new(self.expr(value)),
            },
            Expr::LitU32(_)
            | Expr::LitI32(_)
            | Expr::LitF32(_)
            | Expr::LitBool(_)
            | Expr::InvocationId { .. }
            | Expr::WorkgroupId { .. }
            | Expr::LocalId { .. }
            | Expr::SubgroupLocalId
            | Expr::SubgroupSize
            | Expr::Opaque(_) => expr.clone(),
        }
    }
}

/// Isolated-mode entry splice (backwards-compatible free-function form):
/// rename every arm-local name with `arm_idx`.
pub(super) fn push_alpha_renamed_arm_entry_node(out: &mut Vec<Node>, node: &Node, arm_idx: usize) {
    ArmRenamer::isolated(arm_idx).push_entry_node(out, node);
}

/// Names declared (as a `Let` target or `Loop` induction var) in **two or
/// more** arms of the batch. Only these collide once the arms are spliced into
/// one shared scope, so only these are alpha-renamed by [`ArmRenamer::shared`].
///
/// A name declared in exactly one arm is globally unique — including a value
/// produced in one arm and consumed in another (`let __cmp_N = …` / `Var`),
/// which must keep one name. Declaration multiplicity is the stable invariant
/// across the pairwise merge chain: a name declared once stays declared once,
/// so it is never spuriously prefixed at an inner merge.
///
/// `arm_entries` are the original (pre-rename) arm node lists. Counting is
/// per-arm (a name declared twice within one arm still counts as one arm).
pub(super) fn multiply_declared_names(arm_entries: &[&[Node]]) -> FxHashSet<Ident> {
    let mut decl_arms: rustc_hash::FxHashMap<Ident, usize> = rustc_hash::FxHashMap::default();
    for entry in arm_entries {
        let mut declared = FxHashSet::default();
        for node in *entry {
            collect_declared_names(node, &mut declared);
        }
        for name in declared {
            *decl_arms.entry(name).or_insert(0) += 1;
        }
    }
    decl_arms
        .into_iter()
        .filter_map(|(name, arms)| (arms >= 2).then_some(name))
        .collect()
}

/// Names bound within this arm: `Let` targets and `Loop` induction vars.
/// (`Assign` is a mutation of an existing binding, not a new declaration, so
/// a cross-arm assign target is correctly treated as a reference below.)
fn collect_declared_names(node: &Node, out: &mut FxHashSet<Ident>) {
    match node {
        Node::Let { name, .. } => {
            out.insert(name.clone());
        }
        Node::Loop { var, body, .. } => {
            out.insert(var.clone());
            for n in body {
                collect_declared_names(n, out);
            }
        }
        Node::If {
            then, otherwise, ..
        } => {
            for n in then.iter().chain(otherwise.iter()) {
                collect_declared_names(n, out);
            }
        }
        Node::Block(body) => {
            for n in body {
                collect_declared_names(n, out);
            }
        }
        Node::Region { body, .. } => {
            for n in body.iter() {
                collect_declared_names(n, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Ident;

    fn names<const N: usize>(items: [&str; N]) -> FxHashSet<Ident> {
        items.into_iter().map(Ident::from).collect()
    }

    #[test]
    fn bare_temp_is_prefixed_once() {
        let out = ArmRenamer::isolated(0).ident(&Ident::from("cmp_5"));
        assert_eq!(out.as_str(), "__vyre_fuse_a0_cmp_5");
        assert!(out.as_str().starts_with(FUSION_ARM_PREFIX));
    }

    #[test]
    fn already_qualified_temp_is_not_re_prefixed() {
        // A temp produced by a prior/nested fusion level is already globally
        // unique; re-prefixing it at the next level desyncs the use from its
        // decl (the csrf_missing_token miscompile). Renaming it again must be
        // an identity.
        let inner = ArmRenamer::isolated(0).ident(&Ident::from("cmp_5"));
        let outer = ArmRenamer::isolated(1).ident(&inner);
        assert_eq!(
            outer.as_str(),
            inner.as_str(),
            "fusion temp must not accumulate a second arm prefix"
        );
        let outer2 = ArmRenamer::isolated(2).ident(&outer);
        assert_eq!(outer2.as_str(), inner.as_str());
    }

    #[test]
    fn distinct_bare_temps_in_distinct_arms_stay_distinct() {
        let a0 = ArmRenamer::isolated(0).ident(&Ident::from("x"));
        let a1 = ArmRenamer::isolated(1).ident(&Ident::from("x"));
        assert_ne!(a0.as_str(), a1.as_str());
    }

    #[test]
    fn single_declared_name_is_left_unrenamed_in_every_arm() {
        // The csrf invariant: a value declared in exactly one arm and consumed
        // in another must keep ONE name. Such a name is NOT in the
        // multiply-declared set, so it is never prefixed regardless of arm
        // index — the consumer's `Var` matches the producer's `Let`.
        let multiply_declared = names([]); // `__cmp_5` declared in only one arm
        let producer = ArmRenamer::shared(1, &multiply_declared).ident(&Ident::from("__cmp_5"));
        let consumer = ArmRenamer::shared(0, &multiply_declared).ident(&Ident::from("__cmp_5"));
        assert_eq!(producer.as_str(), "__cmp_5");
        assert_eq!(consumer.as_str(), producer.as_str());
    }

    #[test]
    fn multiply_declared_name_is_renamed_in_shared_mode() {
        // A primitive's internal temp (e.g. `acc`, declared verbatim in two
        // primitives) collides once arms splice into one flat scope, so it is
        // prefixed per arm.
        let multiply_declared = names(["acc"]);
        let a0 = ArmRenamer::shared(0, &multiply_declared).ident(&Ident::from("acc"));
        let a1 = ArmRenamer::shared(1, &multiply_declared).ident(&Ident::from("acc"));
        assert_eq!(a0.as_str(), "__vyre_fuse_a0_acc");
        assert_ne!(a0.as_str(), a1.as_str());
    }

    #[test]
    fn multiply_declared_names_counts_arms_not_occurrences() {
        // `acc` declared in BOTH arms -> multiply declared (collision).
        // `__cmp_5` declared in ONE arm (used free in the other) -> single.
        let arm0 = vec![
            Node::Let {
                name: Ident::from("acc"),
                value: Expr::u32(0),
            },
            Node::Let {
                name: Ident::from("__use"),
                value: Expr::Var(Ident::from("__cmp_5")),
            },
        ];
        let arm1 = vec![
            Node::Let {
                name: Ident::from("acc"),
                value: Expr::u32(1),
            },
            Node::Let {
                name: Ident::from("__cmp_5"),
                value: Expr::u32(2),
            },
        ];
        let entries: Vec<&[Node]> = vec![&arm0, &arm1];
        let multi = multiply_declared_names(&entries);
        assert!(
            multi.contains(&Ident::from("acc")),
            "a name declared in two arms must be renamed to avoid collision"
        );
        assert!(
            !multi.contains(&Ident::from("__cmp_5")),
            "a name declared in exactly one arm is unique and must stay linked"
        );
        assert!(!multi.contains(&Ident::from("__use")));
    }
}
