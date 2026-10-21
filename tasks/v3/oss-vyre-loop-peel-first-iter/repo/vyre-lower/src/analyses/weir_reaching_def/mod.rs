//! Weir reaching-definition import boundary for descriptor rewrites.
//!
//! Weir-derived dataflow facts enter lowered descriptor optimization through
//! the same reaching-definition representation used by the rewrite pipeline.

pub use crate::analyses::reaching_def_facts::{
    import_descriptor_reaching_defs, resolve_copy_alias, ReachingDefFactSet,
};

#[cfg(test)]
mod tests {
    use rustc_hash::FxHashMap;

    use super::*;

    #[test]
    fn weir_reaching_def_api_resolves_copy_alias_chains() {
        let aliases = FxHashMap::from_iter([(20, 10), (30, 20)]);

        assert_eq!(resolve_copy_alias(30, &aliases), 10);
        assert_eq!(resolve_copy_alias(40, &aliases), 40);
    }
}
