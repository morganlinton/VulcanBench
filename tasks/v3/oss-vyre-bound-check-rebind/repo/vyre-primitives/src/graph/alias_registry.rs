//! Compiler Extension Bridge: Binds lock-free aliasing to vyre_foundation.
//!
//! Provides the generic `OpId` interception mechanism mapping the generic query dialect AST
//! directly onto the `union_find` registry payload.

use vyre_foundation::ir::DataType;

/// Stable Operation UUID identifying the Lock-Free Alias Union subkernel.
pub const ALIAS_UNION_OP_ID: &str = "vyre-primitives::graph::alias_union";

/// Descriptor for an alias-analysis extension op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasOpDescriptor {
    /// Operand types accepted by the op.
    pub inputs: Vec<DataType>,
    /// Result type produced by the op.
    pub output: DataType,
    /// Human-readable operation contract.
    pub description: &'static str,
    /// True when argument order does not affect the result.
    pub commutative: bool,
    /// True when the op updates the alias data structure.
    pub side_effects: bool,
}

impl AliasOpDescriptor {
    /// Build the lock-free alias-union descriptor.
    #[must_use]
    pub fn alias_union() -> Self {
        Self {
            inputs: vec![DataType::U32, DataType::U32],
            output: DataType::U32,
            description: "Lock-free warp-accelerated union-find alias join",
            commutative: true,
            side_effects: true,
        }
    }
}

/// Registry of alias-analysis extension operations keyed by stable op id.
#[derive(Debug, Default, Clone)]
pub struct AliasRegistry {
    alias_union: Option<AliasOpDescriptor>,
    extension_ops: Vec<(&'static str, AliasOpDescriptor)>,
}

impl AliasRegistry {
    /// Register a descriptor under a stable op id.
    pub fn register(&mut self, op_id: &'static str, descriptor: AliasOpDescriptor) {
        if op_id == ALIAS_UNION_OP_ID {
            self.alias_union = Some(descriptor);
            return;
        }
        match self
            .extension_ops
            .binary_search_by(|(registered, _)| registered.cmp(&op_id))
        {
            Ok(index) => self.extension_ops[index].1 = descriptor,
            Err(index) => self.extension_ops.insert(index, (op_id, descriptor)),
        }
    }

    /// Look up a descriptor by stable op id.
    #[must_use]
    pub fn get(&self, op_id: &str) -> Option<&AliasOpDescriptor> {
        if op_id == ALIAS_UNION_OP_ID {
            return self.alias_union.as_ref();
        }
        self.extension_ops
            .binary_search_by(|(registered, _)| registered.cmp(&op_id))
            .ok()
            .map(|index| &self.extension_ops[index].1)
    }

    /// True when a descriptor is registered for `op_id`.
    #[must_use]
    pub fn contains(&self, op_id: &str) -> bool {
        self.get(op_id).is_some()
    }

    /// Number of registered alias operations.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.alias_union.is_some()) + self.extension_ops.len()
    }

    /// True when no alias operations are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.alias_union.is_none() && self.extension_ops.is_empty()
    }

    /// Return registered op ids in deterministic lookup order.
    ///
    /// The well-known alias-union op is reported first when present; extension
    /// ops follow in their binary-search order.
    #[must_use]
    pub fn registered_op_ids(&self) -> Vec<&'static str> {
        let mut ids = Vec::with_capacity(self.len());
        if self.alias_union.is_some() {
            ids.push(ALIAS_UNION_OP_ID);
        }
        ids.extend(self.extension_ops.iter().map(|(op_id, _)| *op_id));
        ids
    }
}

/// Registers the lock-free alias solver dynamically onto the compiler engine.
/// When an analysis compiler encounters `x == y` under aliased semantic boundaries,
/// the lowering phase maps the AST into this Extern execution route.
pub fn register_alias_ops(registry: &mut AliasRegistry) {
    registry.register(ALIAS_UNION_OP_ID, AliasOpDescriptor::alias_union());
}

/// Build the primitive-default alias operation registry.
#[must_use]
pub fn default_alias_registry() -> AliasRegistry {
    let mut registry = AliasRegistry::default();
    register_alias_ops(&mut registry);
    registry
}

