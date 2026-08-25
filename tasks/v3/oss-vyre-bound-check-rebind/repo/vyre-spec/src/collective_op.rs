//! Frozen collective-communication operation contracts.
//!
//! These types are IR-level contracts only. Concrete transport bindings such
//! as NCCL, UCX, SHARP, or MPI live in backend crates.
// TAG RESERVATIONS: Sum=0x01, Min=0x02, Max=0x03, BitAnd=0x04,
// BitOr=0x05, BitXor=0x06, 0x07..=0x7F reserved.

/// Reduction operator used by distributed collective nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub enum CollectiveOp {
    /// Sum reduction.
    Sum,
    /// Minimum reduction.
    Min,
    /// Maximum reduction.
    Max,
    /// Bitwise AND reduction.
    BitAnd,
    /// Bitwise OR reduction.
    BitOr,
    /// Bitwise XOR reduction.
    BitXor,
}

impl CollectiveOp {
    /// Frozen builtin wire tag for this collective operator.
    #[must_use]
    pub const fn builtin_wire_tag(self) -> u8 {
        match self {
            Self::Sum => 0x01,
            Self::Min => 0x02,
            Self::Max => 0x03,
            Self::BitAnd => 0x04,
            Self::BitOr => 0x05,
            Self::BitXor => 0x06,
        }
    }

    /// Decode a frozen builtin wire tag.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when `tag` is not assigned.
    pub fn from_wire_tag(tag: u8) -> Result<Self, String> {
        match tag {
            0x01 => Ok(Self::Sum),
            0x02 => Ok(Self::Min),
            0x03 => Ok(Self::Max),
            0x04 => Ok(Self::BitAnd),
            0x05 => Ok(Self::BitOr),
            0x06 => Ok(Self::BitXor),
            value => Err(format!(
                "Fix: unknown CollectiveOp tag {value}; use a Program serializer compatible with this vyre version."
            )),
        }
    }
}

/// Opaque communicator/group handle carried by collective nodes.
///
/// `0` is the process/world group by convention. Other ids are backend-owned
/// handles resolved by the runtime communicator registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct CommGroup(pub u32);

impl CommGroup {
    /// Default world communicator group.
    pub const WORLD: Self = Self(0);

    /// Return the stable group id.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}
