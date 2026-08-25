use super::*;

fn parity_matrix_across_all_registered_ops() {
    let mut summary = Summary::default();
    let runners = backend_runners(&mut summary);
    let entries = unified_entries();
    let expr_rows = expr_variant_rows(&entries);
    let filter = env::var("VYRE_PARITY_FILTER").ok();

    assert!(
        runners.len() >= 2,
        "Fix: parity_matrix requires at least one linked dispatch-capable backend in addition to vyre-reference. Link a concrete driver crate for this gate."
    );
    assert!(
        !entries.is_empty(),
        "Fix: parity matrix linked zero OpEntry registrations. Ensure vyre-libs and vyre-intrinsics are linked into this test binary."
    );
    let missing_expr_variants = expr_variants()
        .iter()
        .copied()
        .filter(|variant| !expr_rows.contains_key(variant))
        .collect::<Vec<_>>();
    assert!(
        missing_expr_variants.is_empty(),
        "Fix: parity matrix must cover every Expr variant from vyre-spec; missing rows for {}",
        missing_expr_variants.join(", ")
    );

    for entry in entries {
        if filter.as_deref().is_some_and(|needle| {
            needle
                .strip_prefix('=')
                .map_or_else(|| !entry.id.contains(needle), |exact| entry.id != exact)
        }) {
            continue;
        }
        summary.ops_total += 1;

        let test_inputs = entry.test_inputs.unwrap_or_else(|| {
            panic!(
                "{}: missing test_inputs. Fix: every registered op must provide fixture inputs.",
                entry.id
            )
        });
        let expected_output = entry.expected_output.unwrap_or_else(|| {
            panic!(
                "{}: missing expected_output. Fix: every registered op must provide fixture oracle.",
                entry.id
            )
        });

        let program = (entry.build)();
        assert_valid(entry.id, &program, &runners);
        assert_region_chain(entry.id, &program);

        let input_cases = test_inputs();
        let expected_cases = expected_output();
        assert!(
            !input_cases.is_empty(),
            "Fix: {} registered empty test_inputs. Empty witnesses are zero execution coverage.",
            entry.id,
        );
        assert!(
            !expected_cases.is_empty(),
            "Fix: {} registered empty expected_output. Empty oracles are zero execution coverage.",
            entry.id,
        );
        assert_eq!(
            input_cases.len(),
            expected_cases.len(),
            "Fix: {} test_inputs / expected_output case count mismatch ({} vs {}).",
            entry.id,
            input_cases.len(),
            expected_cases.len()
        );

        summary.ops_covered += 1;
        let input_plan = backend_dispatch_plan(&program).unwrap_or_else(|error| {
            panic!("Fix: {} backend input plan failed: {error}", entry.id);
        });
        let grid_config = dispatch_grid::config_for_program(&program).unwrap_or_else(|error| {
            panic!("Fix: {} config_for_program failed: {error}", entry.id);
        });
        let mut reference_values = Vec::with_capacity(program.buffers().len());
        let mut outputs = Vec::<(&'static str, Vec<Vec<u8>>)>::with_capacity(runners.len());
        let mut borrowed_inputs = Vec::with_capacity(input_plan.sources.len());
        for (case_index, (inputs, expected)) in
            input_cases.iter().zip(expected_cases.iter()).enumerate()
        {
            let input_hash = hash_buffers(inputs);
            let program_hash_before = hash_program(&program);
            outputs.clear();
            borrowed_inputs.clear();

            let reference_output = runners[0]
                .dispatch_with_plan(
                    &program,
                    inputs,
                    &mut reference_values,
                    Some(&input_plan),
                    &mut borrowed_inputs,
                    &grid_config,
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "Fix: {} case {} reference failed: {error}",
                        entry.id, case_index
                    )
                });
            let reference_hash = hash_buffers(&reference_output);
            assert_eq!(
                hash_program(&program),
                program_hash_before,
                "Fix: {} case {} mutated the Program during dispatch; region chain must remain stable post-run.",
                entry.id,
                case_index
            );
            compare_outputs(
                entry.id,
                "reference",
                "expected_output",
                input_hash,
                &reference_output,
                expected,
                &program,
                &mut summary.divergences,
            );
            outputs.push(("reference", reference_output));

            for runner in runners.iter().skip(1) {
                match catch_unwind(AssertUnwindSafe(|| {
                    runner.dispatch_with_plan(
                        &program,
                        inputs,
                        &mut reference_values,
                        Some(&input_plan),
                        &mut borrowed_inputs,
                        &grid_config,
                    )
                })) {
                    Ok(Ok(output)) => {
                        assert_eq!(
                            hash_program(&program),
                            program_hash_before,
                            "Fix: {} case {} mutated the Program during {} dispatch; region chain must remain stable post-run.",
                            entry.id,
                            case_index,
                            runner.id
                        );
                        outputs.push((runner.id, output));
                    }
                    Ok(Err(error)) => {
                        panic!(
                            "{} on {}: backend dispatch error: {}. Fix: repair backend or op before claiming parity.",
                            entry.id, runner.id, error
                        );
                    }
                    Err(payload) => {
                        summary.divergences.push(Divergence {
                            op_id: entry.id,
                            backend_a: runner.id,
                            backend_b: "reference",
                            input_hash,
                            output_a_hash: hash_buffers(&[]),
                            output_b_hash: reference_hash,
                            detail: format!("dispatch panic: {}", panic_message(payload)),
                        });
                    }
                }
            }

            for i in 0..outputs.len() {
                for j in (i + 1)..outputs.len() {
                    let (backend_a, output_a) = &outputs[i];
                    let (backend_b, output_b) = &outputs[j];
                    compare_outputs(
                        entry.id,
                        backend_a,
                        backend_b,
                        input_hash,
                        output_a,
                        output_b,
                        &program,
                        &mut summary.divergences,
                    );
                }
            }
        }
    }

    eprintln!(
        "PARITY-SUMMARY ops_total={} ops_covered={} backends_linked={} backends_runnable={} divergences={}",
        summary.ops_total,
        summary.ops_covered,
        summary.backends_linked,
        summary.backends_runnable,
        summary.divergences.len()
    );
    for variant in expr_variants() {
        if let Some(op_ids) = expr_rows.get(variant) {
            eprintln!(
                "PARITY-EXPR-COVERAGE variant={} rows={}",
                variant,
                op_ids.join(",")
            );
        }
    }

    assert!(
        summary.ops_covered == summary.ops_total,
        "parity matrix under-coverage: ops_covered={} ops_total={}. Fix: every registered op must run at least one witness case.",
        summary.ops_covered,
        summary.ops_total
    );
    assert!(
        summary.divergences.is_empty(),
        "{}",
        format_divergences(&summary.divergences)
    );
}

