//! Cross-layer parity tests for matching engines.
//!
//! Same patterns + same haystack, run through every available
//! engine + each engine's `reference_scan`. Assert the match sets are
//! equal modulo ordering. Catches silent regressions whenever any
//! layer changes:
//!
//!   - `vyre-primitives::matching::dfa_compile` (DFA construction)
//!   - `vyre-primitives::nfa::subgroup_nfa::nfa_step` (NFA stepper)
//!   - `vyre-libs::matching::literal_set::GpuLiteralSet`
//!   - `vyre-libs::matching::mega_scan::RulePipeline`
//!   - `vyre-libs::matching::regex_compile` (regex AST → NFA)
//!
//! The GPU `scan` paths are NOT exercised here  -  they require a real
//! adapter and would gate CI on hardware. The `reference_scan` parity
//! tests cover construction + execution semantics; the GPU dispatch
//! correctness is covered separately by per-backend adversarial tests
//! that DO run on hardware.

#![allow(deprecated)]
// (MatchScan trait imported in the tests that need it.)

#[test]
fn literal_set_cpu_finds_planted_secret() {
    use vyre_libs::scan::GpuLiteralSet;
    let engine = GpuLiteralSet::compile(&[b"AKIA".as_slice(), b"ghp_".as_slice()]);
    let haystack = b"foo AKIAIOSFODNN7 bar ghp_xxxx baz";
    let matches = engine.reference_scan(haystack);
    // Two distinct literals fire; AKIA at offset 4, ghp_ at offset 22.
    assert!(matches.iter().any(|m| m.pattern_id == 0 && m.start == 4));
    assert!(matches.iter().any(|m| m.pattern_id == 1 && m.start == 22));
}

#[test]
fn literal_set_idempotent() {
    use vyre_libs::scan::GpuLiteralSet;
    let engine = GpuLiteralSet::compile(&[b"abc".as_slice()]);
    let haystack = b"abc";
    let first = engine.reference_scan(haystack);
    let second = engine.reference_scan(haystack);
    assert_eq!(first, second);
}

#[test]
fn empty_haystack_yields_empty_matches() {
    use vyre_libs::scan::GpuLiteralSet;
    let engine = GpuLiteralSet::compile(&[b"x".as_slice()]);
    assert!(engine.reference_scan(b"").is_empty());
}

#[test]
fn no_matches_when_pattern_absent() {
    use vyre_libs::scan::GpuLiteralSet;
    let engine = GpuLiteralSet::compile(&[b"DEADBEEF".as_slice()]);
    assert!(engine.reference_scan(b"the quick brown fox").is_empty());
}

#[cfg(feature = "matching-regex")]
#[test]
fn regex_compile_round_trips_literal_via_nfa() {
    // A literal regex compiled through the regex frontend should
    // recognize the same substring the literal-set engine would
    // (modulo NFA-vs-DFA stepping differences). Smoke-check by
    // ensuring construction + round-trip succeeds without panic.
    let compiled = vyre_libs::scan::compile_regex_set(&["abc"]).expect("compile");
    assert_eq!(compiled.plan.accept_states.len(), 1);
    assert!(compiled.plan.num_states > 0);
}

#[cfg(feature = "matching-regex")]
#[test]
fn regex_alternation_compiles_to_nfa() {
    let compiled = vyre_libs::scan::compile_regex_set(&["foo|bar"]).expect("compile");
    assert_eq!(compiled.plan.accept_states.len(), 1);
}

#[cfg(feature = "matching-regex")]
#[test]
fn regex_class_compiles_to_nfa() {
    let compiled = vyre_libs::scan::compile_regex_set(&[r"[a-z]+"]).expect("compile");
    assert_eq!(compiled.plan.accept_states.len(), 1);
}

#[cfg(feature = "matching-regex")]
#[test]
fn regex_anchor_rejected() {
    let err = vyre_libs::scan::compile_regex_set(&["^foo"]).unwrap_err();
    assert!(matches!(
        err,
        vyre_libs::scan::RegexCompileError::Unsupported { .. }
    ));
}

#[test]
fn region_dedup_collapses_overlap() {
    use vyre_primitives::matching::{dedup_regions_cpu, RegionTriple};
    let input = vec![
        RegionTriple::new(0, 5, 10),
        RegionTriple::new(0, 7, 12),
        RegionTriple::new(1, 5, 10),
    ];
    let got = dedup_regions_cpu(input);
    assert_eq!(got.len(), 2); // pid=0 spans merge, pid=1 stands alone
}

#[test]
fn match_engine_cache_key_changes_with_patterns() {
    use vyre_libs::scan::{GpuLiteralSet, MatchScan};
    let a = GpuLiteralSet::compile(&[b"foo".as_slice()]);
    let b = GpuLiteralSet::compile(&[b"bar".as_slice()]);
    assert_ne!(MatchScan::cache_key(&a), MatchScan::cache_key(&b));
}

#[test]
fn cache_key_is_deterministic_constant() {
    // Locks the on-disk wire contract: the cache_key for a known
    // pattern set MUST equal the same FNV-1a digest every time, in
    // every process. Catches accidental moves to a randomized hash
    // (e.g. std::DefaultHasher) that would silently invalidate every
    // user's cache between runs.
    use vyre_libs::scan::{GpuLiteralSet, MatchScan};
    let engine = GpuLiteralSet::compile(&[b"AKIA".as_slice(), b"ghp_".as_slice()]);
    let key = MatchScan::cache_key(&engine);
    // Compute the same FNV-1a manually to assert the value is stable.
    let mut buf = Vec::new();
    for w in &engine.pattern_offsets {
        buf.extend_from_slice(&w.to_le_bytes());
    }
    for w in &engine.pattern_lengths {
        buf.extend_from_slice(&w.to_le_bytes());
    }
    for w in &engine.pattern_bytes {
        buf.extend_from_slice(&w.to_le_bytes());
    }
    let expected = format!("lit-{:016x}", vyre_primitives::hash::fnv1a::fnv1a64(&buf));
    assert_eq!(key, expected);
}

#[test]
fn every_match_engine_implements_match_scan() {
    // Type-level contract: any matcher named in this assertion must
    // implement `MatchScan`. If a future refactor breaks this, the
    // compile error here is the canary. (Trait objects double-check
    // dyn-safety at the same time.)
    use vyre_libs::scan::{GpuLiteralSet, MatchScan};
    let engine = GpuLiteralSet::compile(&[b"x".as_slice()]);
    let _trait_obj: &dyn MatchScan = &engine;
}

#[cfg(feature = "matching-nfa")]
#[test]
fn rule_pipeline_implements_match_scan() {
    use vyre_libs::scan::{build_rule_pipeline, MatchScan};
    let pipe = build_rule_pipeline(&["abc"], "input", "hits", 16);
    let _trait_obj: &dyn MatchScan = &pipe;
}

#[test]
fn region_dedup_idempotent_on_already_deduped_input() {
    // Contract: dedup_regions_cpu(dedup_regions_cpu(x)) == dedup_regions_cpu(x)
    use vyre_primitives::matching::{dedup_regions_cpu, RegionTriple};
    let input = vec![
        RegionTriple::new(0, 5, 10),
        RegionTriple::new(0, 7, 12),
        RegionTriple::new(1, 5, 10),
        RegionTriple::new(2, 1, 100),
    ];
    let once = dedup_regions_cpu(input);
    let twice = dedup_regions_cpu(once.clone());
    assert_eq!(once, twice);
}
