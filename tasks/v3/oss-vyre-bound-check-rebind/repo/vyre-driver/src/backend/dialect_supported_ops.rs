//! DialectRegistry-sourced op-support set.
//!
//! Returns the set of every op id registered in the live
//! `DialectRegistry`. Backends that opt into the dialect dispatch
//! path use this in their `supported_ops()` implementation so that
//! `validate_program` no longer requires a parallel `OpDefRegistration`
//! registration.
//!
//! The legacy [`default_supported_ops`](super::validation::default_supported_ops)
//! returns only the frozen language-level op ids (`vyre.node.*`,
//! `vyre.lit_u32`, etc.). A backend that supports the full dialect
//! stdlib calls [`dialect_and_language_supported_ops`] which
//! unions the two sources.

use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

use crate::OpDefRegistration;
use vyre_foundation::ir::OpId;

/// The union of every dialect-registered op id and the frozen
/// language-level ops.
///
/// Computed once, cached. Includes the language-level set so that
/// consumers don't have to merge two sources themselves.
#[must_use]
pub fn dialect_and_language_supported_ops() -> &'static HashSet<OpId> {
    static OPS: OnceLock<HashSet<OpId>> = OnceLock::new();
    OPS.get_or_init(|| {
        let language_ops = super::validation::default_supported_ops();
        let registrations = inventory::iter::<OpDefRegistration>.into_iter();
        let inventory_bound = registrations
            .size_hint()
            .1
            .unwrap_or_else(|| registrations.size_hint().0);
        let reserve = language_ops.len().saturating_add(inventory_bound);
        let mut set = HashSet::new();
        let _ = set.try_reserve(reserve);
        set.extend(language_ops.iter().cloned());
        for reg in registrations {
            let def = (reg.op)();
            set.insert(Arc::<str>::from(def.id));
        }
        set
    })
}

/// Just the dialect-registered ids (without language-level ops).
///
/// # Runtime cost
///
/// First call walks the link-time inventory once and freezes the result;
/// every subsequent call is a single atomic load returning a `&'static`
/// reference to the cached set. The prior uncached design allocated a new
/// `HashSet` per call.
#[must_use]
pub fn dialect_only_supported_ops() -> &'static HashSet<OpId> {
    static OPS: OnceLock<HashSet<OpId>> = OnceLock::new();
    OPS.get_or_init(|| {
        // HOT-PATH-OK: inventory::iter runs once on first access; result frozen
        // for all subsequent lookups. See docs/inventory-contract.md.
        let registrations = inventory::iter::<OpDefRegistration>.into_iter();
        let reserve = registrations
            .size_hint()
            .1
            .unwrap_or_else(|| registrations.size_hint().0);
        let mut set = HashSet::new();
        let _ = set.try_reserve(reserve);
        for reg in registrations {
            let def = (reg.op)();
            set.insert(Arc::<str>::from(def.id));
        }
        set
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialect_set_contains_io_ops() {
        let ops = dialect_only_supported_ops();
        for op in [
            "io.dma_from_nvme",
            "io.write_back_to_nvme",
            "mem.zerocopy_map",
            "mem.unmap",
        ] {
            assert!(
                ops.iter().any(|o| o.as_ref() == op),
                "dialect set missing {op}; saw {:?}",
                ops.iter().map(|o| o.as_ref()).collect::<Vec<_>>().len()
            );
        }
    }

    #[test]
    fn union_includes_both_sources() {
        let union = dialect_and_language_supported_ops();
        assert!(union.iter().any(|o| o.as_ref() == "vyre.node.store"));
        assert!(union.iter().any(|o| o.as_ref() == "io.dma_from_nvme"));
    }

    #[test]
    fn union_size_exceeds_language_alone() {
        let lang = super::super::validation::default_supported_ops().len();
        let union = dialect_and_language_supported_ops().len();
        assert!(union > lang);
    }
}
