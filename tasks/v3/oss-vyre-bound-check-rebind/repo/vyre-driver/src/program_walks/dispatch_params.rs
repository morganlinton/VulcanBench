//! Dispatch ABI parameter derivation from binding plans.

use vyre_foundation::ir::Program;

use crate::binding::{Binding, BindingRole};

/// Derive the dispatch element count from a binding plan.
#[must_use]
pub fn dispatch_element_count(bindings: &[Binding]) -> u32 {
    dispatch_element_count_inner(bindings, false)
}

/// Derive the dispatch element count from a binding plan and Program body.
#[must_use]
pub fn dispatch_element_count_for_program(program: &Program, bindings: &[Binding]) -> u32 {
    dispatch_element_count_inner(bindings, program_contains_atomic(program))
}

fn dispatch_element_count_inner(bindings: &[Binding], force_full_span: bool) -> u32 {
    // Single pass over bindings: collect every fact the dispatch
    // policy needs (any-shared / max non-shared / max output) in one
    // scan. Previously up to three independent .iter() passes
    // traversed the same slice  -  for launch shapes that carry 60+
    // bindings each pass is real work.
    let mut any_shared = false;
    let mut max_non_shared: u32 = 0;
    let mut max_output: u32 = 0;
    for binding in bindings {
        if binding.role == BindingRole::Shared {
            any_shared = true;
            continue;
        }
        if binding.element_count > max_non_shared {
            max_non_shared = binding.element_count;
        }
        if matches!(binding.role, BindingRole::Output | BindingRole::InputOutput)
            && binding.element_count > max_output
        {
            max_output = binding.element_count;
        }
    }
    if any_shared || force_full_span {
        return max_non_shared.max(1);
    }
    if max_output > 0 {
        return max_output;
    }
    max_non_shared.max(1)
}

fn program_contains_atomic(program: &Program) -> bool {
    // ProgramStats::atomic_op_count is incremented exactly once per
    // Expr::Atomic during the cached single-pass stats walk. Reading
    // the cached count replaces the recursive node + expr scan this
    // function previously performed.
    program.stats().atomic_op_count > 0
}

/// Build per-buffer element-count parameter words for a dispatch with fallible
/// host-staging allocation.
pub fn try_dispatch_param_words(
    bindings: &[Binding],
    element_count: u32,
) -> Result<Vec<u32>, String> {
    let mut words = Vec::new();
    try_dispatch_param_words_into(bindings, element_count, &mut words)?;
    Ok(words)
}

/// Build per-buffer element-count parameter words into caller-owned storage.
///
/// # Errors
///
/// Returns an error when any binding slot overflows `usize`, or when
/// the host staging allocation fails. The caller-owned `words` buffer is
/// left in an unspecified but valid state (may be partially written); it
/// is the caller's responsibility to handle the error before using the buffer.
pub fn dispatch_param_words_into(
    bindings: &[Binding],
    element_count: u32,
    words: &mut Vec<u32>,
) -> Result<(), String> {
    try_dispatch_param_words_into(bindings, element_count, words)
}

/// Build per-buffer element-count parameter words into caller-owned storage
/// with explicit allocation and ABI-width errors.
pub fn try_dispatch_param_words_into(
    bindings: &[Binding],
    element_count: u32,
    words: &mut Vec<u32>,
) -> Result<(), String> {
    let word_len = dispatch_param_word_len_for_bindings(bindings)?;
    reserve_dispatch_param_words(words, word_len)?;
    words.clear();
    words.resize(word_len, 0);
    words[0] = element_count;
    for binding in bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }
        let slot = dispatch_param_word_slot(binding)?;
        words[slot] = if binding.element_count == 0 {
            element_count
        } else {
            binding.element_count
        };
    }
    Ok(())
}

fn dispatch_param_word_len_for_bindings(bindings: &[Binding]) -> Result<usize, String> {
    let mut word_len = dispatch_param_word_len_checked(bindings.len())?;
    for binding in bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }
        let required = dispatch_param_word_slot(binding)?
            .checked_add(1)
            .ok_or_else(|| {
                format!(
                    "dispatch binding slot {} overflows ABI parameter word count. Fix: split the Program before launch-parameter planning.",
                    binding.binding
                )
            })?;
        if required > word_len {
            word_len = required;
        }
    }
    Ok(word_len)
}

