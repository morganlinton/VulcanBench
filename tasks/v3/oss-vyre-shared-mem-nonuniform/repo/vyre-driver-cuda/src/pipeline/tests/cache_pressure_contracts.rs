use super::*;

#[test]
fn materialized_output_cache_prebuilt_entries_match_direct_remember_for_1024_cases() {
    for seed in 0_u32..1024 {
        let input_len = ((seed.wrapping_mul(11) ^ seed.rotate_left(13)) % 96 + 1) as usize;
        let output_len = ((seed.wrapping_mul(31) ^ seed.rotate_left(5)) % 96 + 1) as usize;
        let mut state = seed ^ 0xCACA_5000;
        let mut input = Vec::with_capacity(input_len);
        for index in 0..input_len {
            state = state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
                .rotate_left((index as u32) & 15);
            input.push((state >> ((index & 3) * 8)) as u8);
        }
        let mut output = Vec::with_capacity(output_len);
        for index in 0..output_len {
            state = state
                .wrapping_mul(22_695_477)
                .wrapping_add(1)
                .rotate_left((index as u32) & 7);
            output.push((state ^ seed.rotate_right(index as u32 & 31)) as u8);
        }
        let outputs = vec![output];
        let mut direct = MaterializedPipelineOutputCache::default();
        direct
            .remember(&[input.as_slice()], &outputs)
            .expect("Fix: direct materialized cache remember must fit");
        let mut prebuilt = MaterializedPipelineOutputCache::default();
        let entry = MaterializedPipelineOutputCacheEntry::new(&[input.as_slice()], &outputs)
            .expect("Fix: prebuilt materialized cache entry construction must fit");
        prebuilt
            .remember_entry(entry)
            .expect("Fix: prebuilt materialized cache entry insertion must fit");
        let input_key = materialized_input_key(&[input.as_slice()])
            .expect("Fix: generated materialized input key must fit");
        let mut keyed = MaterializedPipelineOutputCache::default();
        let keyed_entry = MaterializedPipelineOutputCacheEntry::new_with_key(
            &[input.as_slice()],
            &input_key,
            &outputs,
        )
        .expect("Fix: keyed materialized cache entry construction must fit");
        keyed
            .remember_entry(keyed_entry)
            .expect("Fix: keyed materialized cache entry insertion must fit");

        let mut direct_replay = Vec::new();
        let mut prebuilt_replay = Vec::new();
        let mut keyed_replay = Vec::new();
        assert!(
            direct
                .hit_into(&[input.as_slice()], &mut direct_replay)
                .expect("Fix: direct materialized cache hit must fit"),
            "Fix: direct materialized cache must hit for generated case {seed}."
        );
        assert!(
            prebuilt
                .hit_into(&[input.as_slice()], &mut prebuilt_replay)
                .expect("Fix: prebuilt materialized cache hit must fit"),
            "Fix: prebuilt materialized cache must hit for generated case {seed}."
        );
        assert!(
            keyed
                .hit_into(&[input.as_slice()], &mut keyed_replay)
                .expect("Fix: keyed materialized cache hit must fit"),
            "Fix: keyed materialized cache must hit for generated case {seed}."
        );
        assert_eq!(
            prebuilt_replay, direct_replay,
            "Fix: prebuilt materialized cache insertion must preserve exact outputs for generated case {seed}."
        );
        assert_eq!(
            keyed_replay, direct_replay,
            "Fix: keyed materialized cache insertion must preserve exact outputs for generated case {seed}."
        );
        assert_eq!(
            prebuilt.byte_len(),
            direct.byte_len(),
            "Fix: prebuilt materialized cache insertion must preserve byte accounting for generated case {seed}."
        );
        assert_eq!(
            keyed.byte_len(),
            direct.byte_len(),
            "Fix: keyed materialized cache insertion must preserve byte accounting for generated case {seed}."
        );
    }
}

