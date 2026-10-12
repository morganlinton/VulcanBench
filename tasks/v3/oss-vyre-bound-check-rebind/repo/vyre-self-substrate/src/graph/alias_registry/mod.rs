//! Alias-registry dispatch wrapper.
//!
//! Wires `vyre_primitives::graph::alias_registry::register_alias_ops`
//! into the dispatch path so the optimizer can stand up the
//! lock-free alias-union descriptor table at startup. Registry
//! lookups (a hot path during alias-analysis) bump a dedicated substrate
//! counter so observability dashboards see the consumption rate.

use vyre_primitives::graph::alias_registry::{
    alias_union_registered as primitive_alias_union_registered,
    default_alias_registry as primitive_default_alias_registry, AliasOpDescriptor, AliasRegistry,
};

/// Build a registry pre-populated with vyre's default alias-analysis
/// op descriptors. Bumps the alias-registry substrate counter so
/// observability can track how many registries the dispatch path
/// instantiates.
#[must_use]
pub fn build_default_registry() -> AliasRegistry {
    use crate::observability::{alias_registry_calls, bump};
    bump(&alias_registry_calls);
    primitive_default_alias_registry()
}

/// Look up an alias op descriptor in `registry`. Bumps the
/// substrate counter so per-query observability is visible.
#[must_use]
pub fn lookup_alias_op<'a>(
    registry: &'a AliasRegistry,
    op_id: &str,
) -> Option<&'a AliasOpDescriptor> {
    use crate::observability::{alias_registry_calls, bump};
    bump(&alias_registry_calls);
    registry.get(op_id)
}

/// Convenience: returns whether the well-known alias-union op is
/// registered. The dispatch-time alias analyzer consults this
/// before emitting alias-union nodes.
#[must_use]
pub fn alias_union_registered(registry: &AliasRegistry) -> bool {
    use crate::observability::{alias_registry_calls, bump};
    bump(&alias_registry_calls);
    primitive_alias_union_registered(registry)
}

#[cfg(test)]
mod tests;
