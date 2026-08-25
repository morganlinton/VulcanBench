fn entry_cases(entry: &UnifiedEntry) -> Vec<Vec<Vec<u8>>> {
    let cases = entry.test_inputs.map(|f| f()).unwrap_or_default();
    assert!(
        !cases.is_empty(),
        "Fix: {} has no pairwise test inputs; every op registry entry must publish runnable witnesses.",
        entry.id
    );
    cases
}

fn compatible_pairs() -> &'static [(usize, usize)] {
    static PAIRS: OnceLock<Vec<(usize, usize)>> = OnceLock::new();
    PAIRS.get_or_init(|| {
        let entries = all_entries_vec();
        for entry in &entries {
            let _ = entry_cases(entry);
        }

        let mut pairs = Vec::new();
        for a_idx in 0..entries.len() {
            for b_idx in 0..entries.len() {
                let a = &entries[a_idx];
                let b = &entries[b_idx];
                let composed = match try_compose(a, b) {
                    Ok(program) => program,
                    Err(_) => continue,
                };
                composed.validate().unwrap_or_else(|error| {
                    panic!(
                        "Fix: {} -> {} composed successfully but failed validation: {error}",
                        a.id, b.id
                    )
                });
                if let Some(reason) = missing_capability_reason(&composed) {
                    panic!(
                        "Fix: {} -> {} requires an unsupported backend capability: {reason}",
                        a.id, b.id
                    );
                }
                pairs.push((a_idx, b_idx));
            }
        }
        assert!(
            !pairs.is_empty(),
            "Fix: pairwise composition found zero compatible op pairs; repair op metadata or composition wiring."
        );
        pairs
    })
}

fn compatible_pair_count() -> usize {
    compatible_pairs().len()
}

fn compatible_pair_by_index(idx: usize) -> (&'static UnifiedEntry, &'static UnifiedEntry) {
    let pairs = compatible_pairs();
    let (a_idx, b_idx) = pairs[idx % pairs.len()];
    (entry_by_index(a_idx), entry_by_index(b_idx))
}

// ------------------------------------------------------------------
// Input assembly
// ------------------------------------------------------------------

/// Build the input vector for the fused program.
///
/// `fuse_programs` keeps shared buffers in the position of the first arm
/// that declares them.  Since op_a is arm 0, op_a's buffer list (including
/// the shared output) comes first, followed by op_b's *unique* buffers.
fn build_fused_inputs(
    _prog_a: &Program,
    prog_b: &Program,
    a_inputs: &[Vec<u8>],
    b_inputs: &[Vec<u8>],
    wired_b_in_name: &str,
) -> Vec<Vec<u8>> {
    let mut fused = Vec::new();

    // All of op_a's buffers (including the shared ReadWrite output).
    fused.extend_from_slice(a_inputs);

    // Append op_b's buffers, skipping the wired input because it is already
    // provided by op_a's output buffer.
    for (buf, bytes) in prog_b.buffers().iter().zip(b_inputs.iter()) {
        if buf.name() == wired_b_in_name {
            continue;
        }
        fused.push(bytes.clone());
    }

    fused
}

// ------------------------------------------------------------------
// Execution wrappers
// ------------------------------------------------------------------

fn run_reference(program: &Program, inputs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let values: Vec<Value> = inputs.iter().cloned().map(Value::from).collect();
    let outputs = vyre_reference::reference_eval(program, &values)
        .unwrap_or_else(|e| panic!("Fix: reference_eval failed: {e}"));
    outputs.into_iter().map(|v| v.to_bytes()).collect()
}

fn run_gpu(program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
    let backend = gpu();
    let lowered = vyre_foundation::optimizer::pre_lowering::optimize(program.clone());
    backend
        .dispatch(&lowered, inputs, &DispatchConfig::default())
        .map_err(|e| format!("GPU dispatch error: {e}"))
}

fn f32_to_ordered(bits: u32) -> u32 {
    if (bits & 0x8000_0000) != 0 {
        !bits
    } else {
        bits | 0x8000_0000
    }
}

