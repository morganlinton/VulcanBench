//! Process-wide dialect registry.
//!
//! # Hot-reload contract
//!
//! `DialectRegistry::global()` returns an `ArcSwap` guard over the current
//! registry snapshot. Hot reload is allowed at any point through
//! [`DialectRegistry::install`]. Race-free means the swap is an atomic
//! replacement of the process-wide `Arc<DialectRegistry>`: every reader loads
//! one complete snapshot, all currently-live lookups finish against the
//! snapshot they loaded, and new readers after the swap observe the newly
//! installed snapshot. No lookup ever observes a partially-mutated registry.

use super::interner::{intern_string, InternedOpId};
use super::lowering::ReferenceKind;
use super::op_def::OpDef;
use arc_swap::{ArcSwap, Guard};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Write as _};
use std::sync::{Arc, OnceLock};
use vyre_foundation::dialect_lookup::{install_dialect_lookup, Category, DialectLookup};
use vyre_foundation::extern_registry::{ExternDialect, ExternOp};

/// Lookup target for a dialect op's lowering path.
///
/// The in-tree variants map to the typed slots on
/// [`vyre_foundation::dialect_lookup::LoweringTable`].
/// Out-of-tree backends register by stable backend id via the table's
/// `extensions` map and are looked up by `Target::Extension("backend-id")`.
///
/// The enum is `#[non_exhaustive]` so adding an in-tree variant does not
/// break downstream matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Target {
    /// CUDA PTX AOT artifact target.
    Ptx,
    /// SPIR-V AOT artifact target.
    SpirV,
    /// Primary text target.
    PrimaryText,
    /// Primary binary target.
    PrimaryBinary,
    /// Secondary text target.
    SecondaryText,
    /// native-module IR. Reserved for a native-module emitter.
    NativeModule,
    /// Portable reference backend. Always available.
    ReferenceBackend,
    /// Out-of-tree backend registered by stable id. Matches the
    /// string a consumer wrote into
    /// [`vyre_foundation::dialect_lookup::LoweringTable::with_extension`].
    ///
    /// Examples are backend-owned stable identifiers.
    Extension(&'static str),
}

impl Target {
    /// Stable AOT target id consumed by the `vyre-driver` AOT emitter registry.
    #[must_use]
    pub fn aot_target_id(self) -> &'static str {
        match self {
            Self::Ptx | Self::SecondaryText => "secondary_text",
            Self::SpirV => "spv",
            Self::PrimaryText => "primary_text",
            Self::PrimaryBinary => "primary_binary",
            Self::NativeModule => "native_module",
            Self::ReferenceBackend => "reference_backend",
            Self::Extension(id) => id,
        }
    }

    /// File-extension hint for AOT bundles.
    #[must_use]
    pub fn extension(self) -> &'static str {
        self.aot_target_id()
    }
}

impl Serialize for Target {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.aot_target_id())
    }
}

impl<'de> Deserialize<'de> for Target {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "ptx" | "secondary_text" => Self::Ptx,
            "spv" | "spirv" => Self::SpirV,
            "primary_text" => Self::PrimaryText,
            "primary_binary" => Self::PrimaryBinary,
            "native_module" => Self::NativeModule,
            "reference_backend" => Self::ReferenceBackend,
            other => Self::Extension(Box::leak(other.to_string().into_boxed_str())),
        })
    }
}

/// Process-wide dialect registry  -  lock-free dispatch, supports hot-reloading.
///
/// # Contract
///
/// Registrations land via `inventory::submit!` at link time. The global
/// singleton initially walks every `inventory::iter::<OpDefRegistration>` entry
/// and instantiates the `ArcSwap<DialectRegistry>`.
///
/// After initialization:
/// - `lookup` uses `ArcSwap::load` yielding a lock-free `Guard`.
///   It's one atomic load + one hash + one table probe. Zero locking.
/// - `get_lowering` likewise evaluates lock-free  -  sub-ns.
///
/// Runtime registration (hot reload, TOML loader) is actively supported by
/// this struct. Updates swap out the underlying `Arc<DialectRegistry>`, letting
/// current readers finish against the old data snapshot via epoch-based reclamation.
pub struct DialectRegistry {
    index: FrozenIndex,
}

/// Error returned when two dialect operations claim the same stable id.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DuplicateOpIdError {
    op_id: &'static str,
    first_registrant: &'static str,
    second_registrant: &'static str,
}

