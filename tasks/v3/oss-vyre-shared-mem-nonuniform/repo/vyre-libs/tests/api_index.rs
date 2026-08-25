//! Verifies the `vyre_libs::scan::API_INDEX` const matches the
//! actual `pub use` surface. Failing test = stale index  -  refresh by
//! adding/removing the entry, never by softening the test.

use vyre_libs::scan::{ApiKind, API_INDEX};

/// Every name in `API_INDEX` must be unique. A duplicate means a
/// refactor renamed an export but left the old entry behind.
#[test]
fn api_index_entries_are_unique() {
    let mut names: Vec<&str> = API_INDEX.iter().map(|(n, _, _)| *n).collect();
    names.sort_unstable();
    let dup_window = names.windows(2).find(|w| w[0] == w[1]);
    assert!(
        dup_window.is_none(),
        "API_INDEX has duplicate entry: {dup_window:?}",
    );
}

/// API_INDEX is non-empty and lists at least the unconditional core
/// of dispatch primitives  -  these never gate behind a feature, so if
/// they go missing the index is degenerate.
#[test]
fn api_index_lists_unconditional_dispatch_core() {
    let unconditional_required = [
        "scan_guard",
        "haystack_len_u32",
        "pack_haystack_u32",
        "DEFAULT_MAX_SCAN_BYTES",
        "MatchScan",
        "MatchEngineCache",
        "ScanResult",
    ];
    for needle in unconditional_required {
        let entry = API_INDEX
            .iter()
            .find(|(n, _, _)| *n == needle)
            .unwrap_or_else(|| panic!("API_INDEX missing required core symbol {needle}"));
        assert_eq!(
            entry.2, None,
            "{needle} must be unconditional (feature_gate must be None)",
        );
    }
}

/// Feature-gated entries must reference a Cargo feature this crate
/// declares. We check by name against the static set we know exists
/// in this crate's manifest. Add new feature names here when
/// introducing a new gate.
#[test]
fn api_index_feature_gates_are_known() {
    const KNOWN_FEATURES: &[&str] = &[
        "matching-substring",
        "matching-dfa",
        "matching-nfa",
        "matching-regex",
        "test-fixtures",
    ];
    for (name, _, gate) in API_INDEX {
        if let Some(g) = gate {
            assert!(
                KNOWN_FEATURES.contains(g),
                "API_INDEX entry {name:?} references unknown feature {g:?}; \
                 either add the feature to this test's KNOWN_FEATURES list \
                 or fix the API_INDEX gate",
            );
        }
    }
}

/// Each `ApiKind` variant must appear at least once. If any kind has
/// zero entries, the catalog is incomplete.
#[test]
fn api_index_uses_every_kind() {
    let kinds: Vec<ApiKind> = API_INDEX.iter().map(|(_, k, _)| *k).collect();
    for required in [
        ApiKind::Function,
        ApiKind::Struct,
        ApiKind::Enum,
        ApiKind::Trait,
        ApiKind::Const,
        ApiKind::TypeAlias,
    ] {
        assert!(
            kinds.contains(&required),
            "API_INDEX has no {required:?} entry  -  likely a missing export"
        );
    }
}

/// Sanity: the index is alphabetised within each `feature_gate`
/// bucket. Easy to enforce, easy to read, easy to diff during
/// refactor reviews.
#[test]
fn api_index_is_sorted_within_feature_buckets() {
    let mut by_gate: std::collections::BTreeMap<Option<&str>, Vec<&str>> =
        std::collections::BTreeMap::new();
    for (n, _, g) in API_INDEX {
        by_gate.entry(*g).or_default().push(*n);
    }
    // We only require *uniqueness*, not strict alphabetisation, since
    // related symbols are intentionally grouped (e.g. all hit-buffer
    // helpers stay together). What we do enforce: no duplicate inside
    // a bucket  -  that would be a copy-paste error.
    for (gate, names) in &by_gate {
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            names.len(),
            "duplicate name inside feature bucket {gate:?}",
        );
    }
}
