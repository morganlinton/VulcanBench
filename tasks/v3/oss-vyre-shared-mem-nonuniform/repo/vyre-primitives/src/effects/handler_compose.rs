//! Effect-handler composition primitive (P-1.0-V1.2).
//!
//! Builds a single handler from two: the composed handler discharges
//! the union of the two handlers' effect rows.
//!
//! Algebraic laws (proven by tests below):
//!
//! * **Associativity**: `compose(a, compose(b, c)) == compose(compose(a, b), c)`.
//! * **Commutativity**: `compose(a, b) == compose(b, a)` (handlers are
//!   modeled as effect rows; row-union is commutative).
//! * **Identity**: `compose(a, identity) == a` where identity is the
//!   empty-row handler.
//! * **Idempotence on equal handlers**: `compose(a, a) == a`.
//! * **Apply-compose distributivity**:
//!   `handler_apply(row, compose(a, b)) ==
//!    handler_apply(handler_apply(row, a), b)`.

use super::handler_apply::{EffectRow, Handler};

/// Compose two handlers. The result discharges every effect kind in
/// either input handler.
#[must_use]
#[inline]
pub const fn handler_compose(a: Handler, b: Handler) -> Handler {
    Handler::from_row(EffectRow::from_bits(
        a.handled().bits() | b.handled().bits(),
    ))
}

#[cfg(test)]
mod tests {
    use super::super::handler_apply::{handler_apply, EffectKind, EffectRow, Handler};
    use super::handler_compose;

    fn handler_for(kinds: &[EffectKind]) -> Handler {
        let mut row = EffectRow::empty();
        for k in kinds {
            row = row.union(EffectRow::single(*k));
        }
        Handler::from_row(row)
    }

    #[test]
    fn composed_handler_discharges_union() {
        let a = Handler::single(EffectKind::BufferWrite);
        let b = Handler::single(EffectKind::Atomic);
        let composed = handler_compose(a, b);
        assert!(composed.handled().contains(EffectKind::BufferWrite));
        assert!(composed.handled().contains(EffectKind::Atomic));
        assert!(!composed.handled().contains(EffectKind::HostIo));
    }

    #[test]
    fn compose_is_commutative() {
        let a = Handler::single(EffectKind::BufferWrite);
        let b = Handler::single(EffectKind::Atomic);
        assert_eq!(handler_compose(a, b), handler_compose(b, a));
    }

    #[test]
    fn compose_is_associative() {
        let a = Handler::single(EffectKind::BufferWrite);
        let b = Handler::single(EffectKind::Atomic);
        let c = Handler::single(EffectKind::HostIo);
        assert_eq!(
            handler_compose(a, handler_compose(b, c)),
            handler_compose(handler_compose(a, b), c)
        );
    }

    #[test]
    fn identity_left_neutral() {
        let id = Handler::from_row(EffectRow::empty());
        let h = Handler::single(EffectKind::BufferWrite);
        assert_eq!(
            handler_compose(id, h).handled().bits(),
            EffectKind::BufferWrite.mask()
        );
    }

    #[test]
    fn identity_right_neutral() {
        let id = Handler::from_row(EffectRow::empty());
        let h = Handler::single(EffectKind::BufferWrite);
        assert_eq!(
            handler_compose(h, id).handled().bits(),
            EffectKind::BufferWrite.mask()
        );
    }

    #[test]
    fn idempotent_on_equal() {
        let h = handler_for(&[EffectKind::BufferWrite, EffectKind::Atomic]);
        let mask = EffectKind::BufferWrite.mask() | EffectKind::Atomic.mask();
        assert_eq!(handler_compose(h, h).handled().bits(), mask);
    }

    #[test]
    fn apply_compose_equals_apply_apply() {
        // handler_apply(row, compose(a, b)) ==
        //   handler_apply(handler_apply(row, a), b)
        let row = handler_for(&[
            EffectKind::BufferWrite,
            EffectKind::Atomic,
            EffectKind::HostIo,
        ])
        .handled();
        let a = Handler::single(EffectKind::BufferWrite);
        let b = Handler::single(EffectKind::Atomic);
        let lhs = handler_apply(row, handler_compose(a, b));
        let rhs = handler_apply(handler_apply(row, a), b);
        assert_eq!(lhs, rhs);
    }

    #[test]
    fn composed_full_handler_discharges_full_row() {
        let row = handler_for(&[
            EffectKind::BufferWrite,
            EffectKind::Atomic,
            EffectKind::HostIo,
            EffectKind::GpuDispatch,
        ])
        .handled();
        let composed = handler_compose(
            handler_for(&[EffectKind::BufferWrite, EffectKind::Atomic]),
            handler_for(&[EffectKind::HostIo, EffectKind::GpuDispatch]),
        );
        assert!(handler_apply(row, composed).is_empty());
    }
}