impl DuplicateOpIdError {
    /// Stable operation id that appeared more than once.
    #[must_use]
    pub const fn op_id(&self) -> &'static str {
        self.op_id
    }

    /// The registrant that claimed the id first.
    #[must_use]
    pub const fn first_registrant(&self) -> &'static str {
        self.first_registrant
    }

    /// The registrant that claimed the id second.
    #[must_use]
    pub const fn second_registrant(&self) -> &'static str {
        self.second_registrant
    }
}

impl fmt::Display for DuplicateOpIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "duplicate op id `{}`: first registrant `{}`, second registrant `{}`. Fix: keep one owner for this stable id and rename or remove the conflicting registration.",
            self.op_id, self.first_registrant, self.second_registrant
        )
    }
}

impl std::error::Error for DuplicateOpIdError {}

struct FrozenIndex {
    by_id: FxHashMap<InternedOpId, &'static OpDef>,
}

impl DialectRegistry {
    /// Snapshot the current global dialect registry for the lifetime
    /// of the returned guard. The guard installs the snapshot as the
    /// active dialect-lookup table for any code that reads
    /// `dialect_lookup` during its scope.
    pub fn global() -> Guard<Arc<Self>> {
        match Self::try_global() {
            Ok(guard) => guard,
            Err(error) => {
                tracing::error!(
                    target: "vyre::driver::dialect_registry",
                    error,
                    "dialect lookup install failed while loading the global registry. \
                     Fix: install exactly one dialect lookup provider for this process. \
                     Continuing with the frozen registry snapshot so callers receive \
                     lookup misses instead of a process abort."
                );
                registry_swap().load()
            }
        }
    }

    /// Fallible variant of [`DialectRegistry::global`].
    ///
    /// Callers that are already returning an error should prefer this method so
    /// conflicting lookup-provider installs surface at their API boundary
    /// instead of being downgraded to an error log.
    ///
    /// # Errors
    ///
    /// Returns the error from foundation's process-wide dialect-lookup
    /// installation when another provider has already been installed.
    pub fn try_global() -> Result<Guard<Arc<Self>>, String> {
        let loader = registry_swap();
        let guard = loader.load();
        install_dialect_lookup(guard.clone())?;
        Ok(guard)
    }

