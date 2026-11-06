//! Hidden tests: a transactional store whose inverses live in an undo journal.
//! Nested commits fold into the parent frame; nested rollbacks are isolated;
//! rollback replays inverses in reverse order.

use txnkv::{Store, StoreError};

fn keys(s: &Store) -> Vec<String> {
    s.keys().into_iter().collect()
}

#[test]
fn construct() {
    // passes pre-fix: the types exist and an empty store has no keys
    let s = Store::new();
    assert!(keys(&s).is_empty());
    assert!(!s.in_transaction());
}

#[test]
fn autocommit_without_transaction() {
    let mut s = Store::new();
    s.set("a", "1");
    assert_eq!(s.get("a"), Ok("1"));
    assert_eq!(s.rollback(), Err(StoreError::NoTransaction));
    assert_eq!(s.get("a"), Ok("1"), "autocommitted writes are durable");
}

#[test]
fn rollback_restores_prior_value_and_absence() {
    let mut s = Store::new();
    s.set("a", "1");
    s.begin();
    s.set("a", "2");
    s.set("b", "3");
    assert_eq!(s.get("a"), Ok("2"));
    assert_eq!(s.rollback(), Ok(()));
    assert_eq!(s.get("a"), Ok("1"));
    assert_eq!(s.get("b"), Err(StoreError::MissingKey("b".into())));
}

#[test]
fn rollback_restores_deleted_key() {
    let mut s = Store::new();
    s.set("a", "1");
    s.begin();
    assert_eq!(s.delete("a"), Ok(()));
    assert_eq!(s.get("a"), Err(StoreError::MissingKey("a".into())));
    assert_eq!(s.delete("a"), Err(StoreError::MissingKey("a".into())));
    s.rollback().unwrap();
    assert_eq!(s.get("a"), Ok("1"));
}

#[test]
fn commit_keeps_changes() {
    let mut s = Store::new();
    s.begin();
    s.set("a", "1");
    s.commit().unwrap();
    assert_eq!(s.get("a"), Ok("1"));
    assert_eq!(s.rollback(), Err(StoreError::NoTransaction));
    assert_eq!(s.get("a"), Ok("1"));
}

#[test]
fn nested_commit_then_outer_rollback_undoes_everything() {
    let mut s = Store::new();
    s.set("a", "0");
    s.begin();
    s.set("a", "1");
    s.begin();
    s.set("a", "2");
    s.set("b", "x");
    s.commit().unwrap(); // inner commit folds into the outer frame
    assert_eq!(s.get("a"), Ok("2"));
    assert_eq!(s.get("b"), Ok("x"));
    s.rollback().unwrap(); // outer rollback undoes the inner work too
    assert_eq!(s.get("a"), Ok("0"));
    assert_eq!(s.get("b"), Err(StoreError::MissingKey("b".into())));
}

#[test]
fn nested_inner_rollback_leaves_outer_intact() {
    let mut s = Store::new();
    s.begin();
    s.set("a", "1");
    s.begin();
    s.set("a", "2");
    s.set("b", "2");
    s.rollback().unwrap();
    assert_eq!(s.get("a"), Ok("1"));
    assert_eq!(s.get("b"), Err(StoreError::MissingKey("b".into())));
    s.commit().unwrap();
    assert_eq!(s.get("a"), Ok("1"));
}

#[test]
fn rollback_replays_multiple_writes_in_reverse() {
    let mut s = Store::new();
    s.set("a", "orig");
    s.begin();
    s.set("a", "1");
    s.set("a", "2");
    s.delete("a").unwrap();
    s.set("a", "3");
    s.rollback().unwrap();
    assert_eq!(s.get("a"), Ok("orig"));
}

#[test]
fn commit_without_transaction_errors() {
    let mut s = Store::new();
    assert_eq!(s.commit(), Err(StoreError::NoTransaction));
}

#[test]
fn keys_reflects_transaction_state() {
    let mut s = Store::new();
    s.set("a", "1");
    s.begin();
    s.set("b", "2");
    s.delete("a").unwrap();
    assert_eq!(keys(&s), vec!["b".to_string()]);
    s.rollback().unwrap();
    assert_eq!(keys(&s), vec!["a".to_string()]);
}

#[test]
fn deep_nesting_folds_every_level() {
    let mut s = Store::new();
    s.set("n", "0");
    for i in 1..=4 {
        s.begin();
        s.set("n", &i.to_string());
    }
    for _ in 0..3 {
        s.commit().unwrap();
    }
    assert_eq!(s.get("n"), Ok("4"));
    s.rollback().unwrap();
    assert_eq!(s.get("n"), Ok("0"));
    assert!(!s.in_transaction());
}
