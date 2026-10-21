use super::*;
use vyre_foundation::match_result::Match;

mod count_prefilter_generated;

#[test]
fn irregular_haystack_plants_unaligned_varied_literals() {
    let (haystack, planted) = build_irregular_haystack(128 * 1024);
    let ac = classic_ac_compile(PATTERNS);
    let pattern_lengths = pattern_lengths().unwrap();
    let matches = cpu_bounded_range_matches(&ac, &pattern_lengths, &haystack);

    assert!(planted > PATTERNS.len() as u32);
    assert!(matches.len() >= planted as usize);
    assert!(matches.iter().any(|hit| hit.start % 4 != 0));
    assert!(
        pattern_lengths.iter().max().copied().unwrap() > 16,
        "fixture must include long parser/security literals"
    );
}

#[test]
fn prepare_builds_cuda_compatible_bounded_ranges_program() {
    let prepared = prepare_scan_ac_irregular(None).unwrap();

    assert_eq!(prepared.program.workgroup_size(), [128, 1, 1]);
    assert_eq!(prepared.reset_program.workgroup_size(), [1, 1, 1]);
    assert_eq!(prepared.inputs.len(), 10);
    assert_eq!(MATCH_COUNT_INPUT_INDEX, 6);
    assert_eq!(CANDIDATE_END_MASK_INPUT_INDEX, 7);
    assert_eq!(CANDIDATE_SUFFIX2_MASK_INPUT_INDEX, 8);
    assert_eq!(
        CANDIDATE_SUFFIX3_BLOOM_INPUT_INDEX,
        prepared.inputs.len() - 1
    );
    assert_eq!(
        SCAN_RESOURCE_INDICES[MATCH_COUNT_INPUT_INDEX],
        MATCH_COUNT_INPUT_INDEX
    );
    assert_eq!(
        SCAN_RESOURCE_INDICES[MATCHES_RESOURCE_INDEX],
        MATCHES_RESOURCE_INDEX
    );
    assert_eq!(prepared.program.buffers()[7].name(), "candidate_end_mask");
    assert_eq!(
        prepared.program.buffers()[8].name(),
        "candidate_suffix2_mask"
    );
    assert_eq!(
        prepared.program.buffers()[9].name(),
        "candidate_suffix3_bloom"
    );
    assert_eq!(prepared.program.buffers()[10].name(), "matches");
    assert_eq!(
        match_triples_output_bytes(MAX_MATCHES).unwrap(),
        MAX_MATCHES as usize * 12
    );
    let selected_match_bytes =
        match_triples_readback_bytes(prepared.stats.expected_matches).unwrap();
    let matches_output = prepared
        .program
        .buffers()
        .iter()
        .find(|buffer| buffer.name() == "matches")
        .expect("bounded-ranges program must expose matches output");
    assert_eq!(
        matches_output.output_byte_range(),
        Some(0..selected_match_bytes)
    );
    assert!(
        selected_match_bytes < match_triples_output_bytes(MAX_MATCHES).unwrap(),
        "fixture must prove compact match readback avoids capacity-sized transfer"
    );
    assert_eq!(
        selected_scan_output_bytes(prepared.stats),
        4 + selected_match_bytes as u64
    );
    assert_eq!(prepared.stats.haystack_bytes, HAYSTACK_BYTES as u32);
    assert_eq!(prepared.stats.patterns, PATTERNS.len() as u32);
    assert!(prepared.stats.expected_matches > 0);
    assert!(prepared.stats.expected_matches <= prepared.stats.max_matches);
    assert!(prepared.stats.candidate_end_bytes > 0);
    assert!(prepared.stats.candidate_end_lanes > prepared.stats.expected_matches);
    assert!(prepared.stats.candidate_end_lanes < prepared.stats.haystack_bytes / 8);
    assert!(prepared.stats.candidate_suffix2_lanes > 0);
    assert!(prepared.stats.candidate_suffix2_lanes <= prepared.stats.candidate_end_lanes);
    assert!(prepared.stats.candidate_suffix3_lanes > 0);
    assert!(prepared.stats.candidate_suffix3_lanes <= prepared.stats.candidate_suffix2_lanes);
    assert!(prepared.stats.candidate_suffix3_lanes < prepared.stats.candidate_end_lanes / 10);
}

#[test]
fn count_prepare_builds_compact_cardinality_program() {
    let prepared = count::prepare_scan_ac_irregular_count(None).unwrap();

    assert_eq!(prepared.program.workgroup_size(), [128, 1, 1]);
    assert_eq!(prepared.inputs.len(), 8);
    assert_eq!(prepared.baseline_output.len(), 4);
    assert_eq!(prepared.stats.haystack_bytes, HAYSTACK_BYTES as u32);
    assert_eq!(prepared.stats.patterns, PATTERNS.len() as u32);
    assert!(prepared.stats.expected_matches > 0);
    assert_eq!(prepared.program.buffers()[3].name(), "candidate_end_mask");
    assert_eq!(
        prepared.program.buffers()[4].name(),
        "candidate_suffix2_mask"
    );
    assert_eq!(
        prepared.program.buffers()[5].name(),
        "candidate_suffix3_bloom"
    );
    assert_eq!(prepared.program.buffers()[7].name(), "match_count");
    assert!(prepared.stats.candidate_end_bytes > 0);
    assert!(prepared.stats.candidate_end_bytes < 32);
    assert!(prepared.stats.candidate_end_lanes > 0);
    assert!(prepared.stats.candidate_end_lanes < prepared.stats.haystack_bytes / 2);
    assert!(prepared.stats.candidate_suffix2_lanes > 0);
    assert!(prepared.stats.candidate_suffix2_lanes <= prepared.stats.candidate_end_lanes);
    assert!(prepared.stats.candidate_suffix3_lanes > 0);
    assert!(prepared.stats.candidate_suffix3_lanes <= prepared.stats.candidate_suffix2_lanes);
    assert_eq!(
        selected_scan_output_bytes(prepared.stats),
        4 + u64::from(prepared.stats.expected_matches) * 12
    );
    let full_scan = prepare_scan_ac_irregular(None).unwrap();
    let removed_emit_tables =
        u64::from(full_scan.stats.output_records + full_scan.stats.patterns) * 4;
    assert_eq!(
        prepared.input_bytes_total,
        full_scan.input_bytes_total - removed_emit_tables,
        "count-only preflight should share all bounded-ranges suffix masks and remove only emit tables"
    );
}