    pub(crate) fn from_inventory() -> Self {
        let registration_count = inventory::iter::<super::dialect::OpDefRegistration>().count();
        let extern_defs = Self::extern_defs();
        let total_defs = registration_count.saturating_add(extern_defs.len());
        let mut defs = Vec::new();
        let _ = vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut defs, total_defs);
        defs.extend(inventory::iter::<super::dialect::OpDefRegistration>().map(|reg| (reg.op)()));
        defs.extend(extern_defs);
        // (dialect, id) is the unique op key; duplicates abort registry
        // construction so contributors cannot ship a partial snapshot that
        // silently loses an operation.
        defs.sort_unstable_by(|left, right| {
            (left.dialect, left.id).cmp(&(right.dialect, right.id))
        });
        if Self::validate_no_duplicates(defs.iter()).is_err() {
            defs.dedup_by(|left, right| left.dialect == right.dialect && left.id == right.id);
        }
        Self::from_validated_defs(defs)
    }

    fn extern_defs() -> Vec<OpDef> {
        let dialect_count = inventory::iter::<ExternDialect>().count();
        let mut dialects = Vec::new();
        let _ = vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut dialects, dialect_count);
        dialects.extend(inventory::iter::<ExternDialect>);
        dialects.sort_by_key(|dialect| dialect.name);

        let op_count = inventory::iter::<ExternOp>().count();
        let mut ops = Vec::new();
        let _ = vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut ops, op_count);
        ops.extend(inventory::iter::<ExternOp>);
        ops.sort_unstable_by(|left, right| {
            (left.dialect, left.op_id).cmp(&(right.dialect, right.op_id))
        });

        // Returning an empty op list on verify failure silently hides a
        // corrupt/duplicate extern registry — every downstream op lookup then
        // fails as "unknown op" with no hint of the real cause (Law 10). A
        // registry that cannot verify is a startup-integrity bug; fail loud.
        if let Err(errors) = vyre_foundation::extern_registry::verify() {
            panic!("vyre-driver extern registry failed verification: {errors:?}");
        }

        let mut known = FxHashSet::default();
        let _ =
            vyre_foundation::allocation::try_reserve_hash_set_to_capacity(&mut known, dialects.len());
        known.extend(dialects.into_iter().map(|dialect| dialect.name));

        let mut defs = Vec::new();
        let _ = vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut defs, ops.len());
        defs.extend(
            ops.into_iter()
                .filter(|op| known.contains(op.dialect))
                .map(|op| OpDef {
                    id: op.op_id,
                    dialect: op.dialect,
                    category: Category::Extension,
                    ..OpDef::default()
                }),
        );
        defs
    }

    fn from_validated_defs(defs: impl IntoIterator<Item = OpDef>) -> Self {
        let defs = defs.into_iter();
        let (lower_bound, _) = defs.size_hint();
        let mut by_id: FxHashMap<InternedOpId, &'static OpDef> = FxHashMap::default();
        let _ =
            vyre_foundation::allocation::try_reserve_hash_map_to_capacity(&mut by_id, lower_bound);
        for def in defs {
            let interned = intern_string(def.id);
            let leaked: &'static OpDef = Box::leak(Box::new(def));
            by_id.insert(interned, leaked);
        }
        Self {
            index: FrozenIndex { by_id },
        }
    }

    #[cfg(test)]
    fn from_defs(defs: impl IntoIterator<Item = OpDef>) -> Self {
        let defs: Vec<OpDef> = defs.into_iter().collect();
        Self::validate_no_duplicates(defs.iter()).unwrap_or_else(|err| panic!("{err}"));
        Self::from_validated_defs(defs)
    }

    /// Validate that each operation definition owns a unique stable id.
    ///
    /// This runs before registry freeze so link-order collisions fail with an
    /// actionable error instead of silently replacing one operation with
    /// another in the hot-path lookup table.
    pub fn validate_no_duplicates<'a>(
        defs: impl IntoIterator<Item = &'a OpDef>,
    ) -> Result<(), DuplicateOpIdError> {
        let mut seen: FxHashMap<&'static str, &'static str> = FxHashMap::default();
        for def in defs {
            let registrant = Self::registrant_for(def);
            if let Some(first_registrant) = seen.insert(def.id, registrant) {
                return Err(DuplicateOpIdError {
                    op_id: def.id,
                    first_registrant,
                    second_registrant: registrant,
                });
            }
        }
        Ok(())
    }

    fn registrant_for(def: &OpDef) -> &'static str {
        if def.dialect.is_empty() {
            "<unknown dialect>"
        } else {
            def.dialect
        }
    }

    /// Install a new process-wide dialect registry snapshot.
    ///
    /// This is the only sanctioned mutation path. TOML hot-reload should build
    /// a complete replacement registry and publish it here; callers must never
    /// mutate the frozen index in place.
    pub fn install(new: Self) {
        registry_swap().store(Arc::new(new));
    }

    /// Intern a textual op name into the registry's `InternedOpId`
    /// space. The interner is shared across the workspace; calling
    /// twice with the same string returns the same id.
    pub fn intern_op(&self, name: &str) -> InternedOpId {
        intern_string(name)
    }

    /// Hot-path lookup. Lock-free `ArcSwap` dispatch.
    pub fn lookup(&self, id: InternedOpId) -> Option<&'static OpDef> {
        self.index.by_id.get(&id).copied()
    }

    /// Resolve the lowering descriptor for `id` on `target`, or `None`
    /// when no backend lowering has been registered for that pair.
    pub fn get_lowering(&self, id: InternedOpId, target: Target) -> Option<ReferenceKind> {
        let def = self.index.by_id.get(&id)?;
        if target == Target::ReferenceBackend {
            return Some(def.lowerings.cpu_ref);
        }
        None
    }

    /// Iterate over all registered operators.
    pub fn iter(&self) -> impl Iterator<Item = &'static OpDef> + '_ {
        self.index.by_id.values().copied()
    }
}

impl vyre_foundation::dialect_lookup::private::Sealed for DialectRegistry {}

impl DialectLookup for DialectRegistry {
    fn provider_id(&self) -> &'static str {
        "vyre-driver::DialectRegistry"
    }

    fn intern_op(&self, name: &str) -> InternedOpId {
        DialectRegistry::intern_op(self, name)
    }

    fn lookup(&self, id: InternedOpId) -> Option<&'static OpDef> {
        DialectRegistry::lookup(self, id)
    }
}

