//! Hash module single-source-of-truth architecture contracts.

#![allow(deprecated)]
use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Fix: vyre-libs must live under the workspace root.")
        .to_path_buf()
}

#[test]
fn crc32_lib_wrapper_delegates_to_primitive_without_forking_bit_logic() {
    let wrapper =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/hash/crc32.rs"))
            .expect("Fix: vyre-libs hash CRC32 wrapper must remain readable.");
    let wrapper_plumbing =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/hash/wrap.rs"))
            .expect("Fix: shared hash wrapper plumbing must remain readable.");

    assert!(
        wrapper.contains("use vyre_primitives::hash::crc32::{crc32_program, CRC32_OP_ID};"),
        "Fix: vyre-libs::hash::crc32 must delegate to vyre-primitives::hash::crc32, not fork it."
    );
    assert!(
        wrapper.contains("let primitive = crc32_program(&input, &out, n);"),
        "Fix: build the CRC32 body through the primitive crate before wrapping provenance."
    );
    assert!(
        wrapper.contains("HashWrapperSpec::new(OP_ID, CRC32_OP_ID, FAMILY_PREFIX, 1)")
            && wrapper.contains("SPEC.wrap_static_count(&input, &out, n, primitive)"),
        "Fix: the Tier-3 CRC32 op must delegate primitive provenance through shared hash wrapper plumbing."
    );
    assert!(
        wrapper_plumbing.contains("wrap_child(\n                primitive_op_id,"),
        "Fix: shared hash wrapper plumbing must expose the primitive child region for provenance."
    );

    for forbidden in [
        "Node::loop_for",
        "Expr::bitxor",
        "Expr::shr",
        "CRC32_POLY",
        "CRC32_INIT",
    ] {
        assert!(
            !wrapper.contains(forbidden),
            "Fix: vyre-libs::hash::crc32 contains `{forbidden}`; keep CRC32 bit-level logic in vyre-primitives."
        );
    }
}

#[test]
fn multi_hash_uses_primitive_crc32_ir_helpers_without_forking_crc_logic() {
    let wrapper = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/hash/multi_hash.rs"),
    )
    .expect("Fix: vyre-libs hash multi_hash implementation must remain readable.");
    let primitive =
        fs::read_to_string(workspace_root().join("vyre-primitives/src/hash/multi_hash.rs"))
            .expect("Fix: vyre-primitives fused multi_hash implementation must remain readable.");

    assert!(
        wrapper.contains(
            "use vyre_primitives::hash::multi_hash::{multi_hash_program, MULTI_HASH_OP_ID};"
        ),
        "Fix: vyre-libs::hash::multi_hash must delegate to the primitive fused multi-hash body."
    );
    assert!(
        wrapper.contains("multi_hash_program(&input, &out_crc32, n)")
            && wrapper.contains("SPEC.wrap_static_count"),
        "Fix: Tier-3 multi_hash must wrap the primitive fused body instead of rebuilding the loop."
    );

    for required in [
        "crc32_initial_expr",
        "crc32_update_byte_nodes",
        "crc32_finalize_expr",
        "fnv1a32_initial_expr",
        "fnv1a32_update_byte_node",
        "adler32_initial_a_expr",
        "adler32_initial_b_expr",
        "adler32_update_byte_nodes",
        "adler32_finalize_expr",
    ] {
        assert!(
            primitive.contains(required),
            "Fix: primitive multi_hash must use primitive hash helper `{required}` to keep fused one-pass hashing without forking hash logic."
        );
    }

    for forbidden in [
        "CRC32_POLY",
        "CRC32_POLY_REFLECTED",
        "CRC32_INIT",
        "CRC32_FINAL_XOR",
        "FNV1A32_OFFSET",
        "FNV1A32_PRIME",
        "MOD_ADLER",
    ] {
        assert!(
            !wrapper.contains(forbidden),
            "Fix: multi_hash contains `{forbidden}`; keep CRC constants and bit-loop authority in vyre-primitives."
        );
    }
}

#[test]
fn crc32_primitive_is_the_only_hash_crc32_authority() {
    let primitive = fs::read_to_string(workspace_root().join("vyre-primitives/src/hash/crc32.rs"))
        .expect("Fix: primitive CRC32 authority must remain readable.");

    for required in [
        "pub const CRC32_INIT",
        "pub const CRC32_POLY",
        "pub const CRC32_OP_ID",
        "pub fn crc32(bytes: &[u8]) -> u32",
        "pub fn crc32_program(input: &str, out: &str, n: u32) -> Program",
        "fn crc32_body(input: &str, out: &str, n: u32) -> Vec<Node>",
    ] {
        assert!(
            primitive.contains(required),
            "Fix: primitive CRC32 authority is missing `{required}`."
        );
    }
}

