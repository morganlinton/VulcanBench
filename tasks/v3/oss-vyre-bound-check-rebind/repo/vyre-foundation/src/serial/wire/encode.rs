//! Encode the stable IR wire model into `VIR0` bytes.

/// Version label for BLAKE3 digests computed from canonical VIR0 Program wire
/// bytes. Evidence, cache, and replay surfaces import this label instead of
/// inventing parallel Program digest version strings.
pub const PROGRAM_WIRE_DIGEST_VERSION: &str = "vyre-program-wire-vir0-blake3-v1";

/// Encode a single [`crate::ir::Expr`] into its wire-format tag and payload.
///
/// # Role
///
/// This is the leaf encoder for the expression tree. It maps each
/// `Expr` variant to a discriminant byte followed by a
/// variant-specific payload (literals, variable names, operation
/// tags, or nested sub-expressions).
///
/// # Invariants
///
/// The output buffer is appended to only; no bytes are removed or
/// reordered. Recursive calls for nested expressions preserve this
/// invariant.
///
/// # Pre-conditions
///
/// The expression must use only enum variants that have a registered
/// stable wire tag. Variants added to `Expr` without an assigned
/// tag will fail encoding (audit L.1.27 / I4).
///
/// # Return semantics
///
/// * `Ok(())` – the expression was fully appended to `out`.
/// * `Err(String)` – an actionable diagnostic starting with `Fix:`
///   describing the unsupported variant or oversized payload.
///
/// # Failure modes
///
/// * **Unmapped variant** – `bin_op_tag`, `un_op_tag`, or
///   `atomic_op_tag` returns `Err` when the op has no wire tag.
/// * **String overflow** – `put_string` rejects names longer than
///   [`crate::serial::wire::MAX_STRING_LEN`].
/// * **Length overflow** – `put_len_u32` rejects argument counts
///   larger than `u32::MAX`.
pub use put_expr::put_expr;

/// Encode a single [`crate::ir::Node`] into its wire-format tag and payload.
///
/// # Role
///
/// Encodes statement-level IR: bindings, assignments, stores,
/// control flow, barriers, and async operations. Every variant maps
/// to a discriminant byte followed by a payload whose shape mirrors
/// the in-memory `Node` layout.
///
/// # Invariants
///
/// The output buffer is appended to only. Control-flow nodes that
/// contain nested node lists or expressions delegate to
/// [`put_nodes()`] and [`put_expr()`], preserving the append-only
/// invariant.
///
/// # Pre-conditions
///
/// All expressions embedded in the node must satisfy the
/// pre-conditions of [`put_expr()`] (stable wire tags, bounded string
/// lengths).
///
/// # Return semantics
///
/// * `Ok(())` – the node was fully appended to `out`.
/// * `Err(String)` – an actionable diagnostic starting with `Fix:`.
///
/// # Failure modes
///
/// Same as [`put_expr()`] for expression sub-payloads, plus
/// [`put_nodes()`] failures for nested statement lists (e.g. node
/// count exceeds `u32::MAX`).
pub use put_node::put_node;

/// Encode a slice of [`crate::ir::Node`]s as a length-prefixed sequence.
///
/// # Role
///
/// This is the canonical encoder for statement lists: entry bodies,
/// `If`/`Loop`/`Block` children, and any other `[Node]` slice. The
/// wire format represents every list as a little-endian `u32` count
/// followed by that many encoded nodes.
///
/// # Invariants
///
/// The output buffer is appended to only. The length field written
/// equals `nodes.len()`; truncation or padding would corrupt the
/// decoder's bounds checks (I10).
///
/// # Pre-conditions
///
/// `nodes.len()` must fit in `u32`; otherwise the wire format cannot
/// represent the sequence.
///
/// # Return semantics
///
/// * `Ok(())` – count and all nodes were appended.
/// * `Err(String)` – an actionable diagnostic starting with `Fix:`.
///
/// # Failure modes
///
/// * **Length overflow** – `put_len_u32` rejects `nodes.len()` >
///   `u32::MAX` with a `Fix:` error advising the caller to split
///   the program.
/// * **Node encoding failure** – any error from [`put_node()`] is
///   propagated upward unchanged.
pub use put_nodes::put_nodes;

