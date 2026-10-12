//! Engine tests for the front-end-agnostic borrow checker (`vyre_libs::borrowck`).
//!
//! Hand-built `BorrowFacts` drive the dataflow engine directly (no front-end),
//! proving the spine in isolation. The branch cases show the engine is correct
//! across control flow: borrows live across a branch point conflict, borrows
//! confined to mutually exclusive branches do not.

#![forbid(unsafe_code)]

use vyre_libs::borrowck::{analyze, BorrowFacts, ConflictKind, LoanKind};

/// Build two-or-more-loan facts. Each loan is `(place, kind, issue_point, offset)`.
fn facts(
    point_count: u32,
    cfg: &[(u32, u32)],
    loans: &[(u32, LoanKind, u32, u32)],
    uses: &[(u32, u32)],
) -> BorrowFacts {
    BorrowFacts {
        point_count,
        cfg_edges: cfg.to_vec(),
        loan_place: loans.iter().map(|l| l.0).collect(),
        loan_kind: loans.iter().map(|l| l.1).collect(),
        loan_issued_at: loans.iter().map(|l| l.2).collect(),
        loan_offset: loans.iter().map(|l| l.3).collect(),
        loan_used_at: uses.to_vec(),
    }
}

#[test]
fn straight_line_two_mutable_borrows_conflict() {
    // 0: issue a(&mut x)  1: issue b(&mut x)  2: use a, use b
    let f = facts(
        3,
        &[(0, 1), (1, 2)],
        &[(0, LoanKind::Mut, 0, 10), (0, LoanKind::Mut, 1, 20)],
        &[(0, 2), (1, 2)],
    );
    let c = analyze(&f);
    assert_eq!(c.len(), 1, "expected one conflict, got {c:?}");
    assert_eq!(c[0].kind, ConflictKind::TwoMutable);
    assert_eq!(c[0].offset, 20, "the later loan is the access that errors");
}

#[test]
fn straight_line_mutable_and_shared_conflict() {
    let f = facts(
        3,
        &[(0, 1), (1, 2)],
        &[(0, LoanKind::Shared, 0, 10), (0, LoanKind::Mut, 1, 20)],
        &[(0, 2), (1, 2)],
    );
    let c = analyze(&f);
    assert_eq!(c.len(), 1, "got {c:?}");
    assert_eq!(c[0].kind, ConflictKind::MutableAndShared);
}

#[test]
fn two_shared_borrows_do_not_conflict() {
    let f = facts(
        3,
        &[(0, 1), (1, 2)],
        &[(0, LoanKind::Shared, 0, 10), (0, LoanKind::Shared, 1, 20)],
        &[(0, 2), (1, 2)],
    );
    assert!(analyze(&f).is_empty(), "two shared borrows are allowed");
}

#[test]
fn unused_first_mutable_borrow_is_dead() {
    // a issued at 0 but never used; b issued at 1, used at 2.
    let f = facts(
        3,
        &[(0, 1), (1, 2)],
        &[(0, LoanKind::Mut, 0, 10), (0, LoanKind::Mut, 1, 20)],
        &[(1, 2)],
    );
    assert!(
        analyze(&f).is_empty(),
        "an unused borrow is dead immediately (NLL)"
    );
}

#[test]
fn sequential_non_overlapping_mutable_borrows() {
    // a issued 0 used 1; b issued 2 used 3. Non-overlapping live ranges.
    let f = facts(
        4,
        &[(0, 1), (1, 2), (2, 3)],
        &[(0, LoanKind::Mut, 0, 10), (0, LoanKind::Mut, 2, 20)],
        &[(0, 1), (1, 3)],
    );
    assert!(
        analyze(&f).is_empty(),
        "non-overlapping &mut borrows are allowed (NLL)"
    );
}

#[test]
fn borrows_live_across_a_branch_conflict() {
    // Both issued before the branch (0,1), then used in separate arms (3,4).
    // At point 1 (b's issue) a is still live -> conflict, matching rustc.
    //   0:issue a  1:issue b  2:branch  3:use a  4:use b  5:join
    let f = facts(
        6,
        &[(0, 1), (1, 2), (2, 3), (2, 4), (3, 5), (4, 5)],
        &[(0, LoanKind::Mut, 0, 10), (0, LoanKind::Mut, 1, 20)],
        &[(0, 3), (1, 4)],
    );
    let c = analyze(&f);
    assert_eq!(
        c.len(),
        1,
        "borrows live across the branch point must conflict, got {c:?}"
    );
    assert_eq!(c[0].kind, ConflictKind::TwoMutable);
}

#[test]
fn borrows_in_mutually_exclusive_branches_do_not_conflict() {
    // a issued+used only in the then-arm, b only in the else-arm: never co-live.
    //   0:branch  1:issue a  2:use a  3:issue b  4:use b  5:join
    let f = facts(
        6,
        &[(0, 1), (1, 2), (2, 5), (0, 3), (3, 4), (4, 5)],
        &[(0, LoanKind::Mut, 1, 10), (0, LoanKind::Mut, 3, 20)],
        &[(0, 2), (1, 4)],
    );
    assert!(
        analyze(&f).is_empty(),
        "borrows confined to mutually exclusive branches must not conflict"
    );
}

#[test]
fn scales_to_a_long_chain() {
    // 2000-point straight chain; two &mut of one place issued early, both used
    // at the end -> live across the whole chain -> one conflict, computed fast.
    let n = 2000u32;
    let cfg: Vec<(u32, u32)> = (0..n - 1).map(|i| (i, i + 1)).collect();
    let f = facts(
        n,
        &cfg,
        &[(0, LoanKind::Mut, 0, 10), (0, LoanKind::Mut, 1, 20)],
        &[(0, n - 1), (1, n - 1)],
    );
    let c = analyze(&f);
    assert_eq!(c.len(), 1, "got {c:?}");
    assert_eq!(c[0].kind, ConflictKind::TwoMutable);
}

#[test]
fn scales_with_many_distinct_place_loans() {
    // 500 loans, each over a distinct place, used once along a chain: the
    // O(loans^2) pairing short-circuits on the place check, no conflicts.
    let count = 500u32;
    let n = count + 1;
    let cfg: Vec<(u32, u32)> = (0..n - 1).map(|i| (i, i + 1)).collect();
    let loans: Vec<(u32, LoanKind, u32, u32)> =
        (0..count).map(|i| (i, LoanKind::Mut, i, i * 4)).collect();
    let uses: Vec<(u32, u32)> = (0..count).map(|i| (i, i + 1)).collect();
    let f = facts(n, &cfg, &loans, &uses);
    assert!(
        analyze(&f).is_empty(),
        "distinct-place loans never conflict"
    );
}
