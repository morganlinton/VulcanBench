const TAG_SHIFT: u32 = 0;
const TAG_REDUCE: u32 = 1;
const TAG_ACCEPT: u32 = 2;
const TAG_ERROR: u32 = 3;
const PAYLOAD_MASK: u32 = 0x0FFF_FFFF;

pub(super) const fn pack_shift(state: u32) -> u32 {
    (TAG_SHIFT << 28) | (state & PAYLOAD_MASK)
}

pub(super) const fn pack_reduce(prod: u32) -> u32 {
    (TAG_REDUCE << 28) | (prod & PAYLOAD_MASK)
}

pub(super) const fn pack_accept() -> u32 {
    TAG_ACCEPT << 28
}

pub(super) const fn pack_error() -> u32 {
    TAG_ERROR << 28
}

/// Encoded LR action stored as `(tag << 28) | payload` inside a `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Shift to next state.
    Shift(u32),
    /// Reduce by production id.
    Reduce(u32),
    /// Accept: parse complete.
    Accept,
    /// Error: unrecognized token in current state.
    Error,
}

impl Action {
    /// Pack into a `u32` word suitable for static table storage.
    #[must_use]
    pub const fn pack(self) -> u32 {
        match self {
            Action::Shift(s) => pack_shift(s),
            Action::Reduce(r) => pack_reduce(r),
            Action::Accept => pack_accept(),
            Action::Error => pack_error(),
        }
    }

    /// Unpack a `u32` word back into an `Action`.
    #[must_use]
    pub const fn unpack(word: u32) -> Self {
        let tag = word >> 28;
        let payload = word & PAYLOAD_MASK;
        match tag {
            0 => Action::Shift(payload),
            1 => Action::Reduce(payload),
            2 => Action::Accept,
            _ => Action::Error,
        }
    }
}
