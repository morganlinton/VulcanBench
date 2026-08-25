//! Tier-B TOML rule database for the egraph saturation engine.
//!
//! ROADMAP A6. The Rust-coded `Family::rules` pattern in
//! [`crate::optimizer::eqsat`] keeps every rewrite in source code,
//! which means new equivalences need a recompile. The Tier-B contract
//! says community-contributable rule families should live in TOML so
//! a domain expert can add `(matmul_strassen_2x2 == matmul_2x2)`
//! without touching Rust.
//!
//! This module ships the MVP: a TOML schema for **op-id equivalence
//! rules** plus a `Rule` implementation that unions every pair of
//! e-classes whose enodes name two equivalent op ids. The richer
//! pattern DSL (LHS sub-tree match + RHS substitution) is a
//! follow-up; the equivalence-pair shape covers the most common
//! "these two ops compute the same thing" rewrites that drive
//! algebraic-canonicalisation Families today.
//!
//! ## TOML format
//!
//! ```toml
//! schema = 1
//!
//! [[equivalence]]
//! left = "vyre-libs::math::matmul"
//! right = "vyre-libs::math::matmul_strassen_one_level"
//!
//! [[equivalence]]
//! left = "vyre-primitives::math::elementwise_add"
//! right = "vyre-libs::math::add"
//! ```
//!
//! Each `[[equivalence]]` row tells the rule that whenever both
//! `left` and `right` op ids appear as enodes anywhere in the
//! egraph, their e-classes are equivalent.
//!
//! ## Trait expectations
//!
//! The rule walks `egraph.iter_nodes()` and groups e-classes by the
//! op-id string returned by `OpIdNode::op_id`. Languages that want
//! to consume TOML equivalence rules implement `OpIdNode` in
//! addition to [`crate::optimizer::eqsat::ENodeLang`]. Languages
//! that don't (e.g. pure-arithmetic toy languages from the eqsat
//! tests) don't pay the trait cost.

use std::path::Path;

use rustc_hash::FxHashMap;
use serde::Deserialize;

use crate::optimizer::eqsat::{EClassId, EGraph, ENodeLang, Rule};

/// Languages that participate in TOML equivalence rules expose the
/// op-id string of each enode. The string is the registry id
/// (`vyre-libs::math::matmul`, etc.).
pub trait OpIdNode {
    /// Stable op-id string. `None` for terminal/leaf nodes that don't
    /// carry an op id (literals, builtins)  -  they're skipped by the
    /// equivalence rule.
    fn op_id(&self) -> Option<&str>;
}

/// One TOML-loaded equivalence pair.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EquivalenceRule {
    /// Op-id of the left side.
    pub left: String,
    /// Op-id of the right side.
    pub right: String,
}

/// TOML schema container.
#[derive(Debug, Clone, Deserialize)]
struct RuleFile {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    equivalence: Vec<EquivalenceRule>,
}

/// A loaded TOML equivalence rule set.
///
/// Implements [`Rule`] for any language `L: ENodeLang + OpIdNode`. On
/// each `matches` call, walks the egraph once, groups e-classes by
/// op-id, and emits `(a, b)` pairs for every (left, right) op-id
/// pair where both sides have at least one e-class.
#[derive(Debug, Clone)]
pub struct TomlEquivalenceRules {
    name: &'static str,
    rules: Vec<EquivalenceRule>,
}

impl TomlEquivalenceRules {
    /// Construct an empty rule set with the given debug name.
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            rules: Vec::new(),
        }
    }

    /// Load rule pairs from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns the underlying `std::io::Error` on read failure or a
    /// `toml::de::Error` projected through `std::io::Error::other` on
    /// parse failure. Schema-version mismatch returns
    /// `std::io::ErrorKind::InvalidData`.
    pub fn load(name: &'static str, path: &Path) -> std::io::Result<Self> {
        let text = read_eqsat_toml_bounded(path)?;
        Self::from_toml_str(name, &text)
    }

    /// Parse rule pairs from an in-memory TOML string.
    ///
    /// # Errors
    ///
    /// Returns `std::io::ErrorKind::InvalidData` when the TOML text
    /// cannot be decoded or declares an unsupported schema version.
    pub fn from_toml_str(name: &'static str, text: &str) -> std::io::Result<Self> {
        let parsed: RuleFile = toml::from_str(text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if parsed.schema != 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Fix: TOML rule file declares schema = {}, expected schema = 1.",
                    parsed.schema
                ),
            ));
        }
        Ok(Self {
            name,
            rules: parsed.equivalence,
        })
    }

    /// Number of equivalence rules loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// True when no rules are loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Iterate the loaded rules in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = &EquivalenceRule> {
        self.rules.iter()
    }
}

const MAX_EQSAT_TOML_RULE_BYTES: u64 = 4 * 1024 * 1024;

fn read_eqsat_toml_bounded(path: &Path) -> std::io::Result<String> {
    let mut reader = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    let mut total = 0u64;
    let mut chunk = [0u8; 8192];
    loop {
        let read = std::io::Read::read(&mut reader, &mut chunk)?;
        if read == 0 {
            return String::from_utf8(bytes).map_err(|error| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, error)
            });
        }
        let read = read as u64;
        total = total.saturating_add(read);
        if total > MAX_EQSAT_TOML_RULE_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "eqsat TOML `{}` exceeds {MAX_EQSAT_TOML_RULE_BYTES} byte rule cap",
                    path.display()
                ),
            ));
        }
        bytes.extend_from_slice(&chunk[..read as usize]);
    }
}

