//! Substrate-neutral memory model contracts.

/// Memory ordering attached to atomic and barrier operations.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub enum MemoryOrdering {
    /// No synchronization beyond atomicity of the operation.
    Relaxed,
    /// Subsequent reads observe writes released by another participant.
    Acquire,
    /// Prior writes become visible to acquiring participants.
    Release,
    /// Acquire and release semantics in one operation.
    AcqRel,
    /// Single total order across sequentially consistent operations
    /// within the issuing thread's workgroup.
    SeqCst,
    /// Cross-grid synchronization. Every thread in the dispatch waits
    /// here, and every prior write is globally visible after the
    /// barrier returns. This is strictly stronger than `SeqCst`, which
    /// only synchronizes within a workgroup. `GridSync` is required
    /// when a fused kernel has an arm with divergent stores
    /// (e.g. `if invocation_id == K { store ... }`) followed by an arm
    /// that reads what was stored  -  without grid-level sync, threads
    /// in non-K blocks observe stale state. Backends that lack a
    /// native grid barrier (workgroup-only fences, no cooperative
    /// launch) must lower this to a kernel-split: emit two separate
    /// dispatches that share the underlying buffers.
    GridSync,
}

impl MemoryOrdering {
    /// Stable wire tag for this ordering.
    #[must_use]
    #[inline]
    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::Relaxed => 0,
            Self::Acquire => 1,
            Self::Release => 2,
            Self::AcqRel => 3,
            Self::SeqCst => 4,
            Self::GridSync => 5,
        }
    }

    /// Decode a stable wire tag.
    ///
    /// # Errors
    ///
    /// Returns an actionable error when `tag` is not assigned to a memory
    /// ordering in this schema.
    #[inline]
    pub fn from_wire_tag(tag: u8) -> Result<Self, String> {
        match tag {
            0 => Ok(Self::Relaxed),
            1 => Ok(Self::Acquire),
            2 => Ok(Self::Release),
            3 => Ok(Self::AcqRel),
            4 => Ok(Self::SeqCst),
            5 => Ok(Self::GridSync),
            other => Err(format!(
                "InvalidDiscriminant: memory ordering tag {other} is unknown. Fix: reserialize with a compatible VYRE wire schema."
            )),
        }
    }

    /// Whether this ordering is valid for an atomic RMW operation.
    /// `GridSync` is barrier-only and not a valid atomic ordering.
    #[must_use]
    #[inline]
    pub const fn is_valid_for_atomic_rmw(self) -> bool {
        matches!(
            self,
            Self::Relaxed | Self::Acquire | Self::Release | Self::AcqRel | Self::SeqCst
        )
    }

    /// Whether this ordering is valid for a barrier.
    #[must_use]
    #[inline]
    pub const fn is_valid_for_barrier(self) -> bool {
        matches!(
            self,
            Self::Acquire | Self::Release | Self::AcqRel | Self::SeqCst | Self::GridSync
        )
    }

    /// Whether this ordering requires cross-grid synchronization.
    /// Backends with a native grid barrier emit one instruction; backends
    /// without must split the kernel.
    #[must_use]
    #[inline]
    pub const fn requires_grid_sync(self) -> bool {
        matches!(self, Self::GridSync)
    }
}

impl Default for MemoryOrdering {
    #[inline]
    fn default() -> Self {
        Self::SeqCst
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_seq_cst() {
        assert_eq!(MemoryOrdering::default(), MemoryOrdering::SeqCst);
    }

    #[test]
    fn all_variants_are_distinct() {
        let variants = [
            MemoryOrdering::Relaxed,
            MemoryOrdering::Acquire,
            MemoryOrdering::Release,
            MemoryOrdering::AcqRel,
            MemoryOrdering::SeqCst,
            MemoryOrdering::GridSync,
        ];
        for i in 0..variants.len() {
            for j in (i + 1)..variants.len() {
                assert_ne!(variants[i], variants[j]);
            }
        }
    }

    #[test]
    fn grid_sync_round_trips() {
        let tag = MemoryOrdering::GridSync.wire_tag();
        assert_eq!(tag, 5);
        assert_eq!(
            MemoryOrdering::from_wire_tag(tag).unwrap(),
            MemoryOrdering::GridSync
        );
        assert!(MemoryOrdering::GridSync.is_valid_for_barrier());
        assert!(!MemoryOrdering::GridSync.is_valid_for_atomic_rmw());
        assert!(MemoryOrdering::GridSync.requires_grid_sync());
        assert!(!MemoryOrdering::SeqCst.requires_grid_sync());
    }

    #[test]
    fn clone_eq() {
        let a = MemoryOrdering::AcqRel;
        let b = a;
        assert_eq!(a, b);
    }
}
