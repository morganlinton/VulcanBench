//! Wire-format envelope constants.

/// Leading magic bytes for the vyre IR wire format.
///
/// The `VYRE` tag identifies the stable schema documented in
/// `docs/wire-format.md`; the version byte pair [`WIRE_FORMAT_VERSION`]
/// immediately after the magic identifies which schema version the
/// payload follows.
pub const MAGIC: &[u8; 4] = b"VYRE";

/// Current wire-format schema version. Incremented on any
/// breaking schema change (variant added/removed, field reorder, new
/// non-additive framing). Audit L.1.47: the previous format lacked a
/// version field entirely; any schema drift produced arbitrary
/// parse errors with no way to tell caller "you're on a newer
/// version than this decoder knows about".
///
/// Rev 4 preserves program-level composition-safety flags in metadata
/// so parser/stateful kernels do not become fusible after wire
/// round-trip. Rev 3 introduces: structured version-mismatch errors (see
/// [`crate::error::Error::VersionMismatch`]) and a reserved
/// dialect-manifest section after the header for rev-3+ readers. Rev
/// 2 was never released; versions go 1 → 3 directly.
pub const WIRE_FORMAT_VERSION: u16 = 4;