fn dispatch_param_word_slot(binding: &Binding) -> Result<usize, String> {
    let slot = usize::try_from(binding.binding).map_err(|error| {
        format!(
            "dispatch binding slot {} does not fit host usize ({error}). Fix: split the Program before launch-parameter planning.",
            binding.binding
        )
    })?;
    slot.checked_add(1).ok_or_else(|| {
        format!(
            "dispatch binding slot {} overflows ABI parameter slot. Fix: split the Program before launch-parameter planning.",
            binding.binding
        )
    })
}

fn dispatch_param_word_len_checked(binding_count: usize) -> Result<usize, String> {
    binding_count.checked_add(1).ok_or_else(|| {
        format!(
            "dispatch binding count {binding_count} overflows ABI parameter word count. Fix: split the Program before launch-parameter planning."
        )
    })
}

fn reserve_dispatch_param_words(words: &mut Vec<u32>, word_len: usize) -> Result<(), String> {
    crate::allocation::try_reserve_vec_to_capacity(words, word_len).map_err(|error| {
        format!(
            "dispatch parameter staging could not reserve {word_len} u32 word(s): {error}. Fix: split the Program before launch-parameter planning."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::BindingRole;
    use std::sync::Arc;

    fn binding(buffer_index: usize, element_count: u32) -> Binding {
        Binding {
            name: Arc::from("test"),
            binding: u32::try_from(buffer_index).expect("Fix: test binding index fits u32"),
            buffer_index,
            role: BindingRole::Input,
            element_size: 4,
            preferred_alignment: 4,
            element_count,
            static_byte_len: None,
            input_index: Some(0),
            output_index: None,
        }
    }

    #[test]
    fn dispatch_params_support_sparse_binding_indices_without_repeated_growth() {
        let bindings = [binding(4, 9), binding(1, 0)];
        let words = try_dispatch_param_words(&bindings, 7)
            .expect("Fix: sparse binding parameter words should stage");

        assert_eq!(words, vec![7, 0, 7, 0, 0, 9]);
    }

    #[test]
    fn dispatch_params_are_indexed_by_binding_slot_not_program_buffer_index() {
        let mut sparse = binding(0, 4);
        sparse.binding = 9;
        let mut dynamic = binding(7, 0);
        dynamic.binding = 2;
        let words = try_dispatch_param_words(&[sparse, dynamic], 11)
            .expect("Fix: sparse binding-slot parameter words should stage");

        assert_eq!(words.len(), 11);
        assert_eq!(words[0], 11);
        assert_eq!(words[3], 11);
        assert_eq!(words[10], 4);
        assert_eq!(
            words[8], 0,
            "Fix: CUDA/PTX parameter words are indexed by binding slot, not buffer_index."
        );
    }

    #[test]
    fn generated_dispatch_params_cover_sparse_binding_slot_matrix() {
        let mut checked = 0usize;
        for seed in 0..4096u32 {
            let binding_count = (seed as usize % 8) + 1;
            let mut bindings = Vec::with_capacity(binding_count);
            for index in 0..binding_count {
                let mut item = binding(
                    index,
                    if index % 3 == 0 {
                        0
                    } else {
                        seed + index as u32
                    },
                );
                item.binding = ((seed.wrapping_mul(17) + (index as u32 * 97)) % 1024) + 1;
                item.buffer_index = binding_count - 1 - index;
                bindings.push(item);
            }
            let element_count = seed.wrapping_mul(31) | 1;
            let words = try_dispatch_param_words(&bindings, element_count)
                .expect("Fix: generated sparse binding-slot param words should stage.");
            assert_eq!(words[0], element_count, "seed {seed}");
            for item in &bindings {
                let slot = usize::try_from(item.binding).expect("Fix: test binding fits usize") + 1;
                let expected = if item.element_count == 0 {
                    element_count
                } else {
                    item.element_count
                };
                assert_eq!(
                    words[slot], expected,
                    "Fix: generated dispatch-param seed {seed} binding slot {} must map to words[slot+1], regardless of buffer_index {}.",
                    item.binding, item.buffer_index
                );
                checked += 1;
            }
        }
        assert!(
            checked >= 18_000,
            "Fix: generated dispatch-param ABI coverage should exercise thousands of sparse binding layouts, got {checked}."
        );
    }

    #[test]
    fn dispatch_params_source_keeps_fallible_modular_staging() {
        let source = include_str!("dispatch_params.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: dispatch-param source must contain production section before tests");

        assert!(
            production.contains("pub fn try_dispatch_param_words")
                && production.contains("pub fn try_dispatch_param_words_into")
                && production.contains("fn dispatch_param_word_len_for_bindings")
                && production.contains("fn reserve_dispatch_param_words"),
            "Fix: dispatch parameter planning must expose modular fallible staging APIs."
        );
        assert!(
            !production.contains("Vec::with_capacity")
                && !production.contains("words.resize(binding.buffer_index + 2, 0)")
                && !production.contains("panic!("),
            "Fix: dispatch parameter planning must not allocate infallibly, grow repeatedly, or panic in release-path helpers."
        );
    }

    // Reproducing test for: dispatch-param-words-silent-empty-return
    // Before fix: `dispatch_param_words` (infallible) existed and returned Vec::new() on error,
    // silently producing a zero-length param buffer. After fix: function is removed; callers
    // must use `try_dispatch_param_words` which returns a Result.
    #[test]
    fn no_infallible_dispatch_param_words_function_exists_in_public_surface() {
        // Confirm the fallible API exists and succeeds on valid input.
        let bindings = [binding(0, 10)];
        let words = try_dispatch_param_words(&bindings, 10)
            .expect("Fix: try_dispatch_param_words must succeed for valid bindings");
        assert_eq!(
            words,
            vec![10, 10],
            "Fix: try_dispatch_param_words must return [element_count, binding_element_count]"
        );
        // Confirm the source has no silent empty-vec fallback (the pattern from the bug:
        // `Err(_) => Vec::new()` / `Err(_error) => Vec::new()`).
        let source = include_str!("dispatch_params.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: production section must precede test section");
        assert!(
            !production.contains("=> Vec::new()"),
            "Fix: the infallible dispatch_param_words silent-empty-return fallback must be absent from the production source"
        );
    }

    // Reproducing test for: dispatch-param-words-into-silent-clear
    // Before fix: `dispatch_param_words_into` returned () and silently cleared `words` on error.
    // After fix: it returns `Result<(), String>` so callers can observe and propagate the error.
    #[test]
    fn dispatch_param_words_into_propagates_error_instead_of_silently_clearing() {
        // Valid path: must succeed and populate correctly.
        let bindings = [binding(0, 7), binding(2, 3)];
        let mut words = Vec::new();
        dispatch_param_words_into(&bindings, 7, &mut words)
            .expect("Fix: dispatch_param_words_into must succeed for valid bindings");
        assert_eq!(
            words,
            vec![7, 7, 0, 3],
            "Fix: dispatch_param_words_into must produce [element_count, b0, b1_zero, b2] for valid bindings"
        );

        // Error path: a binding slot whose `slot + 1` overflows `usize` must return
        // Err (and surface a `Fix:` hint), not silently clear the buffer. `binding`
        // is a `u32`, so `usize::try_from` always succeeds and `slot.checked_add(1)`
        // only overflows when `usize` is 32-bit (binding == u32::MAX => MAX + 1). On a
        // 64-bit host u32::MAX + 1 fits `usize`, and the only other error source
        // (a ~17 GB host-staging allocation) is non-deterministic, so the overflow
        // assertion is gated to 32-bit where it is exact. On all hosts the success
        // path above proves the function returns `Ok(...)` with the populated buffer
        // (not the old silent `()`), and `no_infallible_dispatch_param_words_function_exists_in_public_surface`
        // proves the silent-clear fallback is absent from the source.
        #[cfg(target_pointer_width = "32")]
        {
            let mut overflow_binding = binding(0, 5);
            overflow_binding.binding = u32::MAX;
            let mut words2 = vec![0xdead_beef_u32; 4];
            let err = dispatch_param_words_into(&[overflow_binding], 5, &mut words2);
            assert!(
                err.is_err(),
                "Fix: dispatch_param_words_into must return Err on ABI slot overflow, not silently clear the buffer"
            );
            let msg = err.unwrap_err();
            assert!(
                msg.contains("Fix:"),
                "Fix: dispatch_param_words_into error message must include a Fix: hint, got: {msg}"
            );
        }
    }
}