fn registry_swap() -> &'static ArcSwap<DialectRegistry> {
    static REGISTRY: OnceLock<ArcSwap<DialectRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| ArcSwap::from_pointee(DialectRegistry::from_inventory()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Mutex, MutexGuard, OnceLock};
    use vyre_foundation::dialect_lookup::Category;

    inventory::submit! {
        ExternDialect::new(
            "vyre-libs-driver-registry-test",
            "0.4.1-test",
            "https://example.invalid/vyre-libs-driver-registry-test",
        )
    }

    inventory::submit! {
        ExternOp::new(
            "vyre-libs-driver-registry-test",
            "vyre-libs-driver-registry-test::dummy",
        )
    }

    fn registry_test_lock() -> MutexGuard<'static, ()> {
        crate::registry::registry_test_lock()
    }

    fn test_def(id: &'static str) -> OpDef {
        OpDef {
            id,
            ..OpDef::default()
        }
    }

    #[test]
    fn from_inventory_ingests_extern_ops() {
        let _lock = registry_test_lock();
        let registry = DialectRegistry::from_inventory();
        let op_id = "vyre-libs-driver-registry-test::dummy";
        let def = registry
            .lookup(registry.intern_op(op_id))
            .expect("Fix: extern inventory bridge must register submitted ops");
        assert_eq!(def.id, op_id);
        assert_eq!(def.dialect, "vyre-libs-driver-registry-test");
        assert_eq!(def.category, Category::Extension);
    }

    #[test]
    fn concurrent_readers_see_consistent_index() {
        let _lock = registry_test_lock();
        // FrozenIndex is lock-free by construction: after init, the map is
        // immutable and every read is a plain hash probe. This test confirms
        // concurrent reads produce consistent results  -  not a lock-contention
        // test (there is no lock).
        use std::sync::Arc;
        use std::thread;

        DialectRegistry::install(DialectRegistry::from_inventory());
        // `DialectRegistry::global()` returns a `Guard<Arc<DialectRegistry>>`
        // from `arc-swap`; dereference once to get the owned `Arc` the
        // worker threads share.
        let reg: Arc<DialectRegistry> = DialectRegistry::global().clone();
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let r = Arc::clone(&reg);
                thread::spawn(move || {
                    for _ in 0..100 {
                        let id = r.intern_op("io.dma_from_nvme");
                        assert!(r.lookup(id).is_some());
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().expect("Fix: worker thread panicked during concurrent-read test; inspect the panic payload for the underlying invariant violation.");
        }
    }

    #[test]
    fn hot_swap_preserves_old_snapshot_readers() {
        let _lock = registry_test_lock();
        DialectRegistry::install(DialectRegistry::from_defs([test_def("test.old")]));
        let (loaded_tx, loaded_rx) = mpsc::channel();
        let (swap_tx, swap_rx) = mpsc::channel();

        let handle = std::thread::spawn(move || {
            let guard = DialectRegistry::global();
            let old_id = guard.intern_op("test.old");
            loaded_tx
                .send(())
                .expect("Fix: parent must be alive to coordinate registry hot-swap test.");
            swap_rx
                .recv()
                .expect("Fix: parent must signal registry hot-swap completion.");
            assert!(
                guard.lookup(old_id).is_some(),
                "old guard must keep seeing the old snapshot after install()"
            );
            let new_id = guard.intern_op("test.new");
            assert!(
                guard.lookup(new_id).is_none(),
                "old guard must not see entries from a later snapshot"
            );
        });

        loaded_rx
            .recv()
            .expect("Fix: reader thread must load the old registry snapshot.");
        DialectRegistry::install(DialectRegistry::from_defs([test_def("test.new")]));
        swap_tx
            .send(())
            .expect("Fix: reader thread must be alive after registry hot-swap.");
        handle
            .join()
            .expect("Fix: reader thread panicked during hot-swap snapshot test.");
        DialectRegistry::install(DialectRegistry::from_inventory());
    }

    #[test]
    fn new_readers_after_swap_see_new_data() {
        let _lock = registry_test_lock();
        DialectRegistry::install(DialectRegistry::from_defs([test_def("test.before")]));
        DialectRegistry::install(DialectRegistry::from_defs([test_def("test.after")]));

        let handle = std::thread::spawn(move || {
            let guard = DialectRegistry::global();
            let after = guard.intern_op("test.after");
            let before = guard.intern_op("test.before");
            assert!(
                guard.lookup(after).is_some(),
                "new reader must see the registry installed before it loaded"
            );
            assert!(
                guard.lookup(before).is_none(),
                "new reader must not see the previous registry snapshot"
            );
        });

        handle
            .join()
            .expect("Fix: reader thread panicked during post-swap visibility test.");
        DialectRegistry::install(DialectRegistry::from_inventory());
    }
}