#[test]
fn c_preprocessor_hashing_uses_primitive_fnv_source() {
    let expansion_mod = include_str!("../src/parsing/c/preprocess/expansion/mod.rs");
    let expansion_helpers = include_str!("../src/parsing/c/preprocess/expansion/helpers.rs");
    let macro_table = include_str!("../src/parsing/c/preprocess/gpu_pipeline/macro_table.rs");
    let scan_engine = include_str!("../src/scan/engine.rs");

    assert!(
        !expansion_mod.contains("0x811c_9dc5") && !expansion_mod.contains("0x0100_0193"),
        "Fix: C macro expansion must not redefine FNV-1a constants outside vyre-primitives."
    );
    assert!(
        expansion_helpers.contains("fnv1a32_initial_expr")
            && expansion_helpers.contains("fnv1a32_update_byte_node"),
        "Fix: GPU macro source-span hashing must use primitive FNV IR helpers."
    );
    assert!(
        macro_table.contains("primitive_fnv1a32")
            && !macro_table.contains("wrapping_mul(FNV1A32_PRIME)"),
        "Fix: host macro-table hashing must call the primitive FNV CPU implementation."
    );
    assert!(
        scan_engine.contains("fnv1a64_update_byte") && !scan_engine.contains("FNV1A64_PRIME"),
        "Fix: scan cache fingerprinting must use primitive FNV64 update helpers."
    );
}

#[test]
fn c_lexer_and_parser_hashing_reexport_primitive_fnv() {
    let keyword = include_str!("../src/parsing/c/lex/keyword.rs");
    let typedef_ids = include_str!("../src/parsing/c/parse/vast/ref_typedef/identifiers.rs");
    let gnu_builtin_catalog = include_str!("../src/parsing/c/parse/gnu_builtin_catalog.rs");
    let structure_harness = include_str!("../src/parsing/c/parse/structure/harness.rs");

    for (name, source) in [
        ("keyword", keyword),
        ("typedef_ids", typedef_ids),
        ("gnu_builtin_catalog", gnu_builtin_catalog),
        ("structure_harness", structure_harness),
    ] {
        assert!(
            !source.contains("0x811c_9dc5")
                && !source.contains("0x0100_0193")
                && !source.contains("2_166_136_261")
                && !source.contains("16_777_619"),
            "Fix: {name} must not redefine FNV-1a constants outside vyre-primitives."
        );
        assert!(
            source.contains("vyre_primitives::hash::fnv1a::fnv1a32"),
            "Fix: {name} must import or re-export primitive fnv1a32."
        );
    }
}

#[test]
fn c_semantic_and_reference_hashing_use_primitive_fnv() {
    let sema_intern = include_str!("../src/parsing/c/sema/intern.rs");
    let sema_registry_reference = include_str!("../src/parsing/c/sema/registry/reference.rs");
    let gpu_if_expression = include_str!("../src/parsing/c/preprocess/gpu_if_expression.rs");
    let reference_eval =
        include_str!("../../vyre-reference/src/dual_impls/hash/fnv1a/reference.rs");

    for (name, source) in [
        ("sema_intern", sema_intern),
        ("sema_registry_reference", sema_registry_reference),
        ("gpu_if_expression", gpu_if_expression),
        ("reference_eval", reference_eval),
    ] {
        assert!(
            !source.contains("0x811c_9dc5")
                && !source.contains("0x0100_0193")
                && !source.contains("FNV_OFFSET")
                && !source.contains("FNV_PRIME"),
            "Fix: {name} must not carry a local FNV-1a implementation."
        );
        assert!(
            source.contains("vyre_primitives::hash::fnv1a")
                || source.contains("fnv1a32_initial_expr")
                || source.contains("fnv1a32_update_byte"),
            "Fix: {name} must route FNV hashing through vyre-primitives."
        );
    }
}

#[test]
fn adler32_lives_in_primitives_and_libs_only_wraps_it() {
    let primitive_mod = include_str!("../../vyre-primitives/src/hash/mod.rs");
    let primitive_adler = include_str!("../../vyre-primitives/src/hash/adler32.rs");
    let libs_adler = include_str!("../src/hash/adler32.rs");

    assert!(
        primitive_mod.contains("pub mod adler32"),
        "Fix: Adler-32 must be a Tier-2.5 primitive hash module."
    );
    assert!(
        primitive_adler.contains("pub const ADLER32_MOD")
            && primitive_adler.contains("pub fn adler32_program"),
        "Fix: primitive Adler-32 must own constants, CPU reference, and executable IR builder."
    );
    assert!(
        libs_adler.contains("adler32_program") && libs_adler.contains("ADLER32_OP_ID"),
        "Fix: libs Adler-32 must delegate to the primitive builder."
    );
    assert!(
        !libs_adler.contains("const MOD_ADLER") && !libs_adler.contains("65_521"),
        "Fix: libs Adler-32 must not redefine the Adler modulus or algorithm body."
    );
}
