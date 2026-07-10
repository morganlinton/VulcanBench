// Hidden test for oss-itertools-strip-prefix (rust-itertools/itertools PR #1104).
// The Itertools trait must gain `strip_prefix` and `strip_prefix_by`, the
// iterator analogue of str::strip_prefix, plus a public `StripPrefixError`
// struct. On success `Ok` yields the iterator advanced past the prefix; on
// mismatch `Err(StripPrefixError { iterator, prefix, mismatch })` exposes the
// partially-consumed state. Run with `cargo test --test vb_strip_prefix`.

use itertools::{Itertools, StripPrefixError};

#[test]
fn vb_strip_prefix_ok_and_empty() {
    let rest = (1..6).strip_prefix([1, 2]).unwrap().collect_vec();
    assert_eq!(rest, vec![3, 4, 5]);

    // an empty prefix strips nothing
    let all = (1..6)
        .strip_prefix(std::iter::empty::<i32>())
        .unwrap()
        .collect_vec();
    assert_eq!(all, vec![1, 2, 3, 4, 5]);

    // a full-length prefix leaves an empty iterator
    let none = (1..4).strip_prefix([1, 2, 3]).unwrap().collect_vec();
    assert_eq!(none, Vec::<i32>::new());
}

#[test]
fn vb_strip_prefix_mismatch_recovers_state() {
    // (1..6) does not start with [1, 9]: matches 1, then 2 != 9.
    let err: StripPrefixError<_, _, _> = (1..6).strip_prefix([1, 9]).unwrap_err();
    // the failing pair: iterator yielded Some(2), prefix wanted 9
    assert_eq!(err.mismatch, (Some(2), 9));
    // the iterator resumes after the consumed-and-matched prefix item (past 1,2)
    assert_eq!(err.iterator.collect_vec(), vec![3, 4, 5]);
    // the prefix remainder is what was left after the mismatched item
    assert_eq!(err.prefix.collect_vec(), Vec::<i32>::new());
}

#[test]
fn vb_strip_prefix_exhausted_iterator() {
    // iterator shorter than the prefix: exhausted before the prefix completes.
    let err = (1..3).strip_prefix([1, 2, 3]).unwrap_err();
    // first element of the mismatch is None because the iterator ran out
    assert_eq!(err.mismatch, (None, 3));
}

#[test]
fn vb_strip_prefix_by_cross_type_predicate() {
    // strip_prefix_by lets the prefix items differ in type from Self::Item,
    // using an explicit equality predicate.
    let path = ["home", "user", "file"];
    let stripped = path
        .iter()
        .strip_prefix_by(["home", "user"], |a, b| **a == *b)
        .unwrap()
        .collect_vec();
    assert_eq!(stripped, vec![&"file"]);

    // predicate-driven mismatch
    let err = path.iter().strip_prefix_by(["home", "root"], |a, b| **a == *b);
    assert!(err.is_err());
}
