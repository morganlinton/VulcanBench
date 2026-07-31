//! Effect-handler application primitive (P-1.0-V1.1).
//!
//! Given an effect row (the side-effects a Region produces) and a
//! handler (the set of effects it discharges), `handler_apply`
//! returns the residual row. Composition of handlers is V1.2.
//!
//! The substrate handles a finite, ordered set of effect kinds via a
//! u32 bitmask so the apply step is O(1) and lock-free.

/// One concrete side-effect kind. Indexed by bit position into
/// [`EffectRow`]. `#[non_exhaustive]` so future kinds (collective
/// ops, persistent-storage writes, …) can land without breaking
/// pattern matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EffectKind {
    /// A buffer write (Node::Store, Node::AsyncStore).
    BufferWrite,
    /// An atomic read-modify-write (Expr::Atomic).
    Atomic,
    /// A host-visible I/O effect (e.g. printk via host bridge).
    HostIo,
    /// A nested GPU dispatch (Node::IndirectDispatch).
    GpuDispatch,
    /// A barrier or synchronization primitive (Node::Barrier { ordering: vyre::memory_model::MemoryOrdering::SeqCst }).
    Barrier,
    /// An async-load fetching from persistent / streaming storage
    /// (Node::AsyncLoad).
    AsyncLoad,
    /// A trap or abort (Node::Trap).
    Trap,
}

impl EffectKind {
    /// Bit position in an [`EffectRow`].
    #[must_use]
    #[inline]
    pub const fn bit(self) -> u32 {
        match self {
            Self::BufferWrite => 0,
            Self::Atomic => 1,
            Self::HostIo => 2,
            Self::GpuDispatch => 3,
            Self::Barrier => 4,
            Self::AsyncLoad => 5,
            Self::Trap => 6,
        }
    }

    /// Mask with this single bit set.
    #[must_use]
    #[inline]
    pub const fn mask(self) -> u32 {
        1u32 << self.bit()
    }
}

/// Set of effect kinds produced by a Region. A row is a u32 bitmask
/// indexed by `EffectKind::bit()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EffectRow(u32);

impl EffectRow {
    /// Empty row (no effects).
    #[must_use]
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Row from a raw u32 bitmask.
    #[must_use]
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Row containing exactly one effect kind.
    #[must_use]
    #[inline]
    pub const fn single(kind: EffectKind) -> Self {
        Self(kind.mask())
    }

    /// Raw u32 bitmask.
    #[must_use]
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Whether the row contains the given kind.
    #[must_use]
    #[inline]
    pub const fn contains(self, kind: EffectKind) -> bool {
        self.0 & kind.mask() != 0
    }

    /// Whether the row is empty.
    #[must_use]
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Set-union of two rows.
    #[must_use]
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// A handler discharges a fixed set of effect kinds. Modeled as the
/// row of effects it consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handler {
    handled: EffectRow,
}

impl Handler {
    /// Build a handler from the row of effects it discharges.
    #[must_use]
    #[inline]
    pub const fn from_row(handled: EffectRow) -> Self {
        Self { handled }
    }

    /// Single-effect handler.
    #[must_use]
    #[inline]
    pub const fn single(kind: EffectKind) -> Self {
        Self {
            handled: EffectRow::single(kind),
        }
    }

    /// The row of effects this handler discharges.
    #[must_use]
    #[inline]
    pub const fn handled(self) -> EffectRow {
        self.handled
    }
}

