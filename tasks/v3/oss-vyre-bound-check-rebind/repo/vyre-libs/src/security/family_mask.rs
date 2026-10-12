//! Family name → tag-bit mask allocation.
//!
//! Canonical bit allocation for every `@family` predicate label that
//! appears in security rules. Shared across consumers so the predicate-
//! side `@family` resolver and the source-side classifier agree on
//! which bit a family lives at.
//!
//! Every family used in any rule must be represented explicitly in
//! [`CANONICAL_BITS`]. There is no synthetic-bit fallback  -  a family
//! without an entry is a compile-time error.
//!
//! # Bit layout
//!
//! - bits 0..15   -  canonical security families (ALLOCATOR, RECEIVE,
//!   SANITIZER, etc.).
//! - bits 16..18  -  reserved for structural families (FUNCTION, FILE,
//!   PACKAGE) declared by consumers.
//! - bits 19..23  -  reserved for consumer extension tags (e.g.
//!   STRING_LITERAL, STACK_ARRAY).
//! - bits 24..31  -  launch-family unique bits.

/// Canonical family → tag-bit mask. Returns `None` for any family
/// without an explicit allocation.
#[must_use]
pub fn canonical_family_mask(family: &str) -> Option<u32> {
    CANONICAL_BITS
        .iter()
        .find_map(|(name, bit)| (*name == family).then_some(*bit))
}

/// Strict resolution: returns the canonical bit allocation, or an
/// actionable error string for any family without an explicit entry
/// in [`CANONICAL_BITS`]. Callers that have their own error type
/// should wrap this `String` into their domain error.
///
/// # Errors
///
/// Returns `Err` when `family` is not registered in [`CANONICAL_BITS`].
pub fn resolve_label_family_mask(family: &str) -> Result<u32, String> {
    canonical_family_mask(family).ok_or_else(|| {
        format!(
            "label family `@{family}` has no canonical bit allocation. \
             Fix: declare the family in vyre_libs::security::family_mask::CANONICAL_BITS."
        )
    })
}

// ────────────────────────────────────────────────────────────────────
// Canonical security families  -  bits 0..15.
// ────────────────────────────────────────────────────────────────────

pub const ALLOCATOR: u32 = 1 << 0;
pub const RECEIVE: u32 = 1 << 1;
pub const OVERFLOW_CHECK: u32 = 1 << 2;
pub const SANITIZER: u32 = 1 << 3;
pub const SOURCE_NETWORK: u32 = 1 << 4;
pub const SINK_FILESYSTEM: u32 = 1 << 5;
pub const SINK_PROCESS: u32 = 1 << 6;
pub const COPY_TO_USER: u32 = 1 << 7;
pub const COPY_FROM_USER: u32 = 1 << 15;
pub const FREE: u32 = 1 << 8;
pub const COMPARISON_OP: u32 = 1 << 9;
pub const DECODE: u32 = 1 << 10;
pub const INFLATE: u32 = 1 << 11;
pub const TYPE_CAST_UNCHECKED: u32 = 1 << 12;
pub const PRIVILEGE_CHECK: u32 = 1 << 13;
pub const PRIVILEGE_USE: u32 = 1 << 14;

// ────────────────────────────────────────────────────────────────────
// Launch-family unique bits  -  bits 24..31. 8 slots; each is OR'd onto
// a node alongside any canonical bit it semantically inherits, so
// `call_to(@receive_family)` continues to match every receive call
// (broad) while `call_to(@gets_family)` matches only `gets`-shaped
// calls (narrow).
// ────────────────────────────────────────────────────────────────────

pub const GETS_LAUNCH: u32 = 1 << 24;
pub const PRINTF_LAUNCH: u32 = 1 << 25;
pub const UNBOUNDED_COPY_LAUNCH: u32 = 1 << 26;
pub const UNBOUNDED_SPRINTF_LAUNCH: u32 = 1 << 27;
pub const POINTER_USE_LAUNCH: u32 = 1 << 28;
pub const TYPE_TAG_CHECK_LAUNCH: u32 = 1 << 29;
pub const REASSIGN_NULL_AFTER_FREE_LAUNCH: u32 = 1 << 30;
pub const BOUNDED_COPY_OR_LENGTH_CHECK_LAUNCH: u32 = 1 << 31;