#[test]
fn materialized_output_cache_evicts_oldest_entries_under_generated_pressure() {
    let mut cache = MaterializedPipelineOutputCache::default();
    let total_entries = MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE + 17;
    for seed in 0..total_entries {
        let input = (seed as u32).to_le_bytes().to_vec();
        let outputs = vec![vec![seed as u8; 8]];
        cache
            .remember(&[input.as_slice()], &outputs)
            .expect("Fix: generated materialized output cache pressure insert must fit");
    }

    assert_eq!(
        cache.len(),
        MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE,
        "Fix: materialized output cache must evict oldest entries instead of growing past its bounded lane-cache size."
    );
    assert_eq!(
        cache.byte_len(),
        MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE * (std::mem::size_of::<u32>() + 8),
        "Fix: materialized output cache byte accounting must track evicted entries exactly under generated pressure."
    );

    let evicted_input = 0_u32.to_le_bytes().to_vec();
    let mut evicted_replay = vec![b"sentinel".to_vec()];
    assert!(
        !cache
            .hit_into(&[evicted_input.as_slice()], &mut evicted_replay)
            .expect("Fix: evicted materialized output lookup must stay fallible"),
        "Fix: oldest generated materialized output entry must be evicted when capacity is exceeded."
    );
    assert_eq!(
        evicted_replay,
        vec![b"sentinel".to_vec()],
        "Fix: materialized output cache miss must not mutate caller-owned output buffers."
    );

    let retained_seed = (total_entries - 1) as u32;
    let retained_input = retained_seed.to_le_bytes().to_vec();
    let mut retained_replay = Vec::new();
    assert!(
        cache
            .hit_into(&[retained_input.as_slice()], &mut retained_replay)
            .expect("Fix: retained materialized output lookup must fit"),
        "Fix: newest generated materialized output entry must remain cached after pressure eviction."
    );
    assert_eq!(
        retained_replay,
        vec![vec![retained_seed as u8; 8]],
        "Fix: retained generated materialized output entry must replay exact bytes after evictions."
    );
}

#[test]

fn materialized_output_cache_rejects_oversized_entries_without_polluting_cache() {
    let mut cache = MaterializedPipelineOutputCache::default();
    let input = b"oversized compiled CUDA graph replay input";
    let outputs = vec![vec![
        0xA5;
        MAX_MATERIALIZED_OUTPUT_CACHE_BYTES_PER_PIPELINE + 1
    ]];

    cache
        .remember(&[input.as_slice()], &outputs)
        .expect("Fix: oversized materialized output cache entry should be a typed no-admission path, not an allocation or dispatch failure.");

    assert_eq!(
        cache.len(),
        0,
        "Fix: oversized materialized output cache entries must not evict useful entries or consume cache slots."
    );
    assert_eq!(
        cache.byte_len(),
        0,
        "Fix: oversized materialized output cache entries must not perturb byte accounting."
    );
    let mut replay = Vec::new();
    assert!(
        !cache
            .hit_into(&[input.as_slice()], &mut replay)
            .expect("Fix: oversized no-admission lookup must remain fallible"),
        "Fix: oversized materialized output cache entries must not be observable as hits."
    );
}

#[test]
fn materialized_output_cache_preflights_oversized_entries_before_owning_bytes() {
    let input = b"oversized compiled CUDA graph replay input";
    let outputs = vec![vec![
        0xCC;
        MAX_MATERIALIZED_OUTPUT_CACHE_BYTES_PER_PIPELINE + 1
    ]];

    assert!(
        MaterializedPipelineOutputCacheEntry::new_if_cacheable(&[input.as_slice()], &outputs)
            .expect("Fix: oversized materialized cache preflight must be a typed no-admission path.")
            .is_none(),
        "Fix: oversized materialized cache entries must be rejected before constructing owned cache entries."
    );

    let source = include_str!("../materialized_cache.rs");
    let preflight_constructor = source
        .split("pub(crate) fn new_if_cacheable")
        .nth(1)
        .expect("Fix: materialized cache must expose a preflight constructor.")
        .split("pub(crate) fn new(")
        .next()
        .expect("Fix: preflight constructor must precede the fallible owning constructor.");
    assert!(
        preflight_constructor.contains("materialized_cache_entry_byte_len_if_admissible")
            && !preflight_constructor.contains("clone_materialized_cache_bytes"),
        "Fix: materialized CUDA replay cache must compute admissibility before cloning input/output bytes."
    );
}