fn assert_outputs_equal(op_a: &str, op_b: &str, tolerance: u32, cpu: &[Vec<u8>], gpu: &[Vec<u8>]) {
    assert_eq!(
        cpu.len(),
        gpu.len(),
        "Fix: {op_a} -> {op_b}: CPU produced {} buffers, GPU produced {}",
        cpu.len(),
        gpu.len()
    );

    for (buf_idx, (c_buf, g_buf)) in cpu.iter().zip(gpu.iter()).enumerate() {
        assert_eq!(
            c_buf.len(),
            g_buf.len(),
            "Fix: {op_a} -> {op_b}: buffer #{buf_idx} length diverged. CPU={} GPU={}",
            c_buf.len(),
            g_buf.len()
        );

        if tolerance == 0 {
            for (byte_offset, (cb, gb)) in c_buf.iter().zip(g_buf.iter()).enumerate() {
                assert_eq!(
                    cb, gb,
                    "Fix: {op_a} -> {op_b}: buffer #{buf_idx} first divergent byte at offset {byte_offset}. CPU={:02x?} GPU={:02x?}",
                    c_buf, g_buf
                );
            }
        } else {
            assert_eq!(
                c_buf.len() % 4,
                0,
                "Fix: {op_a} -> {op_b}: tolerance-based compare requires f32-aligned bytes"
            );
            for (lane, (c_word, g_word)) in
                c_buf.chunks_exact(4).zip(g_buf.chunks_exact(4)).enumerate()
            {
                let c_bits = u32::from_le_bytes(c_word.try_into().unwrap());
                let g_bits = u32::from_le_bytes(g_word.try_into().unwrap());
                let diff = f32_to_ordered(c_bits).abs_diff(f32_to_ordered(g_bits));
                assert!(
                    diff <= tolerance,
                    "Fix: {op_a} -> {op_b}: buffer #{buf_idx} lane {lane} diverged above {tolerance} ULP. CPU bits=0x{c_bits:08x} GPU bits=0x{g_bits:08x}"
                );
            }
        }
    }
}

// ------------------------------------------------------------------
// Proptest configuration
// ------------------------------------------------------------------

fn proptest_config() -> ProptestConfig {
    let cases = if std::env::var("CI_EXHAUSTIVE").is_ok() {
        50_000
    } else {
        5_000
    };
    ProptestConfig {
        cases,
        ..ProptestConfig::default()
    }
}

// ------------------------------------------------------------------
// Proving test  -  composition parity
// ------------------------------------------------------------------

proptest! {
    #![proptest_config(proptest_config())]

    #[test]
    fn pairwise_composition_parity(
        pair_idx in 0..compatible_pair_count(),
        case_idx in any::<usize>(),
    ) {
        let (a, b) = compatible_pair_by_index(pair_idx);

        let a_cases = entry_cases(a);
        let b_cases = entry_cases(b);

        let a_case = &a_cases[case_idx % a_cases.len()];
        let b_case = &b_cases[case_idx % b_cases.len()];

        // Build and validate compatibility.
        let composed =
            try_compose(a, b).expect("Fix: compatible_pair_by_index returned an incompatible pair");

        // Validate the fused IR.
        if let Err(e) = composed.validate() {
            panic!(
                "Fix: {} -> {} composed program validation failed: {e}",
                a.id, b.id
            );
        }

        if let Some(reason) = missing_capability_reason(&composed) {
            panic!(
                "Fix: {} -> {} backend capability check failed after compatibility precomputation: {reason}",
                a.id, b.id
            );
        }

        // Assemble fused inputs.
        let prog_a = (a.build)();
        let prog_b = (b.build)();
        let b_in = prog_b
            .buffers()
            .iter()
            .find(|buf| {
                matches!(
                    buf.access(),
                    BufferAccess::ReadOnly | BufferAccess::Uniform
                )
            })
            .expect("Fix: try_compose already verified op_b has an input");
        let fused_inputs = build_fused_inputs(&prog_a, &prog_b, a_case, b_case, b_in.name());

        // CPU reference oracle.
        let cpu = run_reference(&composed, &fused_inputs);

        // GPU backend.
        let gpu = match run_gpu(&composed, &fused_inputs) {
            Ok(out) => out,
            Err(reason) => {
                panic!(
                    "Fix: {} -> {} GPU dispatch failed in pairwise parity: {reason}",
                    a.id, b.id
                )
            }
        };

        let tolerance = fp_contract::effective_tolerance(a.id, &composed)
            .max(fp_contract::effective_tolerance(b.id, &composed));
        assert_outputs_equal(a.id, b.id, tolerance, &cpu, &gpu);
    }
}