/// Canonical family allocation table.
///
/// Every family that appears in a `@family` predicate or in
/// classifier labels must be represented explicitly here. Adding a
/// new family is a one-line append; running out of low-32 room
/// requires widening `pg_node_tags` to two words and lifting this
/// table to `u64`.
pub const CANONICAL_BITS: &[(&str, u32)] = &[
    // Canonical security families.
    ("allocator_family", ALLOCATOR),
    ("deallocator_family", FREE),
    ("free_family", FREE),
    ("buffer_source_family", RECEIVE),
    ("buffer_source", RECEIVE),
    ("user_input_family", RECEIVE),
    ("untrusted_input_family", RECEIVE),
    ("receive_family", RECEIVE),
    ("untrusted_input", RECEIVE),
    ("overflow_check_family", OVERFLOW_CHECK),
    ("range_check_family", OVERFLOW_CHECK),
    ("length_clamp_family", OVERFLOW_CHECK),
    ("bounded_check_family", OVERFLOW_CHECK),
    ("checked_arith_or_size_clamp", OVERFLOW_CHECK),
    ("sanitizer_family", SANITIZER),
    ("html_escape_family", SANITIZER),
    ("shell_escape_family", SANITIZER),
    ("sql_escape_family", SANITIZER),
    ("url_validation_family", SANITIZER),
    ("auth_check_family", SANITIZER),
    ("authz_check_family", SANITIZER),
    ("password_check_family", SANITIZER),
    ("comparison_family", SANITIZER),
    ("prefix_guard_family", SANITIZER),
    ("pathname_sanitize_family", SANITIZER),
    ("path_canonicalize_family", SANITIZER),
    ("regex_safety_family", SANITIZER),
    ("crlf_sanitizer_family", SANITIZER),
    ("proto_key_sanitizer_family", SANITIZER),
    ("verify_arg_slot", SANITIZER),
    ("bounded_by_array_capacity", SANITIZER),
    ("bounded_sprintf_or_length_check", SANITIZER),
    ("network_input_source", SOURCE_NETWORK),
    ("http_input_family", SOURCE_NETWORK),
    ("http_client_family", SOURCE_NETWORK),
    ("file_sink", SINK_FILESYSTEM),
    ("file_open_family", SINK_FILESYSTEM),
    ("filesystem_open_family", SINK_FILESYSTEM),
    ("exec_family", SINK_PROCESS),
    ("exec_sink", SINK_PROCESS),
    ("copy_to_user_family", COPY_TO_USER),
    ("comparison_op_family", COMPARISON_OP),
    ("decoder_family", DECODE),
    ("decompressor_family", DECODE),
    ("inflate_family", INFLATE),
    ("decompression_cap_family", INFLATE),
    ("narrow_cast_family", TYPE_CAST_UNCHECKED),
    ("narrowing_cast_family", TYPE_CAST_UNCHECKED),
    ("pointer_cast_family", TYPE_CAST_UNCHECKED),
    ("type_cast_unchecked_family", TYPE_CAST_UNCHECKED),
    ("privilege_check_family", PRIVILEGE_CHECK),
    ("privilege_use_family", PRIVILEGE_USE),
    ("privileged_op_family", PRIVILEGE_USE),
    // Launch families.
    ("sized_input_read_family", RECEIVE),
    ("sized_memory_copy_family", UNBOUNDED_COPY_LAUNCH),
    ("reallocator_family", ALLOCATOR),
    ("null_check_family", SANITIZER),
    ("pointer_assignment_family", ALLOCATOR),
    ("gets_family", GETS_LAUNCH),
    ("gets", GETS_LAUNCH),
    ("printf_family", PRINTF_LAUNCH),
    ("printf", PRINTF_LAUNCH),
    ("unbounded_string_copy_family", UNBOUNDED_COPY_LAUNCH),
    ("unbounded_string_copy", UNBOUNDED_COPY_LAUNCH),
    ("unbounded_sprintf_family", UNBOUNDED_SPRINTF_LAUNCH),
    ("unbounded_sprintf", UNBOUNDED_SPRINTF_LAUNCH),
    ("pointer_use_family", POINTER_USE_LAUNCH),
    ("type_tag_check", TYPE_TAG_CHECK_LAUNCH),
    ("reassign_or_null_after_free", REASSIGN_NULL_AFTER_FREE_LAUNCH),
    ("bounded_copy_or_length_check", BOUNDED_COPY_OR_LENGTH_CHECK_LAUNCH),
    ("allocator", ALLOCATOR),
    ("deallocator", FREE),
    ("worker_family", SINK_PROCESS),
    ("channel_family", RECEIVE),
    ("cleanup_family", FREE),
    ("copy_from_user_family", COPY_FROM_USER),
    ("aes_cipher_family", DECODE),
    ("alg_none_patterns", COMPARISON_OP),
    ("archive_entry_family", DECODE),
    ("array_access_family", OVERFLOW_CHECK),
    ("array_index_family", OVERFLOW_CHECK),
    ("authentication_success_family", PRIVILEGE_CHECK),
    ("binary_magic_family", COMPARISON_OP),
    ("buffer_write_family", OVERFLOW_CHECK),
    ("canonicalize_family", SANITIZER),
    ("cipher_init_family", DECODE),
    ("cli_arg_family", RECEIVE),
    ("cli_source", RECEIVE),
    ("credential_source", RECEIVE),
    ("credential_source_family", RECEIVE),
    ("crypto_param_family", DECODE),
    ("csrf_token_family", PRIVILEGE_CHECK),
    ("db_fetch_family", RECEIVE),
    ("deserializer_family", DECODE),
    ("ecb_mode_literal", COMPARISON_OP),
    ("ecdsa_sign_family", DECODE),
    ("encrypt_family", DECODE),
    ("eval_family", SINK_PROCESS),
    ("external_entity_family", DECODE),
    ("false_literal", COMPARISON_OP),
    ("family", COMPARISON_OP),
    ("file_source", RECEIVE),
    ("filesystem_read_family", RECEIVE),
    ("filesystem_stat_family", RECEIVE),
    ("filesystem_unlink_family", SINK_FILESYSTEM),
    ("form_render_family", SANITIZER),
    ("html_render_family", SANITIZER),
    ("http_route_handler_family", RECEIVE),
    ("id_param_family", RECEIVE),
    ("index_mask_family", OVERFLOW_CHECK),
    ("insecure_context_literal", COMPARISON_OP),
    ("ioctl_ptr_extract_family", RECEIVE),
    ("ioctl_size_extract_family", RECEIVE),
    ("jwt_decode_family", DECODE),
    ("jwt_secure_alg", COMPARISON_OP),
    ("kdf_family", DECODE),
    ("key_material_family", DECODE),
    ("length_extractor_family", OVERFLOW_CHECK),
    ("length_reconciliation_family", OVERFLOW_CHECK),
    ("llm_prompt_family", RECEIVE),
    ("log_family", SINK_FILESYSTEM),
    ("mac_family", DECODE),
    ("memory_map_exec_family", SINK_PROCESS),
    ("min_kdf_iterations", COMPARISON_OP),
    ("network_sink", SOURCE_NETWORK),
    ("npm_script_source", RECEIVE),
    ("null_out_family", SANITIZER),
    ("object_merge_family", SANITIZER),
    ("outbound_http_family", SOURCE_NETWORK),
    ("packer_magic_literal", COMPARISON_OP),
    ("password_input_family", RECEIVE),
    ("pickle_load_family", DECODE),
    ("pointer_move_family", ALLOCATOR),
    ("reassign_family", ALLOCATOR),
    ("recursive_call_family", RECEIVE),
    ("redirect_sink_family", SOURCE_NETWORK),
    ("regex_compile_family", SANITIZER),
    ("rsa_pkcs1v15_family", DECODE),
    ("rust_DA_family", SANITIZER),
    ("rust_safe_drop_family", FREE),
    ("rust_safe_indexing_family", OVERFLOW_CHECK),
    ("schema_validation_family", SANITIZER),
    ("sensitive_file_source", RECEIVE),
    ("session_regenerate_family", PRIVILEGE_CHECK),
    ("session_set_family", PRIVILEGE_CHECK),
    ("shared_access_family", RECEIVE),
    ("shell_source", RECEIVE),
    ("speculation_barrier_family", SANITIZER),
    ("sql_execute_family", SINK_PROCESS),
    ("sql_query_family", SINK_PROCESS),
    ("sql_sink", SINK_PROCESS),
    ("stream_advance_family", RECEIVE),
    ("sync_primitive_family", RECEIVE),
    ("system_source", RECEIVE),
    ("template_helper_family", SANITIZER),
    ("template_render_family", SANITIZER),
    ("template_sandbox_family", SANITIZER),
    ("templating_library_family", SANITIZER),
    ("tls_client_config_family", DECODE),
    ("token_generator_family", DECODE),
    ("total_output_cap_family", OVERFLOW_CHECK),
    ("training_corpus_family", RECEIVE),
    ("untrusted_prompt_source_family", RECEIVE),
    ("weak_hash_family", DECODE),
    ("weak_rng_family", DECODE),
    ("xml_entity_disable_family", SANITIZER),
    ("xss_sink", SANITIZER),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_family_mask_returns_explicit_bit() {
        assert_eq!(canonical_family_mask("allocator_family"), Some(ALLOCATOR));
        assert_eq!(canonical_family_mask("gets_family"), Some(GETS_LAUNCH));
        assert_eq!(canonical_family_mask("not_a_family"), None);
    }

    #[test]
    fn resolve_errors_for_unknown_family() {
        let err = resolve_label_family_mask("unregistered").expect_err("must error");
        assert!(err.contains("unregistered"));
        assert!(err.contains("Fix:"));
    }

    #[test]
    fn launch_bits_are_unique() {
        let launch = [
            GETS_LAUNCH,
            PRINTF_LAUNCH,
            UNBOUNDED_COPY_LAUNCH,
            UNBOUNDED_SPRINTF_LAUNCH,
            POINTER_USE_LAUNCH,
            TYPE_TAG_CHECK_LAUNCH,
            REASSIGN_NULL_AFTER_FREE_LAUNCH,
            BOUNDED_COPY_OR_LENGTH_CHECK_LAUNCH,
        ];
        let mut seen = 0u32;
        for &b in &launch {
            assert_eq!(seen & b, 0, "launch bit collision at 0x{b:08x}");
            seen |= b;
        }
    }
}
