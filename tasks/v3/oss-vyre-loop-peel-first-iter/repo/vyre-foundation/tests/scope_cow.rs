//! Tests for the COW `Scope` generic map used by validation and lowering.
//!
//! `Scope` must support nested `child()` scopes that inherit parent
//! bindings but do not leak mutations back up.

use vyre_foundation::ir::model::program::Scope;

#[test]
fn new_scope_is_empty() {
    let scope: Scope<String, i32> = Scope::new();
    assert!(scope.is_empty());
    assert_eq!(scope.len(), 0);
}

#[test]
fn insert_and_get_roundtrip() {
    let mut scope = Scope::new();
    assert_eq!(scope.insert("x".to_string(), 42), None);
    assert_eq!(scope.get("x"), Some(&42));
}

#[test]
fn insert_returns_old_value() {
    let mut scope = Scope::new();
    let _ = scope.insert("x".to_string(), 1);
    assert_eq!(scope.insert("x".to_string(), 2), Some(1));
    assert_eq!(scope.get("x"), Some(&2));
}

#[test]
fn contains_key_finds_existing() {
    let mut scope = Scope::new();
    let _ = scope.insert("x".to_string(), 42);
    assert!(scope.contains_key("x"));
}

#[test]
fn contains_key_returns_false_for_missing() {
    let scope: Scope<String, i32> = Scope::new();
    assert!(!scope.contains_key("x"));
}

#[test]
fn child_inherits_parent_bindings() {
    let mut parent = Scope::new();
    let _ = parent.insert("x".to_string(), 42);
    let child = parent.child();
    assert_eq!(child.get("x"), Some(&42));
}

#[test]
fn child_insert_does_not_affect_parent() {
    let mut parent = Scope::new();
    let _ = parent.insert("x".to_string(), 42);
    let mut child = parent.child();
    let _ = child.insert("x".to_string(), 99);
    assert_eq!(parent.get("x"), Some(&42));
    assert_eq!(child.get("x"), Some(&99));
}

#[test]
fn child_insert_new_does_not_affect_parent() {
    let mut parent = Scope::new();
    let _ = parent.insert("x".to_string(), 42);
    let mut child = parent.child();
    let _ = child.insert("y".to_string(), 99);
    assert_eq!(parent.get("y"), None);
    assert_eq!(child.get("y"), Some(&99));
}

#[test]
fn grandchild_inherits_grandparent() {
    let mut gparent = Scope::new();
    let _ = gparent.insert("x".to_string(), 1);
    let parent = gparent.child();
    let grandchild = parent.child();
    assert_eq!(grandchild.get("x"), Some(&1));
}

#[test]
fn from_map_preserves_entries() {
    let mut map = std::collections::HashMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    let scope = Scope::from_map(map);
    assert_eq!(scope.len(), 2);
    assert_eq!(scope.get("a"), Some(&1));
    assert_eq!(scope.get("b"), Some(&2));
}

#[test]
fn child_of_empty_is_empty() {
    let parent: Scope<String, i32> = Scope::new();
    let child = parent.child();
    assert!(child.is_empty());
}

#[test]
fn len_counts_only_current_scope() {
    let mut parent = Scope::new();
    let _ = parent.insert("x".to_string(), 1);
    let mut child = parent.child();
    let _ = child.insert("y".to_string(), 2);
    assert_eq!(parent.len(), 1);
    assert_eq!(child.len(), 2);
}
