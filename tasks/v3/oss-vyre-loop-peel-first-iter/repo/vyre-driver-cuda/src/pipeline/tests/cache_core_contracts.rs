use super::*;

#[test]
fn materialized_output_cache_hits_4096_generated_exact_inputs() {
    let mut cache = MaterializedPipelineOutputCache::default();
    for seed in 0_u32..4096 {
        let input_len = ((seed.wrapping_mul(19) ^ seed.rotate_left(3)) % 128 + 1) as usize;
        let output_len = ((seed.wrapping_mul(23) ^ seed.rotate_left(7)) % 128 + 1) as usize;
        let mut state = seed ^ 0xD15C_A11E;
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
            output.push((state ^ seed.rotate_left(index as u32 & 31)) as u8);
        }
        let outputs = vec![output];
        cache
            .remember(&[input.as_slice()], &outputs)
            .expect("Fix: generated materialized CUDA output cache insert must fit");

        let mut replayed = vec![Vec::with_capacity(output_len + 31)];
        assert!(
            cache
                .hit_into(&[input.as_slice()], &mut replayed)
                .expect("Fix: generated materialized CUDA output cache hit must fit"),
            "Fix: materialized CUDA output cache must hit immediately for generated exact input case {seed}."
        );
        assert_eq!(
            replayed, outputs,
            "Fix: materialized CUDA output cache must replay exact output bytes for generated case {seed}."
        );
        assert!(
            cache.len() <= MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE,
            "Fix: materialized CUDA output cache must enforce the bounded entry count."
        );
        assert!(
            cache.byte_len() <= MAX_MATERIALIZED_OUTPUT_CACHE_BYTES_PER_PIPELINE,
            "Fix: materialized CUDA output cache must enforce the bounded byte budget."
        );
    }
}

#[test]
fn materialized_output_cache_replaces_same_key_without_double_counting_bytes() {
    let mut cache = MaterializedPipelineOutputCache::default();
    let input = b"same compiled CUDA graph replay input";
    let outputs_a = vec![b"old output".to_vec()];
    let outputs_b = vec![b"new output with a different byte length".to_vec()];

    cache
        .remember(&[input.as_slice()], &outputs_a)
        .expect("Fix: first materialized output cache insert must fit");
    assert_eq!(cache.len(), 1);
    let first_bytes = cache.byte_len();
    assert_eq!(first_bytes, input.len() + outputs_a[0].len());

    cache
        .remember(&[input.as_slice()], &outputs_b)
        .expect("Fix: same-key materialized output cache replacement must fit");
    assert_eq!(
        cache.len(),
        1,
        "Fix: same-key materialized output cache replacement must not create duplicate entries."
    );
    assert_eq!(
        cache.byte_len(),
        input.len() + outputs_b[0].len(),
        "Fix: same-key materialized output cache replacement must subtract the old entry before adding the new one."
    );

    let mut replayed = Vec::new();
    assert!(cache
        .hit_into(&[input.as_slice()], &mut replayed)
        .expect("Fix: same-key materialized output cache hit must fit"));
    assert_eq!(
        replayed, outputs_b,
        "Fix: same-key materialized output cache hit must return the newest output bytes."
    );
}

#[test]
fn materialized_output_snapshot_survives_same_key_replacement() {
    let mut cache = MaterializedPipelineOutputCache::default();
    let input = b"snapshot input retained outside the CUDA graph cache lock";
    let outputs_a = vec![b"snapshot bytes copied after lock release".to_vec()];
    let outputs_b = vec![b"replacement bytes stored by another replay".to_vec()];

    cache
        .remember(&[input.as_slice()], &outputs_a)
        .expect("Fix: initial materialized output snapshot fixture insert must fit");
    let snapshot = cache
        .snapshot(&[input.as_slice()])
        .expect("Fix: materialized output snapshot lookup must fit")
        .expect("Fix: materialized output snapshot must exist for exact input");

    cache
        .remember(&[input.as_slice()], &outputs_b)
        .expect("Fix: same-key materialized output replacement must fit after snapshot");

    let mut replayed_from_snapshot = Vec::new();
    snapshot
        .copy_into(&mut replayed_from_snapshot)
        .expect("Fix: materialized output snapshot copy after replacement must fit");
    assert_eq!(
        replayed_from_snapshot, outputs_a,
        "Fix: CUDA materialized cache hit snapshots must keep immutable output ownership so dispatch can copy after releasing the cache lock."
    );

    let mut replayed_from_cache = Vec::new();
    assert!(cache
        .hit_into(&[input.as_slice()], &mut replayed_from_cache)
        .expect("Fix: post-replacement materialized cache hit must fit"));
    assert_eq!(
        replayed_from_cache, outputs_b,
        "Fix: same-key replacement must still expose the newest cached output after an older snapshot escapes the cache lock."
    );
}

#[test]
fn materialized_output_cache_hit_preserves_existing_output_slots_until_reservation_succeeds() {
    let source = include_str!("../materialized_cache.rs");
    let copier = source
        .split("fn copy_materialized_outputs_into(")
        .nth(1)
        .expect("Fix: materialized cache must expose output copy helper.")
        .split("fn clone_materialized_cache_bytes(")
        .next()
        .expect("Fix: materialized output copy helper must precede byte clone helper.");
    let reserve_pos = copier
        .find("try_reserve_exact(source.len() - target.capacity())")
        .expect("Fix: materialized cache hit must reserve existing output bytes before mutation.");
    let append_clone_pos = copier
        .find("clone_materialized_cache_bytes(\n                source,\n                \"new output destination bytes\"")
        .expect("Fix: materialized cache hit must build new output slots before mutating the caller output vector.");
    let truncate_pos = copier
        .find("dst.truncate(outputs.len());")
        .expect("Fix: materialized cache hit must trim stale caller slots only after reservation.");
    let clear_pos = copier
        .find("target.clear();\n        target.extend_from_slice(source);")
        .expect(
            "Fix: materialized cache hit must rewrite existing output slots after reservation.",
        );

    assert!(
        reserve_pos < truncate_pos
            && append_clone_pos < truncate_pos
            && truncate_pos < clear_pos
            && copier.contains("dst.extend(appended_outputs);")
            && !copier.contains("target.clear();\n        target.try_reserve"),
        "Fix: CUDA materialized output cache hits must reserve/build output storage before truncating or clearing caller-owned outputs."
    );

    let mut cache = MaterializedPipelineOutputCache::default();
    let input = b"capacity-preserving materialized cache input";
    let outputs = vec![b"cached output a".to_vec(), b"cached output b".to_vec()];
    cache
        .remember(&[input.as_slice()], &outputs)
        .expect("Fix: materialized cache insert must fit capacity-preservation fixture.");

    let mut replayed = vec![
        Vec::with_capacity(64),
        Vec::with_capacity(32),
        b"stale extra output".to_vec(),
    ];
    replayed[0].extend_from_slice(b"old-a");
    replayed[1].extend_from_slice(b"old-b");
    let first_capacity = replayed[0].capacity();
    let second_capacity = replayed[1].capacity();

    assert!(
        cache
            .hit_into(&[input.as_slice()], &mut replayed)
            .expect("Fix: materialized cache hit must fit capacity-preservation fixture."),
        "Fix: materialized cache must hit exact capacity-preservation fixture input."
    );
    assert_eq!(replayed, outputs);
    assert_eq!(replayed[0].capacity(), first_capacity);
    assert_eq!(replayed[1].capacity(), second_capacity);
}

