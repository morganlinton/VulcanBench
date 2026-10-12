//! Lego-block decision-table examples for `tests/SKILL.md`.
//!
//! Every row of the SKILL.md "Decision tables  -  picking a matching
//! primitive" section gets one runnable test here. Failing test = the
//! decision table now lies about a primitive's behaviour. Refresh the
//! table or fix the primitive  -  never both at once.

#![allow(deprecated)]
#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
use vyre_libs::scan::{
    cached_load_or_compile, dedup_regions_inplace, dedup_regions_reference, engine_cache_path,
    pack_haystack_u32, pack_u32_slice, scan_guard, unpack_match_triples, GpuLiteralSet,
    RegionTriple, DEFAULT_MAX_SCAN_BYTES,
};

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
#[test]
fn skill_md_dispatch_helpers_compile_and_run() {
    // pack_haystack_u32: pack 4 bytes into one little-endian u32.
    let packed = pack_haystack_u32(b"abcd");
    assert_eq!(packed, vec![b'a', b'b', b'c', b'd']);

    // pack_u32_slice: write LE bytes for u32 words.
    let words = [0x01020304u32];
    assert_eq!(pack_u32_slice(&words), vec![0x04, 0x03, 0x02, 0x01]);

    // scan_guard returns the validated length.
    let len = scan_guard(b"hello", "skill", DEFAULT_MAX_SCAN_BYTES).expect("under ceiling");
    assert_eq!(len, 5);

    // unpack_match_triples decodes a packed triple buffer.
    let triple_bytes: Vec<u8> = pack_u32_slice(&[1, 0, 4]);
    let matches = unpack_match_triples(&triple_bytes, 1);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern_id, 1);
    assert_eq!(matches[0].start, 0);
    assert_eq!(matches[0].end, 4);
}

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
#[test]
fn skill_md_region_dedup_examples_match_table() {
    // dedup_regions_reference: returns a fresh, sorted, coalesced vector.
    let owned = vec![
        RegionTriple::new(0, 5, 10),
        RegionTriple::new(0, 7, 12),
        RegionTriple::new(1, 3, 4),
    ];
    let deduped = dedup_regions_reference(owned);
    assert_eq!(deduped.len(), 2, "same-pid overlap collapses to one span");

    // dedup_regions_inplace: zero-alloc compaction.
    let mut buf = vec![RegionTriple::new(0, 0, 5), RegionTriple::new(0, 0, 5)];
    dedup_regions_inplace(&mut buf);
    assert_eq!(buf.len(), 1, "exact duplicate collapses");
}

#[cfg(any(
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "matching-nfa"
))]
#[test]
fn skill_md_cache_helpers_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cache_dir = tmp.path();
    let key = "skill-md-test-engine";

    // engine_cache_path returns Some(<dir>/<key>.bin).
    let path = engine_cache_path(cache_dir, key).expect("path");
    assert!(path.ends_with(format!("{key}.bin")));

    // First call compiles + saves.
    let engine_a: GpuLiteralSet = cached_load_or_compile(cache_dir, key, || {
        GpuLiteralSet::compile(&[b"AKIA".as_slice()])
    });
    assert!(!engine_a.pattern_lengths.is_empty());

    // Second call hits the warm cache.
    let engine_b: GpuLiteralSet = cached_load_or_compile(cache_dir, key, || {
        unreachable!("second call must hit the warm cache, not recompile")
    });
    assert_eq!(engine_a.pattern_lengths, engine_b.pattern_lengths);
}

#[cfg(feature = "matching-nfa")]
#[test]
fn skill_md_rule_pipeline_cpu_finds_documented_match() {
    use vyre_libs::scan::build_rule_pipeline;
    // Decision-table row: "RulePipeline / mega_scan  -  regex (NFA)".
    let pipe = build_rule_pipeline(&["abc"], "input", "hits", 16);
    let matches = pipe.reference_scan(b"xxabcxx");
    assert!(
        matches.iter().any(|m| m.start == 2 && m.end == 5),
        "RulePipeline must find 'abc' at bytes 2..5"
    );
}

#[cfg(feature = "matching-regex")]
#[test]
fn skill_md_compile_regex_set_round_trips() {
    use vyre_libs::scan::compile_regex_set;
    // Decision-table row: "compile_regex_set  -  regex set → RulePipeline".
    let set = compile_regex_set(&["AKIA[A-Z0-9]{4}"]).expect("compile");
    assert_eq!(set.plan.accept_states.len(), 1);
}

#[cfg(feature = "matching-substring")]
#[test]
fn skill_md_substring_search_emits_program() {
    use vyre::ir::Node;
    use vyre_libs::scan::substring_search;
    use vyre_libs::scan::SCAN_SUBSTRING_OP_ID;
    // Decision-table row: "substring_search  -  one literal needle".
    let prog = substring_search("input", "needle", "matches", 64, 4);
    let [Node::Region { generator, .. }] = prog.entry() else {
        panic!("substring_search must emit one scan region");
    };
    assert_eq!(generator.as_str(), SCAN_SUBSTRING_OP_ID);
}

#[cfg(feature = "matching-dfa")]
#[test]
fn skill_md_aho_corasick_emits_program() {
    use vyre::ir::Node;
    use vyre_libs::scan::{aho_corasick, dfa_compile};
    // Decision-table row: "aho_corasick  -  many literals".
    let dfa = dfa_compile(&[b"AKIA".as_slice(), b"ghp_".as_slice()]);
    let prog = aho_corasick(
        "input",
        "transitions",
        "accept_mask",
        "matches",
        128,
        dfa.state_count,
    );
    let [Node::Region { generator, .. }] = prog.entry() else {
        panic!("aho_corasick must emit one scan region");
    };
    assert_eq!(generator.as_str(), "vyre-libs::matching::aho_corasick");
}