/// Apply `handler` to `row` and return the residual row of open effects.
///
/// Algebraic identity: `handler_apply(handler_apply(row, h), h) ==
/// handler_apply(row, h)` (idempotent), and
/// `handler_apply(row, Handler::from_row(EffectRow::empty())) == row`
/// (identity handler).
#[must_use]
#[inline]
pub const fn handler_apply(row: EffectRow, handler: Handler) -> EffectRow {
    EffectRow(row.0 & !handler.handled.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_row_stays_empty() {
        let h = Handler::single(EffectKind::BufferWrite);
        assert_eq!(handler_apply(EffectRow::empty(), h), EffectRow::empty());
    }

    #[test]
    fn handler_discharges_its_kind() {
        let row = EffectRow::single(EffectKind::BufferWrite);
        let h = Handler::single(EffectKind::BufferWrite);
        assert!(handler_apply(row, h).is_empty());
    }

    #[test]
    fn handler_passes_through_other_kinds() {
        let row = EffectRow::single(EffectKind::Atomic);
        let h = Handler::single(EffectKind::BufferWrite);
        assert_eq!(handler_apply(row, h).bits(), EffectKind::Atomic.mask());
    }

    #[test]
    fn identity_handler_preserves_every_row() {
        let id = Handler::from_row(EffectRow::empty());
        for kind in [
            EffectKind::BufferWrite,
            EffectKind::Atomic,
            EffectKind::HostIo,
            EffectKind::GpuDispatch,
            EffectKind::Barrier,
            EffectKind::AsyncLoad,
            EffectKind::Trap,
        ] {
            let row = EffectRow::single(kind);
            assert_eq!(handler_apply(row, id).bits(), kind.mask());
        }
    }

    #[test]
    fn handler_apply_is_idempotent() {
        let row =
            EffectRow::single(EffectKind::BufferWrite).union(EffectRow::single(EffectKind::Atomic));
        let h = Handler::single(EffectKind::BufferWrite);
        let once = handler_apply(row, h);
        let twice = handler_apply(once, h);
        assert_eq!(once, twice);
    }

    #[test]
    fn multi_effect_row_partial_discharge() {
        let row =
            EffectRow::single(EffectKind::BufferWrite).union(EffectRow::single(EffectKind::Atomic));
        let h = Handler::single(EffectKind::BufferWrite);
        let residual = handler_apply(row, h);
        assert!(!residual.contains(EffectKind::BufferWrite));
        assert!(residual.contains(EffectKind::Atomic));
    }

    #[test]
    fn full_handler_discharges_full_row() {
        let row = EffectRow::single(EffectKind::BufferWrite)
            .union(EffectRow::single(EffectKind::Atomic))
            .union(EffectRow::single(EffectKind::HostIo));
        let h = Handler::from_row(row);
        assert!(handler_apply(row, h).is_empty());
    }

    #[test]
    fn distinct_kinds_have_distinct_bits() {
        let bits: Vec<u32> = [
            EffectKind::BufferWrite,
            EffectKind::Atomic,
            EffectKind::HostIo,
            EffectKind::GpuDispatch,
            EffectKind::Barrier,
            EffectKind::AsyncLoad,
            EffectKind::Trap,
        ]
        .iter()
        .map(|k| k.bit())
        .collect();
        for i in 0..bits.len() {
            for j in (i + 1)..bits.len() {
                assert_ne!(bits[i], bits[j], "kinds {i} and {j} share a bit");
            }
        }
    }

    #[test]
    fn from_bits_round_trip() {
        // Bits 0, 1, 3, 5 set = BufferWrite + Atomic + GpuDispatch + AsyncLoad.
        // Bit positions per `EffectKind::bit()`: BufferWrite=0, Atomic=1,
        // HostIo=2, GpuDispatch=3, Barrier=4, AsyncLoad=5, Trap=6.
        let raw = 0b0010_1011u32;
        let row = EffectRow::from_bits(raw);
        assert_eq!(row.bits(), raw);
        assert!(row.contains(EffectKind::BufferWrite));
        assert!(row.contains(EffectKind::Atomic));
        assert!(!row.contains(EffectKind::HostIo));
        assert!(row.contains(EffectKind::GpuDispatch));
        assert!(!row.contains(EffectKind::Barrier));
        assert!(row.contains(EffectKind::AsyncLoad));
        assert!(!row.contains(EffectKind::Trap));
    }
}