/// Serialize a complete [`crate::ir_inner::model::program::Program`] into the versioned `VIR0` wire envelope.
///
/// # Role
///
/// This is the entry-point encoder. It produces the exact byte
/// sequence that [`crate::ir_inner::model::program::Program::from_wire`] expects: magic,
/// version, entry-op id, buffer table, work-group size, and entry
/// body.
///
/// # Invariants
///
/// The output is a fresh `Vec<u8>`; the caller owns it. Capacity is
/// pre-allocated heuristically to avoid reallocations on typical
/// programs, but the vector grows naturally if the estimate is low.
///
/// # Pre-conditions
///
/// The program must use only enum variants that have stable wire
/// tags. A well-formed program should always encode successfully;
/// encoding failure signals either an unsupported variant
/// (audit L.1.27 / I4) or a field that exceeds wire-format bounds
/// (audit I10).
///
/// # Return semantics
///
/// * `Ok(Vec<u8>)` – a complete VIR0 blob starting with
///   [`crate::serial::wire::framing::MAGIC`] and
///   [`crate::serial::wire::framing::WIRE_FORMAT_VERSION`].
/// * `Err(String)` – an actionable diagnostic starting with `Fix:`.
///
/// # Failure modes
///
/// * **Buffer count overflow** – more than `u32::MAX` buffers.
/// * **String overflow** – buffer names or the entry op id longer
///   than [`crate::serial::wire::MAX_STRING_LEN`] are rejected.
/// * **Unmapped variant** – `access_tag`, `put_data_type`, or nested
///   `put_expr` / `put_node` calls fail when an enum variant has no
///   wire tag.
///
/// # Versioning
///
/// The version bytes are emitted immediately after the magic
/// (audit L.1.47). Any breaking schema change must bump
/// [`crate::serial::wire::framing::WIRE_FORMAT_VERSION`] so older
/// decoders reject the payload with a clear version-mismatch
/// message instead of arbitrary downstream parse errors.
pub use to_wire::to_wire;
pub use to_wire::to_wire_into;
pub use to_wire::to_wire_with_buffer_order_into;

/// Versioned scan database header helpers for cache and benchmark evidence
/// payloads that are not `VIR0` Program blobs.
pub use scan_database_header::{
    decode_compatible_scan_database_header, decode_scan_database_header,
    decode_scan_database_header_with_compatibility, encode_scan_database_header,
    put_scan_database_header, ScanDatabaseCompatibilityRecord, ScanDatabaseHeader,
    ScanDatabaseMode, ScanDatabaseReaderCompatibility, ScanDatabaseSectionHeader,
    ScanDatabaseSectionKind, UnsupportedScanFeature, MAX_SCAN_DATABASE_SECTIONS,
    MAX_SCAN_DATABASE_UNSUPPORTED_FEATURES, SCAN_DATABASE_HEADER_MAGIC,
    SCAN_DATABASE_HEADER_VERSION,
};

/// Zero-allocation error type for hot-path wire encoders.
pub mod error;
pub use error::WireEncodeErr;

/// Expression tag-and-payload encoder.
///
/// Maps each [`crate::ir::Expr`] variant to a discriminant byte
/// followed by type-specific payload bytes. Keeps the tag
/// assignment table in one place so adding a new expression variant
/// requires a single file change.
///
/// See [`put_expr()`] for the public entry point.
pub(crate) mod put_expr;

/// Node tag-and-payload encoder.
///
/// Maps each [`crate::ir::Node`] variant to a discriminant byte
/// followed by type-specific payload bytes. Control-flow variants
/// recursively encode their body lists via [`put_nodes()`].
///
/// See [`put_node()`] for the public entry point.
pub(crate) mod put_node;

/// Sequence-length encoder.
///
/// Prefixes a node slice with a little-endian `u32` count, then
/// encodes each node in order. This is the canonical way to
/// serialize statement lists (entry bodies, `If` branches, `Loop`
/// bodies, etc.).
///
/// See [`put_nodes()`] for the public entry point.
pub mod put_nodes;

/// Versioned scan database header encoder/decoder.
pub mod scan_database_header;

/// Top-level program encoder.
///
/// Emits the `VIR0` magic, schema version, entry-op id, buffer
/// table, work-group size, and entry body in the order defined by
/// the wire specification. Any schema change must bump
/// [`crate::serial::wire::framing::WIRE_FORMAT_VERSION`] (audit L.1.47).
///
/// See [`to_wire()`] for the public entry point.
pub mod to_wire;
