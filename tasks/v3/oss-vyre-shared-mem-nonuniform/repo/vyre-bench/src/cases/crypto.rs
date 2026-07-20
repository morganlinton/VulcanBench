//! `crypto.aes_ctr.encrypt.10mb`  -  AES-128-CTR encryption over 10MB.
//!
//! GPU kernel: counter-mode AES where each thread encrypts one 16-byte
//! counter block using the AES-128 round function, then XORs the generated
//! keystream with plaintext.
//!
//! CPU baseline: OpenSSL EVP AES-128-CTR, which routes through AES-NI on
//! x86_64 hosts with AES acceleration.

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::resident::{
    dispatch_program_timed, input_bytes_total, transfer_accounting, ResidentInputSet,
};
use crate::api::suite::SuiteKind;
use openssl::symm::{Cipher, Crypter, Mode};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_primitives::wire::pack_u32_iter;

// 100MB = 6_553_600 blocks of 16 bytes each
// Reduced to 10MB for smoke suite to keep tests fast
const BLOCK_COUNT: u32 = 655_360; // 10MB / 16 bytes = 655360 blocks
const BLOCK_SIZE_WORDS: u32 = 4; // 16 bytes = 4 u32
const TOTAL_WORDS: u32 = BLOCK_COUNT * BLOCK_SIZE_WORDS;

const HONEST_SUITES: &[SuiteKind] = &[
    SuiteKind::Honest,
    SuiteKind::Deep,
    SuiteKind::Release,
    SuiteKind::Smoke,
];

const AES_BLOCK_BYTES: usize = 16;
const AES_ROUNDS: usize = 10;
const ROUND_KEY_BYTES: usize = AES_BLOCK_BYTES * (AES_ROUNDS + 1);
const ROUND_KEY_WORDS: usize = 4 * (AES_ROUNDS + 1);
const AES_TE0_OFFSET: usize = 64;
const AES_TE1_OFFSET: usize = AES_TE0_OFFSET + 256;
const AES_TE2_OFFSET: usize = AES_TE1_OFFSET + 256;
const AES_TE3_OFFSET: usize = AES_TE2_OFFSET + 256;
const AES_SBOX_OFFSET: usize = AES_TE3_OFFSET + 256;
const AES_TABLE_WORDS: usize = AES_SBOX_OFFSET + AES_SBOX.len();
const AES_IV: [u8; AES_BLOCK_BYTES] = [0u8; AES_BLOCK_BYTES];

const AES_SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const AES_RCON: [u8; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1B, 0x36];

pub struct AesCtrEncrypt;

struct AesCtrPrepared {
    program: Program,
    plaintext_bytes: Vec<u8>,
    key_bytes: [u8; AES_BLOCK_BYTES],
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    resident: Option<ResidentInputSet>,
}

