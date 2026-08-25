use super::*;

#[test]
fn release_per_op_f32_ulp_audit() {
    let backend = build_dispatch_backend();
    let entries = all_entries();

    let mut table: BTreeMap<&'static str, u32> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::with_capacity(entries.len());
    let mut cpu_values = Vec::new();
    let mut cpu_outputs = Vec::new();
    let mut adversarial_inputs = Vec::new();
    let mut base = Vec::new();

    for entry in entries {
        let program = (entry.build)();
        let has_f32 = program
            .buffers()
            .iter()
            .any(|b| b.element() == DataType::F32);
        if !has_f32 {
            continue;
        }

        let Some(test_inputs) = entry.test_inputs else {
            failures.push(format!(
                    "{}: missing test_inputs for F32 ULP audit. Fix: F32 ops require runnable witnesses.",
                    entry.id
                ));
            continue;
        };
        let Some(expected_output) = entry.expected_output else {
            failures.push(format!(
                    "{}: missing expected_output for F32 ULP audit. Fix: F32 ops require an oracle fixture.",
                    entry.id
                ));
            continue;
        };

        let tolerance = audit_f32_ulp_budget(&program);
        let required = program_caps::scan(&program);
        if let Err(missing) = program_caps::check_backend_capabilities(
            backend.id(),
            backend.supports_subgroup_ops(),
            backend.supports_f16(),
            backend.supports_bf16(),
            backend.supports_indirect_dispatch(),
            true,
            backend.supports_distributed_collectives(),
            backend.max_workgroup_size(),
            &required,
        ) {
            failures.push(format!(
                "{}: backend `{}` missing required capabilities for F32 ULP audit: {missing}",
                entry.id,
                backend.id(),
            ));
            continue;
        }
        let config = match dispatch_grid::config_for_program(&program) {
            Ok(config) => config,
            Err(error) => {
                failures.push(format!("{}: {error}", entry.id));
                continue;
            }
        };

        let cases = test_inputs();
        let expected_cases = expected_output();
        if cases.is_empty() {
            failures.push(format!(
                "{}: empty test_inputs for F32 ULP audit. Fix: empty fixtures are zero coverage.",
                entry.id
            ));
            continue;
        }
        if expected_cases.is_empty() {
            failures.push(format!(
                    "{}: empty expected_output for F32 ULP audit. Fix: empty oracles are zero coverage.",
                    entry.id
                ));
            continue;
        }
        if cases.len() != expected_cases.len() {
            failures.push(format!(
                "{}: test_inputs/expected_output case count mismatch ({} vs {})",
                entry.id,
                cases.len(),
                expected_cases.len()
            ));
            continue;
        }
        let input_plan = match backend_dispatch_plan(&program) {
            Ok(plan) => plan,
            Err(error) => {
                failures.push(format!(
                    "{}: ULP audit input planning failed: {error}",
                    entry.id
                ));
                continue;
            }
        };
        let adv_input_indices = backend_input_buffer_indices(&input_plan);

        let mut op_max_ulp = 0u32;

        // Fixture cases
        let mut backend_inputs: Vec<&[u8]> = Vec::new();
        backend_inputs.reserve(program.buffers().len());
        for (case_index, inputs) in cases.iter().enumerate() {
            if let Err(error) =
                backend_inputs_from_fixture_into(inputs, &input_plan, &mut backend_inputs)
            {
                failures.push(format!(
                    "{} case {}: ULP audit input planning failed: {error}",
                    entry.id, case_index
                ));
                continue;
            }
            let cpu = match run_cpu_from_slices(
                &program,
                &backend_inputs,
                &mut cpu_values,
                &mut cpu_outputs,
            ) {
                Ok(o) => o,
                Err(e) => {
                    failures.push(format!(
                        "{} case {}: CPU reference failed: {e}",
                        entry.id, case_index
                    ));
                    continue;
                }
            };
            let gpu = match backend.dispatch_borrowed(&program, &backend_inputs, &config) {
                Ok(o) => o,
                Err(e) => {
                    failures.push(format!(
                        "{} case {}: backend dispatch failed: {e}",
                        entry.id, case_index
                    ));
                    continue;
                }
            };
            let max_ulp = match max_ulp_delta(&cpu, &gpu, &program) {
                Some(u) => u,
                None => {
                    failures.push(format!(
                        "{} case {}: output buffer shape mismatch",
                        entry.id, case_index
                    ));
                    continue;
                }
            };
            op_max_ulp = op_max_ulp.max(max_ulp);
            if max_ulp > tolerance {
                failures.push(format!(
                    "{} case {}: max ULP {} > tolerance {}",
                    entry.id, case_index, max_ulp, tolerance
                ));
            }
        }

        // Adversarial companion
        if !cases.is_empty() {
            if let Err(error) =
                backend_inputs_from_fixture_into_owned(&cases[0], &input_plan, &mut base)
            {
                failures.push(format!(
                    "{}: ULP audit adversarial base planning failed: {error}",
                    entry.id
                ));
                continue;
            }
            for &adv in ADVERSARIAL_VALUES {
                adversarial_inputs.clear();
                make_adversarial_inputs_into(
                    &base,
                    &program,
                    &adv_input_indices,
                    adv,
                    &mut adversarial_inputs,
                );
                let mut backend_inputs_for_adversarial = Vec::new();
                backend_inputs_from_vectors(
                    &adversarial_inputs,
                    &mut backend_inputs_for_adversarial,
                );
                let cpu = match run_cpu_from_slices(
                    &program,
                    &backend_inputs_for_adversarial,
                    &mut cpu_values,
                    &mut cpu_outputs,
                ) {
                    Ok(o) => o,
                    Err(error) => {
                        failures.push(format!(
                            "{} adversarial ({:?}): CPU reference failed: {error}",
                            entry.id, adv
                        ));
                        continue;
                    }
                };
                match backend.dispatch_borrowed(&program, &backend_inputs_for_adversarial, &config)
                {
                    Ok(gpu) => {
                        let max_ulp = match max_ulp_delta(&cpu, &gpu, &program) {
                            Some(u) => u,
                            None => {
                                failures.push(format!(
                                    "{} adversarial ({:?}): output buffer shape mismatch",
                                    entry.id, adv
                                ));
                                continue;
                            }
                        };
                        op_max_ulp = op_max_ulp.max(max_ulp);
                        if adversarial_value_requires_ulp(adv) && max_ulp > tolerance {
                            failures.push(format!(
                                "{} adversarial ({:?}): max ULP {} > tolerance {}",
                                entry.id, adv, max_ulp, tolerance
                            ));
                        }
                    }
                    Err(error) => failures.push(format!(
                        "{} adversarial ({:?}): backend dispatch failed: {error}",
                        entry.id, adv
                    )),
                }
            }
        }

        table.insert(entry.id, op_max_ulp);
    }

    eprintln!("\n=== RELEASE ULP AUDIT TABLE ===");
    eprintln!("{:<60} {:>6}", "op_id", "max_ulp");
    eprintln!("{}", "-".repeat(68));
    for (op_id, max_ulp) in &table {
        eprintln!("{:<60} {:>6} ULP", op_id, max_ulp);
    }
    eprintln!("{}\n", "-".repeat(68));

    assert!(
        failures.is_empty(),
        "ULP audit failures:\n  - {}",
        failures.join("\n  - ")
    );
}
