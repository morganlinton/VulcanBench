//! External alias and points-to facts for lowering legality.
//!
//! This module is the substrate-neutral boundary between Dataflow
//! analyses and Vyre descriptor rewrites. Passes use [`AliasFactSet`]
//! instead of ad-hoc structural alias guesses when external facts are
//! available.

use std::collections::BTreeSet;

/// One proven non-aliasing relation between two descriptor addresses.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct NoAliasFact {
    /// Binding slot for the first descriptor address.
    pub left_binding: u32,
    /// Descriptor result id that computes the first address index.
    pub left_index: u32,
    /// Binding slot for the second descriptor address.
    pub right_binding: u32,
    /// Descriptor result id that computes the second address index.
    pub right_index: u32,
}

/// Alias facts imported from external points-to/may-alias analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AliasFactSet {
    no_alias: BTreeSet<NoAliasFact>,
}

impl AliasFactSet {
    /// Insert a bidirectional no-alias fact.
    pub fn insert_no_alias(&mut self, fact: NoAliasFact) {
        self.no_alias.insert(fact.clone());
        self.no_alias.insert(NoAliasFact {
            left_binding: fact.right_binding,
            left_index: fact.right_index,
            right_binding: fact.left_binding,
            right_index: fact.left_index,
        });
    }

    /// Return true when external analysis proved the two descriptor addresses cannot alias.
    #[must_use]
    pub fn proves_no_alias(
        &self,
        left_binding: u32,
        left_index: u32,
        right_binding: u32,
        right_index: u32,
    ) -> bool {
        self.no_alias.contains(&NoAliasFact {
            left_binding,
            left_index,
            right_binding,
            right_index,
        })
    }

    /// Number of stored directed facts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.no_alias.len()
    }

    /// True when no facts have been imported.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.no_alias.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_alias_facts_are_bidirectional() {
        let mut facts = AliasFactSet::default();
        facts.insert_no_alias(NoAliasFact {
            left_binding: 1,
            left_index: 7,
            right_binding: 2,
            right_index: 9,
        });
        assert!(facts.proves_no_alias(1, 7, 2, 9));
        assert!(facts.proves_no_alias(2, 9, 1, 7));
    }
}