impl BenchCase for AesCtrEncrypt {
    fn id(&self) -> BenchId {
        BenchId("crypto.aes_ctr.encrypt.10mb".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "AES-CTR Encrypt 10MB".to_string(),
            description: "AES-128 counter-mode encryption, 10MB stream, per-block parallel"
                .to_string(),
            tags: vec![
                "honest".to_string(),
                "crypto".to_string(),
                "compute-bound".to_string(),
            ],
            layer: BenchLayer::Honest,
            workload: WorkloadClass::Honest,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        HONEST_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: Some((TOTAL_WORDS as u64) * 4 * 2), // input + output
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_3x(
            "AES-CTR encryption",
            "OpenSSL EVP AES-128-CTR",
            "OpenSSL 0.10.78 EVP path with hardware AES acceleration",
        ))
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        let bytes = prepared
            .downcast_ref::<AesCtrPrepared>()
            .map(|prepared| prepared.plaintext_bytes.len() as u64)
            .unwrap_or_else(|| TOTAL_WORDS as u64 * 4);
        (bytes, bytes) // read plaintext, write ciphertext
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let body = aes_ctr_kernel_body();
        let prog = Program::wrapped(
            vec![
                BufferDecl::storage("plaintext", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(TOTAL_WORDS),
                BufferDecl::output("ciphertext", 1, DataType::U32).with_count(TOTAL_WORDS),
                BufferDecl::storage("aes_tables", 2, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(AES_TABLE_WORDS as u32),
            ],
            [256, 1, 1],
            body,
        );
        let plaintext_bytes = pack_u32_iter(0..TOTAL_WORDS);
        let key_bytes = [
            0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB,
            0xCD, 0xEF,
        ];
        let inputs = vec![plaintext_bytes.clone(), aes_table_bytes(key_bytes)];
        let input_bytes_total = input_bytes_total(&inputs);
        let resident = ResidentInputSet::upload_program_ordered_with_zeroed_outputs_optional(
            ctx,
            &prog,
            &inputs,
            "aes-ctr bench",
        )?;
        Ok(Box::new(AesCtrPrepared {
            program: prog,
            plaintext_bytes,
            key_bytes,
            inputs,
            input_bytes_total,
            resident,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<AesCtrPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared.downcast_ref::<AesCtrPrepared>().ok_or_else(|| {
            BenchError::ExecutionFailed("aes-ctr prepared payload type mismatch".to_string())
        })?;

        let dispatch = dispatch_program_timed(
            ctx,
            &prepared.program,
            prepared.resident.as_ref(),
            &prepared.inputs,
            &ctx.dispatch_config,
        )?;
        let resident_used = dispatch.resident_used;
        let timed = dispatch.timed;
        let outputs = timed.outputs;

        // CPU baseline
        let start_ref = std::time::Instant::now();
        let cpu_result = cpu_openssl_aes_ctr(&prepared.plaintext_bytes, &prepared.key_bytes)?;
        let elapsed_ref = start_ref.elapsed().as_nanos() as u64;
        let input_bytes = prepared.input_bytes_total;
        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting = transfer_accounting(input_bytes, output_bytes, resident_used);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(input_bytes),
                output_bytes: Some(output_bytes),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(elapsed_ref),
                input_bytes: Some(prepared.plaintext_bytes.len() as u64),
                output_bytes: Some(cpu_result.len() as u64),
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(vec![cpu_result]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

fn aes_ctr_kernel_body() -> Vec<Node> {
    let mut body = vec![Node::let_bind("tid", Expr::gid_x())];
    let mut guarded = Vec::new();
    guarded.push(Node::let_bind("w0", round_key_load(Expr::u32(0))));
    guarded.push(Node::let_bind("w1", round_key_load(Expr::u32(1))));
    guarded.push(Node::let_bind("w2", round_key_load(Expr::u32(2))));
    guarded.push(Node::let_bind(
        "w3",
        Expr::bitxor(counter_word_expr(), round_key_load(Expr::u32(3))),
    ));
    guarded.push(Node::loop_for(
        "round",
        Expr::u32(1),
        Expr::u32(AES_ROUNDS as u32),
        aes_t_round_loop_body(),
    ));
    aes_final_round_nodes(&mut guarded);
    guarded.push(Node::let_bind(
        "off",
        Expr::mul(Expr::var("tid"), Expr::u32(BLOCK_SIZE_WORDS)),
    ));
    for word in 0..BLOCK_SIZE_WORDS {
        guarded.push(Node::store(
            "ciphertext",
            Expr::add(Expr::var("off"), Expr::u32(word)),
            Expr::bitxor(
                Expr::load("plaintext", Expr::add(Expr::var("off"), Expr::u32(word))),
                Expr::var(format!("w{word}")),
            ),
        ));
    }
    body.push(Node::if_then(
        Expr::lt(Expr::var("tid"), Expr::u32(BLOCK_COUNT)),
        guarded,
    ));
    body
}

fn counter_word_expr() -> Expr {
    Expr::bitor(
        Expr::bitor(
            Expr::bitand(Expr::shr(Expr::var("tid"), Expr::u32(24)), Expr::u32(0xff)),
            Expr::bitand(Expr::shr(Expr::var("tid"), Expr::u32(8)), Expr::u32(0xff00)),
        ),
        Expr::bitor(
            Expr::bitand(
                Expr::shl(Expr::var("tid"), Expr::u32(8)),
                Expr::u32(0xff0000),
            ),
            Expr::shl(
                Expr::bitand(Expr::var("tid"), Expr::u32(0xff)),
                Expr::u32(24),
            ),
        ),
    )
}

fn aes_t_round_loop_body() -> Vec<Node> {
    let mut nodes = Vec::with_capacity(8);
    for col in 0..4 {
        let expr = xor5(
            table_load(AES_TE0_OFFSET, word_byte_expr(column_word(col), 0)),
            table_load(
                AES_TE1_OFFSET,
                word_byte_expr(column_word((col + 1) % 4), 1),
            ),
            table_load(
                AES_TE2_OFFSET,
                word_byte_expr(column_word((col + 2) % 4), 2),
            ),
            table_load(
                AES_TE3_OFFSET,
                word_byte_expr(column_word((col + 3) % 4), 3),
            ),
            round_key_load(Expr::add(
                Expr::mul(Expr::var("round"), Expr::u32(4)),
                Expr::u32(col as u32),
            )),
        );
        nodes.push(Node::let_bind(format!("round_w{col}"), expr));
    }
    for col in 0..4 {
        nodes.push(Node::assign(
            format!("w{col}"),
            Expr::var(format!("round_w{col}")),
        ));
    }
    nodes
}

fn aes_final_round_nodes(nodes: &mut Vec<Node>) {
    for col in 0..4 {
        let packed = pack_word([
            table_load(AES_SBOX_OFFSET, word_byte_expr(column_word(col), 0)),
            table_load(
                AES_SBOX_OFFSET,
                word_byte_expr(column_word((col + 1) % 4), 1),
            ),
            table_load(
                AES_SBOX_OFFSET,
                word_byte_expr(column_word((col + 2) % 4), 2),
            ),
            table_load(
                AES_SBOX_OFFSET,
                word_byte_expr(column_word((col + 3) % 4), 3),
            ),
        ]);
        nodes.push(Node::let_bind(
            format!("final_w{col}"),
            Expr::bitxor(
                packed,
                round_key_load(Expr::u32((AES_ROUNDS * 4 + col) as u32)),
            ),
        ));
    }
    for col in 0..4 {
        nodes.push(Node::assign(
            format!("w{col}"),
            Expr::var(format!("final_w{col}")),
        ));
    }
}

fn column_word(col: usize) -> Expr {
    Expr::var(format!("w{col}"))
}

fn word_byte_expr(word: Expr, byte: u32) -> Expr {
    Expr::bitand(Expr::shr(word, Expr::u32(byte * 8)), Expr::u32(0xff))
}

fn round_key_load(index: Expr) -> Expr {
    Expr::load("aes_tables", index)
}

fn table_load(offset: usize, index: Expr) -> Expr {
    Expr::load("aes_tables", Expr::add(Expr::u32(offset as u32), index))
}

fn xor5(a: Expr, b: Expr, c: Expr, d: Expr, e: Expr) -> Expr {
    Expr::bitxor(xor4(a, b, c, d), e)
}

fn xor4(a: Expr, b: Expr, c: Expr, d: Expr) -> Expr {
    Expr::bitxor(Expr::bitxor(a, b), Expr::bitxor(c, d))
}

fn pack_word(bytes: [Expr; 4]) -> Expr {
    Expr::bitor(
        Expr::bitor(bytes[0].clone(), Expr::shl(bytes[1].clone(), Expr::u32(8))),
        Expr::bitor(
            Expr::shl(bytes[2].clone(), Expr::u32(16)),
            Expr::shl(bytes[3].clone(), Expr::u32(24)),
        ),
    )
}

fn u32_words_bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

fn aes_table_bytes(key: [u8; AES_BLOCK_BYTES]) -> Vec<u8> {
    let round_keys = expand_aes128_key_words(key);
    let (te0, te1, te2, te3) = aes_t_tables();
    let mut words = vec![0u32; AES_TABLE_WORDS];
    words[..ROUND_KEY_WORDS].copy_from_slice(&round_keys);
    words[AES_TE0_OFFSET..AES_TE0_OFFSET + 256].copy_from_slice(&te0);
    words[AES_TE1_OFFSET..AES_TE1_OFFSET + 256].copy_from_slice(&te1);
    words[AES_TE2_OFFSET..AES_TE2_OFFSET + 256].copy_from_slice(&te2);
    words[AES_TE3_OFFSET..AES_TE3_OFFSET + 256].copy_from_slice(&te3);
    for (dst, src) in words[AES_SBOX_OFFSET..AES_SBOX_OFFSET + AES_SBOX.len()]
        .iter_mut()
        .zip(AES_SBOX.iter().copied())
    {
        *dst = u32::from(src);
    }
    u32_words_bytes(&words)
}

fn aes_t_tables() -> ([u32; 256], [u32; 256], [u32; 256], [u32; 256]) {
    let mut te0 = [0u32; 256];
    let mut te1 = [0u32; 256];
    let mut te2 = [0u32; 256];
    let mut te3 = [0u32; 256];
    for (idx, sboxed) in AES_SBOX.iter().copied().enumerate() {
        let mul2 = gf_mul2_byte(sboxed);
        let mul3 = mul2 ^ sboxed;
        te0[idx] = pack_u8_word([mul2, sboxed, sboxed, mul3]);
        te1[idx] = pack_u8_word([mul3, mul2, sboxed, sboxed]);
        te2[idx] = pack_u8_word([sboxed, mul3, mul2, sboxed]);
        te3[idx] = pack_u8_word([sboxed, sboxed, mul3, mul2]);
    }
    (te0, te1, te2, te3)
}

fn gf_mul2_byte(value: u8) -> u8 {
    let doubled = value << 1;
    if value & 0x80 == 0 {
        doubled
    } else {
        doubled ^ 0x1b
    }
}

fn pack_u8_word(bytes: [u8; 4]) -> u32 {
    u32::from(bytes[0])
        | (u32::from(bytes[1]) << 8)
        | (u32::from(bytes[2]) << 16)
        | (u32::from(bytes[3]) << 24)
}

fn expand_aes128_key_words(key: [u8; AES_BLOCK_BYTES]) -> [u32; ROUND_KEY_WORDS] {
    let expanded = expand_aes128_key(key);
    let mut words = [0u32; ROUND_KEY_WORDS];
    for (word, chunk) in words.iter_mut().zip(expanded.chunks_exact(4)) {
        *word = pack_u8_word([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    words
}

fn expand_aes128_key(key: [u8; AES_BLOCK_BYTES]) -> [u8; ROUND_KEY_BYTES] {
    let mut expanded = [0u8; ROUND_KEY_BYTES];
    expanded[..AES_BLOCK_BYTES].copy_from_slice(&key);
    let mut generated = AES_BLOCK_BYTES;
    let mut rcon = 0usize;
    let mut temp = [0u8; 4];
    while generated < ROUND_KEY_BYTES {
        temp.copy_from_slice(&expanded[generated - 4..generated]);
        if generated % AES_BLOCK_BYTES == 0 {
            temp.rotate_left(1);
            for byte in &mut temp {
                *byte = AES_SBOX[*byte as usize];
            }
            temp[0] ^= AES_RCON[rcon];
            rcon += 1;
        }
        for value in temp {
            expanded[generated] = expanded[generated - AES_BLOCK_BYTES] ^ value;
            generated += 1;
        }
    }
    expanded
}

fn cpu_openssl_aes_ctr(
    plaintext: &[u8],
    key: &[u8; AES_BLOCK_BYTES],
) -> Result<Vec<u8>, BenchError> {
    let mut crypter = Crypter::new(Cipher::aes_128_ctr(), Mode::Encrypt, key, Some(&AES_IV))
        .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
    crypter.pad(false);
    let mut output = vec![0u8; plaintext.len() + AES_BLOCK_BYTES];
    let mut written = crypter
        .update(plaintext, &mut output)
        .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
    written += crypter
        .finalize(&mut output[written..])
        .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
    output.truncate(written);
    Ok(output)
}

inventory::submit! {
    &AesCtrEncrypt as &'static dyn BenchCase
}