impl<L> Rule<L> for TomlEquivalenceRules
where
    L: ENodeLang + OpIdNode,
{
    fn name(&self) -> &'static str {
        self.name
    }

    fn matches(&self, egraph: &EGraph<L>) -> Vec<(EClassId, EClassId)> {
        if self.rules.is_empty() {
            return Vec::new();
        }
        // Group e-classes by op-id in one pass.
        let mut by_op: FxHashMap<&str, Vec<EClassId>> = FxHashMap::default();
        for (cid, node) in egraph.iter_nodes() {
            if let Some(op_id) = node.op_id() {
                by_op.entry(op_id).or_default().push(cid);
            }
        }
        let mut equivs = Vec::new();
        for rule in &self.rules {
            let lefts = by_op.get(rule.left.as_str());
            let rights = by_op.get(rule.right.as_str());
            if let (Some(lefts), Some(rights)) = (lefts, rights) {
                // Emit the cross product. The egraph union-find
                // collapses redundant unions so duplicate pairs are
                // cheap.
                for &a in lefts {
                    for &b in rights {
                        if a != b {
                            equivs.push((a, b));
                        }
                    }
                }
            }
        }
        equivs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimizer::eqsat::{EChildren, EGraph};
    use std::hash::{Hash, Hasher};

    /// Toy language for the TOML rule tests: a `Named(op_id, children)`
    /// node and a leaf `Lit`. `Named.op_id` is what the equivalence
    /// rule keys on.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Toy {
        Named(&'static str, Vec<EClassId>),
        Lit(u32),
    }

    impl Hash for Toy {
        fn hash<H: Hasher>(&self, state: &mut H) {
            match self {
                Toy::Named(name, children) => {
                    state.write_u8(0);
                    name.hash(state);
                    for c in children {
                        c.hash(state);
                    }
                }
                Toy::Lit(v) => {
                    state.write_u8(1);
                    v.hash(state);
                }
            }
        }
    }

    impl ENodeLang for Toy {
        fn children(&self) -> EChildren {
            match self {
                Toy::Named(_, kids) => kids.iter().copied().collect(),
                Toy::Lit(_) => EChildren::new(),
            }
        }
        fn with_children(&self, children: &[EClassId]) -> Self {
            match self {
                Toy::Named(name, _) => Toy::Named(name, children.to_vec()),
                Toy::Lit(v) => Toy::Lit(*v),
            }
        }
    }

    impl OpIdNode for Toy {
        fn op_id(&self) -> Option<&str> {
            match self {
                Toy::Named(name, _) => Some(name),
                Toy::Lit(_) => None,
            }
        }
    }

    #[test]
    fn from_toml_str_parses_equivalence_pairs() {
        let toml = r#"
schema = 1
[[equivalence]]
left = "a"
right = "b"
[[equivalence]]
left = "c"
right = "d"
"#;
        let rules = TomlEquivalenceRules::from_toml_str("test", toml).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules.iter().next().unwrap().left, "a");
    }

    #[test]
    fn from_toml_str_rejects_wrong_schema() {
        let toml = "schema = 99\nequivalence = []\n";
        let err = TomlEquivalenceRules::from_toml_str("test", toml).unwrap_err();
        assert!(format!("{err}").contains("schema = 1"));
    }

    #[test]
    fn from_toml_str_accepts_empty_equivalence() {
        let toml = "schema = 1\n";
        let rules = TomlEquivalenceRules::from_toml_str("test", toml).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn matches_returns_empty_when_no_rules() {
        let mut egraph: EGraph<Toy> = EGraph::new();
        let _ = egraph.add(Toy::Named("a", vec![]));
        let _ = egraph.add(Toy::Named("b", vec![]));
        let rules = TomlEquivalenceRules::new("empty");
        assert!(rules.matches(&egraph).is_empty());
    }

    #[test]
    fn matches_emits_pair_when_both_op_ids_present() {
        let mut egraph: EGraph<Toy> = EGraph::new();
        let a = egraph.add(Toy::Named("a", vec![]));
        let b = egraph.add(Toy::Named("b", vec![]));
        let toml = "schema = 1\n[[equivalence]]\nleft = \"a\"\nright = \"b\"\n";
        let rules = TomlEquivalenceRules::from_toml_str("test", toml).unwrap();
        let pairs = rules.matches(&egraph);
        assert_eq!(pairs.len(), 1);
        assert!(
            (pairs[0].0 == a && pairs[0].1 == b) || (pairs[0].0 == b && pairs[0].1 == a),
            "expected (a, b) pair; got {pairs:?}"
        );
    }

    #[test]
    fn matches_empty_when_one_side_absent() {
        let mut egraph: EGraph<Toy> = EGraph::new();
        let _ = egraph.add(Toy::Named("a", vec![]));
        // "b" is absent.
        let toml = "schema = 1\n[[equivalence]]\nleft = \"a\"\nright = \"b\"\n";
        let rules = TomlEquivalenceRules::from_toml_str("test", toml).unwrap();
        assert!(rules.matches(&egraph).is_empty());
    }

    #[test]
    fn matches_skips_leaf_nodes_without_op_id() {
        let mut egraph: EGraph<Toy> = EGraph::new();
        let _ = egraph.add(Toy::Lit(7));
        let _ = egraph.add(Toy::Lit(8));
        // Lit has no op_id, so a rule keying on "anything" finds
        // nothing.
        let toml = "schema = 1\n[[equivalence]]\nleft = \"7\"\nright = \"8\"\n";
        let rules = TomlEquivalenceRules::from_toml_str("test", toml).unwrap();
        assert!(rules.matches(&egraph).is_empty());
    }

    #[test]
    fn rule_name_forwards_constructor_name() {
        let rules: TomlEquivalenceRules = TomlEquivalenceRules::new("algebra_v1");
        let r: &dyn Rule<Toy> = &rules;
        assert_eq!(r.name(), "algebra_v1");
    }
}
