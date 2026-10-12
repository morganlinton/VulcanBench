//! Memory / data-layout catalog (Phase 4C).
//!
//! Buffer + load/store rewrites: const-buffer folding, dead-buffer
//! elimination, vector/coalescing layout hint promotion, and the
//! decode→scan storage-to-workgroup handoff fusion.

/// Compile-time constant-buffer load folding.
pub mod const_buffer_fold;
/// Remove declared buffers that cannot affect any output.
pub mod dead_buffer_elim;
/// Drop sibling `Node::Store` whose value is overwritten before any
/// reader can observe it (ROADMAP A20).
pub mod dead_store_elim;
/// Decode→scan storage-to-workgroup handoff fusion.
pub mod decode_scan_fuse;
/// Hoist `Let(name, Load(ro_buf, idx))` out of common branch
/// prefixes when `ro_buf` is declared `BufferAccess::ReadOnly`
/// (ROADMAP A15  -  buffer-aliasing-fact-aware load elision via
/// the trivial alias proof).
pub mod read_only_load_hoist;
/// Replace `Let(name, Load(b, i))` with the value of an immediately
/// preceding `Store(b, i, v)` when no intervening node could observe
/// or mutate `b` between the two (ROADMAP A22).
pub mod store_to_load_forward;
/// Proven-safe vector/coalescing layout hint promotion.
pub mod vectorization;
