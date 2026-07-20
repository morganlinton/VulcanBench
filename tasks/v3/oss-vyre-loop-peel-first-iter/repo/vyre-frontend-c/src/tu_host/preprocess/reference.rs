use super::*;
/// Expand preprocessor macros with the host reference implementation used by
/// parity tests and oracle comparisons.
pub fn reference_expand_preprocessor_macros(source: &str) -> String {
    let mut macros = HashMap::<String, MacroDef>::new();
    let mut conditionals = Vec::<ConditionalFrame>::new();
    let mut out = String::new();

    for raw_line in source.lines() {
        let leading_trimmed = raw_line.trim_start();
        let directive_line = leading_trimmed
            .starts_with('#')
            .then(|| strip_directive_comments(leading_trimmed));
        let trimmed = directive_line.as_deref().unwrap_or(leading_trimmed);
        let active = conditionals.last().is_none_or(|f| f.current_active);
        if let Some(rest) = trimmed.strip_prefix("#define") {
            if active {
                if let Some((name, def)) = parse_define(rest) {
                    macros.insert(name, def);
                }
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#undef") {
            if active {
                macros.remove(rest.trim());
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#ifdef") {
            let parent_active = active;
            let cond = parent_active && macros.contains_key(rest.trim());
            conditionals.push(ConditionalFrame {
                parent_active,
                branch_taken: cond,
                current_active: parent_active && cond,
                saw_else: false,
            });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#ifndef") {
            let parent_active = active;
            let cond = parent_active && !macros.contains_key(rest.trim());
            conditionals.push(ConditionalFrame {
                parent_active,
                branch_taken: cond,
                current_active: parent_active && cond,
                saw_else: false,
            });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#if") {
            let parent_active = active;
            let cond = parent_active && eval_preproc_expr(rest.trim(), &macros);
            conditionals.push(ConditionalFrame {
                parent_active,
                branch_taken: cond,
                current_active: parent_active && cond,
                saw_else: false,
            });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#elif") {
            let Some(frame) = conditionals.last_mut() else {
                panic!("#elif without matching #if");
            };
            if frame.saw_else {
                panic!("#elif after #else");
            }
            let cond = frame.parent_active
                && !frame.branch_taken
                && eval_preproc_expr(rest.trim(), &macros);
            frame.current_active = frame.parent_active && cond;
            frame.branch_taken |= cond;
            continue;
        }
        if trimmed.starts_with("#else") {
            let Some(frame) = conditionals.last_mut() else {
                panic!("#else without matching #if");
            };
            if frame.saw_else {
                panic!("duplicate preprocessor #else");
            }
            let cond = !frame.branch_taken;
            frame.current_active = frame.parent_active && cond;
            frame.branch_taken = true;
            frame.saw_else = true;
            continue;
        }
        if trimmed.starts_with("#endif") {
            if conditionals.pop().is_none() {
                panic!("#endif without matching #if");
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#error") {
            if active {
                panic!("active preprocessor #error encountered:{rest}");
            }
            continue;
        }
        if trimmed.starts_with("#pragma") || trimmed.starts_with("#line") {
            continue;
        }

        if active {
            out.push_str(&expand_line_macros(raw_line, &macros, 0));
            out.push('\n');
        }
    }

    if !conditionals.is_empty() {
        panic!("unterminated conditional: {} unclosed #if/#ifdef/#ifndef block(s)", conditionals.len());
    }

    out
}

#[cfg(test)]
mod recursion_tests {
    use super::*;

    #[test]
    fn overdeep_macro_expansion_stops_at_bounded_frontier() {
        let mut source = String::new();
        for idx in 0..40 {
            source.push_str(&format!("#define M{idx} M{}\n", idx + 1));
        }
        source.push_str("M0\n");
        let out = reference_expand_preprocessor_macros(&source);
        assert!(out.starts_with('M'));
        assert!(!out.contains("M40"));
    }

    #[test]
    fn inactive_parent_blocks_do_not_evaluate_nested_if_expressions() {
        let source = "#if 0\n#if 1 / 0\nbad\n#endif\n#elif 1\nok\n#endif\n";
        assert_eq!(reference_expand_preprocessor_macros(source), "ok\n");
    }

    #[test]
    fn inactive_parent_blocks_do_not_evaluate_nested_elif_expressions() {
        let source = "#if 0\n#if 0\nbad\n#elif 1 / 0\nbad\n#endif\n#endif\n";
        assert_eq!(reference_expand_preprocessor_macros(source), "");
    }

    #[test]
    #[should_panic(expected = "active preprocessor #error encountered")]
    fn active_error_directive_fails_loudly() {
        let _ = reference_expand_preprocessor_macros("#error unsupported target\n");
    }

    #[test]
    fn inactive_error_directive_is_ignored() {
        let source = "#if 0\n#error skipped\n#endif\nok\n";
        assert_eq!(reference_expand_preprocessor_macros(source), "ok\n");
    }

    #[test]
    #[should_panic(expected = "#else without matching #if")]
    fn unmatched_else_fails_loudly() {
        let _ = reference_expand_preprocessor_macros("#else\nbad\n");
    }

    #[test]
    #[should_panic(expected = "#endif without matching #if")]
    fn unmatched_endif_fails_loudly() {
        let _ = reference_expand_preprocessor_macros("#endif\n");
    }

    #[test]
    #[should_panic(expected = "#elif after #else")]
    fn elif_after_else_fails_loudly() {
        let _ = reference_expand_preprocessor_macros("#if 0\n#else\nok\n#elif 1\nbad\n#endif\n");
    }

    #[test]
    #[should_panic(expected = "duplicate preprocessor #else")]
    fn duplicate_else_fails_loudly() {
        let _ = reference_expand_preprocessor_macros("#if 0\n#else\nok\n#else\nbad\n#endif\n");
    }

    #[test]
    #[should_panic(expected = "unterminated conditional")]
    fn unterminated_if_fails_loudly() {
        let _ = reference_expand_preprocessor_macros("#if 1\nok\n");
    }
}
