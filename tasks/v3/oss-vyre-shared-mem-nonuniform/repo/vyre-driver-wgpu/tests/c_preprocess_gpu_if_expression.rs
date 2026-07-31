//! Hardware WGPU parity tests for C preprocessor `#if` expression kernels.

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_libs::parsing::c::lex::tokens::TOK_PP_IF;
use vyre_libs::parsing::c::parse::gnu_builtins::{
    gpu_builtin_hash_table_words, GPU_BUILTIN_HASH_TABLE_SEED, GPU_BUILTIN_HASH_TABLE_SIZE,
};
use vyre_libs::parsing::c::preprocess::gpu_if_expression::gpu_if_expression;

fn pack_u32_words(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

fn pack_source_bytes(source: &[u8]) -> Vec<u8> {
    let mut packed = Vec::with_capacity(source.len().div_ceil(4).max(1) * 4);
    for chunk in source.chunks(4) {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        packed.extend_from_slice(&word);
    }
    if packed.is_empty() {
        packed.extend_from_slice(&0u32.to_le_bytes());
    }
    packed
}

fn run_gpu_ifs(sources: &[&[u8]]) -> Vec<u32> {
    run_gpu_ifs_with_macros(sources, &[], &[])
}

fn run_gpu_ifs_with_macros(
    sources: &[&[u8]],
    macro_names: &[&[u8]],
    macro_values: &[u32],
) -> Vec<u32> {
    assert_eq!(
        macro_names.len(),
        macro_values.len(),
        "Fix: each packed macro name needs one integer value"
    );
    let mut starts = Vec::with_capacity(sources.len());
    let mut lens = Vec::with_capacity(sources.len());
    let mut combined = Vec::new();
    for source in sources {
        starts.push(combined.len() as u32);
        lens.push(source.len() as u32);
        combined.extend_from_slice(source);
    }
    let mut packed_macro_names = Vec::new();
    let mut macro_offsets = Vec::with_capacity(macro_names.len() + 1);
    macro_offsets.push(0);
    for name in macro_names {
        packed_macro_names.extend_from_slice(name);
        macro_offsets.push(packed_macro_names.len() as u32);
    }
    if packed_macro_names.is_empty() {
        packed_macro_names.extend_from_slice(&0u32.to_le_bytes());
    }
    let mut packed_macro_values = gpu_builtin_hash_table_words();
    packed_macro_values.extend_from_slice(macro_values);

    let program = gpu_if_expression(sources.len() as u32, combined.len() as u32);
    let inputs = vec![
        pack_u32_words(&starts),
        pack_u32_words(&lens),
        pack_u32_words(&vec![TOK_PP_IF; sources.len()]),
        pack_source_bytes(&combined),
        pack_source_bytes(&packed_macro_names),
        pack_u32_words(&macro_offsets),
        pack_u32_words(&packed_macro_values),
        pack_u32_words(&vec![0; sources.len().max(1)]),
    ];

    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: WGPU backend must acquire the local GPU for C preprocessor parity tests");
    let outputs = backend
        .dispatch(&program, &inputs, &DispatchConfig::default())
        .expect("Fix: gpu_if_expression must dispatch on the WGPU backend");
    let bytes = outputs
        .first()
        .expect("Fix: gpu_if_expression must return directive_values output");
    bytes
        .chunks_exact(4)
        .take(sources.len())
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn run_gpu_if(source: &[u8]) -> u32 {
    run_gpu_ifs(&[source])[0]
}

fn packed_byte_expr(buffer: &'static str, byte_index: u32) -> Expr {
    Expr::bitand(
        Expr::shr(
            Expr::load(buffer, Expr::u32(byte_index / 4)),
            Expr::u32((byte_index % 4) * 8),
        ),
        Expr::u32(0xff),
    )
}

fn run_hash_table_probe(name: &[u8]) -> u32 {
    let mut body = vec![Node::let_bind("hash", Expr::u32(0x811c_9dc5))];
    for idx in 0..name.len() as u32 {
        body.push(Node::assign(
            "hash",
            Expr::mul(
                Expr::bitxor(Expr::var("hash"), packed_byte_expr("source", idx)),
                Expr::u32(0x0100_0193),
            ),
        ));
    }
    body.push(Node::let_bind(
        "slot",
        Expr::rem(
            Expr::mul(Expr::var("hash"), Expr::u32(GPU_BUILTIN_HASH_TABLE_SEED)),
            Expr::u32(GPU_BUILTIN_HASH_TABLE_SIZE as u32),
        ),
    ));
    body.push(Node::let_bind(
        "found_hash",
        Expr::load("builtin_hashes", Expr::var("slot")),
    ));
    body.push(Node::store(
        "out",
        Expr::u32(0),
        Expr::select(
            Expr::eq(Expr::var("found_hash"), Expr::var("hash")),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("source", 0, BufferAccess::ReadOnly, DataType::U32).with_count(0),
            BufferDecl::storage("builtin_hashes", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(0),
            BufferDecl::storage("out", 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_output_byte_range(0..4),
        ],
        [1, 1, 1],
        body,
    );
    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: WGPU backend must acquire the local GPU for builtin hash tests");
    let outputs = backend
        .dispatch(
            &program,
            &[
                pack_source_bytes(name),
                pack_u32_words(&gpu_builtin_hash_table_words()),
                pack_u32_words(&[0]),
            ],
            &DispatchConfig::default(),
        )
        .expect("Fix: builtin hash-table probe must dispatch on WGPU");
    let bytes = outputs
        .first()
        .expect("Fix: builtin hash-table probe must return one output");
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[test]
fn wgpu_builtin_hash_table_probe_finds_catalog_entry() {
    assert_eq!(run_hash_table_probe(b"__builtin_expect"), 1);
}

#[test]
fn wgpu_if_expression_literal_probe_runs_full_evaluator() {
    assert_eq!(run_gpu_if(b"#if 1\n"), 1);
}

#[test]
fn wgpu_if_expression_handles_generic_has_operators_on_device() {
    assert_eq!(
        run_gpu_ifs(&[
            b"#if !__has_attribute(visibility)\n".as_slice(),
            b"#if __has_feature(c_static_assert)\n".as_slice(),
        ]),
        vec![1, 0]
    );
}

#[test]
fn wgpu_if_expression_reads_object_like_macro_values_after_builtin_table_prefix() {
    assert_eq!(
        run_gpu_ifs_with_macros(
            &[
                b"#if FOO == 7\n".as_slice(),
                b"#if ZERO\n".as_slice(),
                b"#if MISSING\n".as_slice(),
                b"#if FOO + 1 == 8\n".as_slice(),
            ],
            &[b"FOO".as_slice(), b"ZERO".as_slice()],
            &[7, 0],
        ),
        vec![1, 0, 0, 1]
    );
}

#[test]
fn wgpu_if_expression_handles_has_builtin_on_device() {
    assert_eq!(
        run_gpu_ifs(&[
            b"#if __has_builtin(__builtin_expect)\n".as_slice(),
            b"#if __has_builtin(__builtin_memcpy)\n".as_slice(),
            b"#if __has_builtin ( __builtin_trap )\n".as_slice(),
            b"#if __has_builtin(__vyre_not_a_builtin)\n".as_slice(),
            b"#if __has_constexpr_builtin(__builtin_expect)\n".as_slice(),
            b"#if __has_constexpr_builtin(__vyre_not_a_builtin)\n".as_slice(),
            b"#if !__has_builtin(__vyre_not_a_builtin)\n".as_slice(),
            b"#if 1 && __has_builtin(__builtin_memcpy)\n".as_slice(),
            b"#if 1 && __has_constexpr_builtin(__builtin_expect)\n".as_slice(),
            b"#if 1 && __has_builtin(__builtin_trap)\n".as_slice(),
            b"#if 1 && __has_builtin(__builtin_unreachable)\n".as_slice(),
            b"#if 1 && __has_builtin(__builtin_alloca)\n".as_slice(),
            b"#if 1 && __has_builtin(__builtin_bswap64)\n".as_slice(),
            b"#if 1 && __has_builtin(__builtin_isnan)\n".as_slice(),
            b"#if 1 && __has_builtin(__builtin_va_start)\n".as_slice(),
            b"#if 1 && __has_builtin(__builtin_allocax)\n".as_slice(),
        ]),
        vec![1, 1, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0]
    );
}