#[test]
fn aho_cpu_baseline_matches_classic_bounded_ranges_oracle() {
    let (haystack, _) = build_irregular_haystack(32 * 1024);
    let ac = classic_ac_compile(PATTERNS);
    let pattern_lengths = pattern_lengths().unwrap();
    let mut classic = cpu_bounded_range_matches(&ac, &pattern_lengths, &haystack);
    classic.sort_unstable();

    let aho = cpu_aho_overlapping_matches(PATTERNS, &haystack).unwrap();

    assert_eq!(aho, classic);
}

#[test]
fn bounded_ranges_program_reference_eval_matches_cpu_oracle() {
    let (haystack, _) = build_irregular_haystack(4 * 1024);
    let ac = classic_ac_compile(PATTERNS);
    let pattern_lengths = pattern_lengths().unwrap();
    let mut expected = cpu_bounded_range_matches(&ac, &pattern_lengths, &haystack);
    expected.sort_unstable();
    let candidate_end_mask = classic_ac_candidate_end_byte_mask_words(&ac.dfa);
    let candidate_suffix2_mask = classic_ac_candidate_suffix2_mask_words(&ac.dfa);
    let candidate_suffix3_bloom = classic_ac_candidate_suffix3_bloom_words(PATTERNS);
    let program = try_build_ac_bounded_ranges_suffix3_prefilter_program_ext(
        &ac.dfa,
        pattern_lengths.len() as u32,
        4096,
        false,
    )
    .unwrap();
    let inputs = scan_ac_inputs(
        &ac,
        &pattern_lengths,
        &haystack,
        &candidate_end_mask,
        &candidate_suffix2_mask,
        &candidate_suffix3_bloom,
    );
    let values = inputs
        .into_iter()
        .map(vyre_reference::value::Value::from)
        .collect::<Vec<_>>();
    let outputs = vyre_reference::reference_eval(&program, &values)
        .unwrap()
        .into_iter()
        .map(|value| value.to_bytes())
        .collect::<Vec<_>>();
    let actual = decode_scan_outputs(&outputs, "reference").unwrap();

    assert_eq!(actual, expected);
}

fn with_reference_dispatch_lanes(program: Program, lanes: u32) -> Program {
    let buffers = program
        .buffers()
        .iter()
        .cloned()
        .map(|buffer| {
            if buffer.name() == "match_count" {
                buffer.with_count(lanes.max(1)).with_output_byte_range(0..4)
            } else {
                buffer
            }
        })
        .collect();
    program.with_rewritten_buffers(buffers)
}

#[test]
fn bounded_count_program_reference_eval_matches_cpu_cardinality() {
    let (haystack, _) = build_irregular_haystack(64 * 1024);
    let ac = classic_ac_compile(PATTERNS);
    let expected = cpu_aho_overlapping_matches(PATTERNS, &haystack)
        .unwrap()
        .len() as u32;
    let program = with_reference_dispatch_lanes(
        vyre_libs::scan::classic_ac::build_ac_bounded_count_suffix3_prefilter_program(&ac.dfa),
        haystack.len() as u32,
    );
    let mut inputs = count::scan_ac_count_inputs(&ac, &haystack);
    inputs[7] = vec![0_u8; haystack.len() * 4];
    let values = inputs
        .into_iter()
        .map(vyre_reference::value::Value::from)
        .collect::<Vec<_>>();
    let outputs = vyre_reference::reference_eval(&program, &values)
        .unwrap()
        .into_iter()
        .map(|value| value.to_bytes())
        .collect::<Vec<_>>();

    assert_eq!(outputs, vec![pack_u32_slice(&[expected])]);
}

#[test]
fn decoded_outputs_canonicalize_atomic_order_variation() {
    let first = Match::new(0, 3, 7);
    let second = Match::new(1, 11, 15);
    let actual = vec![pack_u32_slice(&[2]), encode_match_triples(&[second, first])];

    let decoded = decode_scan_outputs(&actual, "actual").unwrap();
    assert_eq!(decoded, vec![first, second]);
}

#[test]
fn compact_match_readback_still_rejects_over_count_outputs() {
    let only_match = Match::new(0, 3, 7);
    let actual = vec![pack_u32_slice(&[2]), encode_match_triples(&[only_match])];

    let error = decode_scan_outputs(&actual, "compact actual")
        .expect_err("compact match readback must reject count values beyond returned triples");
    assert!(
        error.to_string().contains("match triples failed to decode"),
        "decode error should describe the truncated compact match payload: {error}"
    );
}
