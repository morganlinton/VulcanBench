//! Shared memory-address canonicalization for lowered-IR memory rewrites.

use rustc_hash::FxHashMap;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum AddressKey {
    Const(u32),
    Result(u32),
}

pub(super) fn address_key(index: u32, literal_values: &FxHashMap<u32, u32>) -> AddressKey {
    literal_values
        .get(&index)
        .copied()
        .map(AddressKey::Const)
        .unwrap_or(AddressKey::Result(index))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct MemoryTarget {
    pub(super) space: MemorySpace,
    pub(super) slot: u32,
}

impl MemoryTarget {
    pub(super) const fn global(slot: u32) -> Self {
        Self {
            space: MemorySpace::Global,
            slot,
        }
    }

    pub(super) const fn shared(slot: u32) -> Self {
        Self {
            space: MemorySpace::Shared,
            slot,
        }
    }

    pub(super) const fn constant(slot: u32) -> Self {
        Self {
            space: MemorySpace::Constant,
            slot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum MemorySpace {
    Global,
    Shared,
    Constant,
}

#[derive(Debug, Clone, Copy, Eq)]
pub(super) struct MemoryLocation {
    pub(super) target: MemoryTarget,
    pub(super) index_operand: u32,
    pub(super) address: AddressKey,
}

impl MemoryLocation {
    pub(super) const fn new(target: MemoryTarget, index_operand: u32, address: AddressKey) -> Self {
        Self {
            target,
            index_operand,
            address,
        }
    }
}

impl PartialEq for MemoryLocation {
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target && self.address == other.address
    }
}

impl Hash for MemoryLocation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.hash(state);
        self.address.hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlotAliasPolicy {
    DistinctSlotsMayAlias,
    DistinctSlotsNeverAlias,
}

pub(super) fn locations_may_alias(
    left: MemoryLocation,
    right: MemoryLocation,
    alias_facts: Option<&crate::analyses::alias_facts::AliasFactSet>,
    slot_policy: SlotAliasPolicy,
) -> bool {
    if left.target.space != right.target.space {
        return false;
    }
    if matches!(left.target.space, MemorySpace::Constant) {
        return false;
    }
    if left.target.slot != right.target.slot {
        return match slot_policy {
            SlotAliasPolicy::DistinctSlotsNeverAlias => false,
            SlotAliasPolicy::DistinctSlotsMayAlias => !alias_facts.is_some_and(|facts| {
                facts.proves_no_alias(
                    left.target.slot,
                    left.index_operand,
                    right.target.slot,
                    right.index_operand,
                )
            }),
        };
    }
    match (left.address, right.address) {
        (AddressKey::Const(left), AddressKey::Const(right)) => return left == right,
        (AddressKey::Result(left), AddressKey::Result(right)) if left == right => return true,
        _ => {}
    }
    !alias_facts.is_some_and(|facts| {
        facts.proves_no_alias(
            left.target.slot,
            left.index_operand,
            right.target.slot,
            right.index_operand,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_key_canonicalizes_literal_results() {
        let mut literals = FxHashMap::default();
        literals.insert(7, 42);

        assert_eq!(address_key(7, &literals), AddressKey::Const(42));
        assert_eq!(address_key(8, &literals), AddressKey::Result(8));
    }

    #[test]
    fn generated_address_key_preserves_sparse_nonliteral_ids() {
        let mut literals = FxHashMap::default();
        for id in (0_u32..=2048).step_by(2) {
            literals.insert(id, id.wrapping_mul(3));
        }

        for id in 0_u32..=2048 {
            let expected = if id % 2 == 0 {
                AddressKey::Const(id.wrapping_mul(3))
            } else {
                AddressKey::Result(id)
            };
            assert_eq!(address_key(id, &literals), expected);
        }
    }

    #[test]
    fn memory_location_equality_ignores_equivalent_index_operand_ids() {
        let left = MemoryLocation::new(MemoryTarget::global(0), 7, AddressKey::Const(128));
        let right = MemoryLocation::new(MemoryTarget::global(0), 9, AddressKey::Const(128));

        assert_eq!(left, right);
    }

    #[test]
    fn slot_alias_policy_preserves_pass_specific_conservatism() {
        let left = MemoryLocation::new(MemoryTarget::global(0), 1, AddressKey::Result(1));
        let right = MemoryLocation::new(MemoryTarget::global(1), 2, AddressKey::Result(2));

        assert!(locations_may_alias(
            left,
            right,
            None,
            SlotAliasPolicy::DistinctSlotsMayAlias
        ));
        assert!(!locations_may_alias(
            left,
            right,
            None,
            SlotAliasPolicy::DistinctSlotsNeverAlias
        ));
    }

    #[test]
    fn constant_space_never_aliases_mutable_writes() {
        let left = MemoryLocation::new(MemoryTarget::constant(0), 1, AddressKey::Result(1));
        let right = MemoryLocation::new(MemoryTarget::constant(0), 1, AddressKey::Result(1));

        assert!(!locations_may_alias(
            left,
            right,
            None,
            SlotAliasPolicy::DistinctSlotsMayAlias
        ));
    }
}