/// True when the well-known alias-union operation is registered.
#[must_use]
pub fn alias_union_registered(registry: &AliasRegistry) -> bool {
    registry.contains(ALIAS_UNION_OP_ID)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_contains_alias_union_only() {
        let registry = default_alias_registry();
        assert_eq!(registry.len(), 1);
        assert!(alias_union_registered(&registry));
    }

    #[test]
    fn empty_registry_has_no_implicit_alias_union() {
        let registry = AliasRegistry::default();
        assert!(registry.is_empty());
        assert!(!alias_union_registered(&registry));
        assert!(!registry.contains(ALIAS_UNION_OP_ID));
    }

    #[test]
    fn alias_union_descriptor_contract_is_pinned() {
        let registry = default_alias_registry();
        let desc = registry
            .get(ALIAS_UNION_OP_ID)
            .expect("Fix: default registry must contain alias-union descriptor");
        assert_eq!(desc.inputs, vec![DataType::U32, DataType::U32]);
        assert_eq!(desc.output, DataType::U32);
        assert!(desc.commutative, "alias-union must be commutative");
        assert!(desc.side_effects, "alias-union mutates union-find state");
    }

    #[test]
    fn extension_registration_updates_without_duplicating_entries() {
        let mut registry = default_alias_registry();
        let mut descriptor = AliasOpDescriptor::alias_union();
        descriptor.description = "test extension alias op";
        descriptor.commutative = false;
        descriptor.side_effects = false;

        registry.register("vyre-primitives::graph::alias_test_ext", descriptor.clone());
        registry.register("vyre-primitives::graph::alias_test_ext", descriptor);

        assert_eq!(registry.len(), 2);
        let ext = registry
            .get("vyre-primitives::graph::alias_test_ext")
            .expect("Fix: extension alias op should be registered");
        assert_eq!(ext.description, "test extension alias op");
        assert!(!ext.commutative);
        assert!(!ext.side_effects);
        assert!(alias_union_registered(&registry));
    }

    #[test]
    fn extension_ops_are_kept_sorted_for_binary_lookup() {
        let mut registry = default_alias_registry();
        for op_id in [
            "vyre-primitives::graph::alias_z",
            "vyre-primitives::graph::alias_a",
            "vyre-primitives::graph::alias_m",
            "vyre-primitives::graph::alias_b",
        ] {
            registry.register(op_id, AliasOpDescriptor::alias_union());
        }

        let ids = registry
            .extension_ops
            .iter()
            .map(|(op_id, _)| *op_id)
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "vyre-primitives::graph::alias_a",
                "vyre-primitives::graph::alias_b",
                "vyre-primitives::graph::alias_m",
                "vyre-primitives::graph::alias_z",
            ],
            "Fix: alias extension registry must stay sorted so lookup is binary-searchable."
        );
        assert!(registry.get("vyre-primitives::graph::alias_m").is_some());
        assert!(registry
            .get("vyre-primitives::graph::alias_missing")
            .is_none());
    }

    #[test]
    fn duplicate_extension_update_preserves_sorted_registry_position() {
        let mut registry = default_alias_registry();
        registry.register(
            "vyre-primitives::graph::alias_c",
            AliasOpDescriptor::alias_union(),
        );
        registry.register(
            "vyre-primitives::graph::alias_a",
            AliasOpDescriptor::alias_union(),
        );
        let mut updated = AliasOpDescriptor::alias_union();
        updated.description = "updated alias op";
        updated.commutative = false;
        registry.register("vyre-primitives::graph::alias_c", updated);

        assert_eq!(registry.len(), 3);
        assert_eq!(
            registry.extension_ops[0].0,
            "vyre-primitives::graph::alias_a"
        );
        assert_eq!(
            registry.extension_ops[1].0,
            "vyre-primitives::graph::alias_c"
        );
        let desc = registry
            .get("vyre-primitives::graph::alias_c")
            .expect("Fix: updated alias_c descriptor must remain registered");
        assert_eq!(desc.description, "updated alias op");
        assert!(!desc.commutative);
    }

    #[test]
    fn registered_op_ids_are_deterministic_and_do_not_expose_descriptors() {
        let mut registry = default_alias_registry();
        registry.register(
            "vyre-primitives::graph::alias_z",
            AliasOpDescriptor::alias_union(),
        );
        registry.register(
            "vyre-primitives::graph::alias_a",
            AliasOpDescriptor::alias_union(),
        );

        assert_eq!(
            registry.registered_op_ids(),
            vec![
                ALIAS_UNION_OP_ID,
                "vyre-primitives::graph::alias_a",
                "vyre-primitives::graph::alias_z",
            ]
        );
    }
}
