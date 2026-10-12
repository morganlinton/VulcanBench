//! Tests for `MemoryOrdering` wire-tag round-trip and validity predicates.
//!
//! Memory ordering is part of the atomic and barrier contracts; a
//! mis-mapped tag silently changes synchronization semantics.

use vyre::MemoryOrdering;

#[test]
fn relaxed_wire_tag_roundtrips() {
    let tag = MemoryOrdering::Relaxed.wire_tag();
    assert_eq!(
        MemoryOrdering::from_wire_tag(tag).unwrap(),
        MemoryOrdering::Relaxed
    );
}

#[test]
fn acquire_wire_tag_roundtrips() {
    let tag = MemoryOrdering::Acquire.wire_tag();
    assert_eq!(
        MemoryOrdering::from_wire_tag(tag).unwrap(),
        MemoryOrdering::Acquire
    );
}

#[test]
fn release_wire_tag_roundtrips() {
    let tag = MemoryOrdering::Release.wire_tag();
    assert_eq!(
        MemoryOrdering::from_wire_tag(tag).unwrap(),
        MemoryOrdering::Release
    );
}

#[test]
fn acq_rel_wire_tag_roundtrips() {
    let tag = MemoryOrdering::AcqRel.wire_tag();
    assert_eq!(
        MemoryOrdering::from_wire_tag(tag).unwrap(),
        MemoryOrdering::AcqRel
    );
}

#[test]
fn seq_cst_wire_tag_roundtrips() {
    let tag = MemoryOrdering::SeqCst.wire_tag();
    assert_eq!(
        MemoryOrdering::from_wire_tag(tag).unwrap(),
        MemoryOrdering::SeqCst
    );
}

#[test]
fn from_wire_tag_rejects_unknown() {
    let err = MemoryOrdering::from_wire_tag(255).unwrap_err();
    assert!(err.contains("Fix:"));
}

#[test]
fn all_tags_are_unique() {
    let orderings = [
        MemoryOrdering::Relaxed,
        MemoryOrdering::Acquire,
        MemoryOrdering::Release,
        MemoryOrdering::AcqRel,
        MemoryOrdering::SeqCst,
    ];
    let mut tags: Vec<u8> = orderings.iter().map(|o| o.wire_tag()).collect();
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(
        tags.len(),
        orderings.len(),
        "every MemoryOrdering must have a unique wire tag"
    );
}

#[test]
fn relaxed_valid_for_atomic_rmw() {
    assert!(MemoryOrdering::Relaxed.is_valid_for_atomic_rmw());
}

#[test]
fn acquire_valid_for_atomic_rmw() {
    assert!(MemoryOrdering::Acquire.is_valid_for_atomic_rmw());
}

#[test]
fn release_valid_for_atomic_rmw() {
    assert!(MemoryOrdering::Release.is_valid_for_atomic_rmw());
}

#[test]
fn acq_rel_valid_for_atomic_rmw() {
    assert!(MemoryOrdering::AcqRel.is_valid_for_atomic_rmw());
}

#[test]
fn seq_cst_valid_for_atomic_rmw() {
    assert!(MemoryOrdering::SeqCst.is_valid_for_atomic_rmw());
}

#[test]
fn relaxed_not_valid_for_barrier() {
    assert!(!MemoryOrdering::Relaxed.is_valid_for_barrier());
}

#[test]
fn acquire_valid_for_barrier() {
    assert!(MemoryOrdering::Acquire.is_valid_for_barrier());
}

#[test]
fn release_valid_for_barrier() {
    assert!(MemoryOrdering::Release.is_valid_for_barrier());
}

#[test]
fn acq_rel_valid_for_barrier() {
    assert!(MemoryOrdering::AcqRel.is_valid_for_barrier());
}

#[test]
fn seq_cst_valid_for_barrier() {
    assert!(MemoryOrdering::SeqCst.is_valid_for_barrier());
}

#[test]
fn only_grid_sync_requires_grid_sync() {
    assert!(!MemoryOrdering::Relaxed.requires_grid_sync());
    assert!(!MemoryOrdering::Acquire.requires_grid_sync());
    assert!(!MemoryOrdering::Release.requires_grid_sync());
    assert!(!MemoryOrdering::AcqRel.requires_grid_sync());
    assert!(!MemoryOrdering::SeqCst.requires_grid_sync());
}