// ------------------------------------------------------------------
// Adversarial test  -  never panic, never silent-wrong
// ------------------------------------------------------------------

proptest! {
    #![proptest_config(proptest_config())]

    #[test]
    fn pairwise_composition_adversarial(
        a_idx in 0..entry_count(),
        b_idx in 0..entry_count(),
        case_idx in any::<usize>(),
    ) {
        let a = entry_by_index(a_idx);
        let b = entry_by_index(b_idx);

        // try_compose must NEVER panic, regardless of compatibility.
        let composed_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            try_compose(a, b)
        }));

        assert!(
            composed_result.is_ok(),
            "Fix: {} -> {}: try_compose panicked  -  composition logic must reject gracefully",
            a.id, b.id
        );

        match composed_result.unwrap() {
            Ok(composed) => {
                // The pair was compatible.  Verify it does not produce silent
                // wrong output by running the reference vs GPU differential.

                let a_cases = a.test_inputs.map(|f| f()).unwrap_or_default();
                let b_cases = b.test_inputs.map(|f| f()).unwrap_or_default();
                if a_cases.is_empty() || b_cases.is_empty() {
                    panic!(
                        "Fix: {} -> {} compatible pair is missing test inputs.",
                        a.id, b.id
                    );
                }
                let a_case = &a_cases[case_idx % a_cases.len()];
                let b_case = &b_cases[case_idx % b_cases.len()];

                if composed.validate().is_err() {
                    // Validation failure on a supposedly-compatible pair is a bug.
                    panic!(
                        "Fix: {} -> {}: composed program failed validation despite compatibility check",
                        a.id, b.id
                    );
                }

                // Tolerance is derived from the composed program so FMA
                // contraction and transcendental policy cannot drift by test lane.

                if let Some(reason) = missing_capability_reason(&composed) {
                    panic!(
                        "Fix: {} -> {} backend capability check failed: {reason}",
                        a.id, b.id
                    );
                }

                let prog_a = (a.build)();
                let prog_b = (b.build)();
                let b_in = prog_b
                    .buffers()
                    .iter()
                    .find(|buf| {
                        matches!(
                            buf.access(),
                            BufferAccess::ReadOnly | BufferAccess::Uniform
                        )
                    })
                    .expect("Fix: compatible pair must have b_in");
                let fused_inputs = build_fused_inputs(&prog_a, &prog_b, a_case, b_case, b_in.name());

                let cpu = run_reference(&composed, &fused_inputs);

                let gpu = run_gpu(&composed, &fused_inputs).unwrap_or_else(|reason| {
                    panic!(
                        "Fix: {} -> {} GPU dispatch failed in adversarial pairwise parity: {reason}",
                        a.id, b.id
                    )
                });
                let tolerance = fp_contract::effective_tolerance(a.id, &composed)
                    .max(fp_contract::effective_tolerance(b.id, &composed));
                assert_outputs_equal(a.id, b.id, tolerance, &cpu, &gpu);
            }
            Err(reason) => {
                // Incompatible pair rejected cleanly  -  this is the expected
                // adversarial path.  The error must be actionable.
                assert!(
                    reason.contains("Fix:"),
                    "Fix: {} -> {}: rejection reason missing actionable hint: {}",
                    a.id, b.id, reason
                );
            }
        }
    }
}
