//! Live-GPU parity for `Expr::subgroup_reduce` on the CUDA/PTX backend.
//!
//! The IR/WGSL/oracle contract for a subgroup reduction is *all-lane broadcast*:
//! every active lane receives the full reduction (matching `naga`'s
//! `subgroupAdd`, PTX `redux.sync` on integers, and the reference interpreter).
//! Each lane stores its reduction result into a distinct output slot, so a
//! correct reduction makes *every* slot hold the full reduction; a
//! reduce-to-lane-0 path would leave lanes 1.. with partial values.
//!
//! `in` is declared read-only so the ONLY result buffer the dispatch returns is
//! `out` (`outputs[0]`). (A read-write `in` is also returned as an output, ahead
//! of `out`, so reading `outputs[0]` would read back the unchanged input — a
//! test trap, not a kernel error.)

#![cfg(test)]

mod common;

use common::with_live_backend;
use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre::DispatchConfig;

const LANES: u32 = 32;

fn bytes_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

fn bytes_u32(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

/// `out[gid] = subgroup_reduce(op, in[gid])` over a single 32-lane subgroup.
/// `in` is read-only (NOT a result), so `out` is `outputs[0]`.
fn subgroup_reduce_program(reduce: fn(Expr) -> Expr, dtype: DataType) -> Program {
    let body = vec![
        Node::let_bind("gid", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(Expr::var("gid"), Expr::buf_len("out")),
            vec![Node::store(
                "out",
                Expr::var("gid"),
                reduce(Expr::load("in", Expr::var("gid"))),
            )],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::read("in", 0, dtype.clone()).with_count(LANES),
            BufferDecl::output("out", 1, dtype).with_count(LANES),
        ],
        [LANES, 1, 1],
        vec![Node::Region {
            generator: "test::subgroup_reduce_parity".into(),
            source_region: None,
            body: std::sync::Arc::new(body),
        }],
    )
}

fn run_reduce_bytes(reduce: fn(Expr) -> Expr, dtype: DataType, input: &[u8]) -> Vec<u8> {
    let program = subgroup_reduce_program(reduce, dtype);
    let mut config = DispatchConfig::default();
    config.grid_override = Some([1, 1, 1]);
    let outputs = with_live_backend("subgroup reduce parity", |backend| {
        backend
            .dispatch(&program, &[input.to_vec()], &config)
            .unwrap_or_else(|error| panic!("Fix: CUDA subgroup reduce dispatch failed: {error}"))
    });
    // `in` is read-only, so the sole returned buffer is `out`.
    assert_eq!(
        outputs.len(),
        1,
        "read-only `in` must not be returned as an output; only `out` should be"
    );
    outputs.into_iter().next().expect("one output buffer")
}

fn run_reduce_f32(reduce: fn(Expr) -> Expr, input: &[f32]) -> Vec<f32> {
    assert_eq!(input.len(), LANES as usize);
    let bytes: Vec<u8> = input.iter().flat_map(|v| v.to_le_bytes()).collect();
    bytes_f32(&run_reduce_bytes(reduce, DataType::F32, &bytes))
}

fn run_reduce_u32(reduce: fn(Expr) -> Expr, input: &[u32]) -> Vec<u32> {
    assert_eq!(input.len(), LANES as usize);
    let bytes: Vec<u8> = input.iter().flat_map(|v| v.to_le_bytes()).collect();
    bytes_u32(&run_reduce_bytes(reduce, DataType::U32, &bytes))
}

#[test]
fn cuda_subgroup_add_f32_broadcasts_full_reduction_to_every_lane() {
    let input: Vec<f32> = (0..LANES).map(|i| (i as f32) + 1.0).collect();
    let expected: f32 = input.iter().sum();
    assert_eq!(expected, 528.0, "1.0+2.0+..+32.0 sanity");

    let out = run_reduce_f32(Expr::subgroup_add, &input);

    for (lane, &value) in out.iter().enumerate() {
        assert_eq!(
            value, expected,
            "f32 subgroup_add must broadcast the full sum ({expected}) to lane {lane}, got {value}"
        );
    }
}

#[test]
fn cuda_subgroup_max_f32_broadcasts_full_reduction_to_every_lane() {
    // Smallest value is at lane 0, max (12.5) at lane 31: a reduce-to-lane-0
    // path would leave lane 0 with -3.0 instead of the broadcast max.
    let input: Vec<f32> = (0..LANES).map(|i| (i as f32) * 0.5 - 3.0).collect();
    let expected = input.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert_eq!(expected, 12.5, "max of 0.5*i-3 over 0..32");

    let out = run_reduce_f32(Expr::subgroup_max, &input);

    for (lane, &value) in out.iter().enumerate() {
        assert_eq!(
            value, expected,
            "f32 subgroup_max must broadcast the full max ({expected}) to lane {lane}, got {value}"
        );
    }
}

#[test]
fn cuda_subgroup_mul_f32_broadcasts_full_product_to_every_lane() {
    // f32 product goes through the SAME shfl.idx XOR butterfly as the integer
    // product, but with a `mul.f32` combine (bitcast through b32 around the
    // shuffle). All factors are exact dyadic f32 values placed at scattered
    // lanes, so the product (2*3*0.5*4 = 12.0) is exact and order-independent —
    // a reduce-to-lane-0 path would leave most lanes at the 1.0 fill.
    let mut input = vec![1.0_f32; LANES as usize];
    for (slot, factor) in [2.0_f32, 3.0, 0.5, 4.0].into_iter().enumerate() {
        input[slot * 8] = factor;
    }
    let expected: f32 = input.iter().product();
    assert_eq!(expected, 12.0, "2*3*0.5*4 sanity");

    let out = run_reduce_f32(Expr::subgroup_mul, &input);

    for (lane, &value) in out.iter().enumerate() {
        assert_eq!(
            value, expected,
            "f32 subgroup_mul must broadcast the full product ({expected}) to lane {lane}, got {value}"
        );
    }
}

#[test]
fn cuda_subgroup_add_u32_broadcasts_full_reduction_to_every_lane() {
    let input: Vec<u32> = (0..LANES).map(|i| i + 1).collect();
    let expected: u32 = input.iter().sum();
    assert_eq!(expected, 528, "1+2+..+32 sanity");

    let out = run_reduce_u32(Expr::subgroup_add, &input);

    for (lane, &value) in out.iter().enumerate() {
        assert_eq!(
            value, expected,
            "u32 subgroup_add must broadcast the full sum ({expected}) to lane {lane}, got {value}"
        );
    }
}

#[test]
fn cuda_subgroup_mul_u32_broadcasts_full_product_to_every_lane() {
    // Integer product has no `redux.sync`; it lowers to the shfl.idx XOR
    // butterfly. Most lanes are 1 with a few small primes so the product
    // (2*3*5*7*11 = 2310) fits u32 and is order-independent.
    let mut input = vec![1u32; LANES as usize];
    for (slot, prime) in [2u32, 3, 5, 7, 11].into_iter().enumerate() {
        input[slot * 5] = prime;
    }
    let expected: u32 = input.iter().product();
    assert_eq!(expected, 2310, "2*3*5*7*11 sanity");

    let out = run_reduce_u32(Expr::subgroup_mul, &input);

    for (lane, &value) in out.iter().enumerate() {
        assert_eq!(
            value, expected,
            "u32 subgroup_mul must broadcast the full product ({expected}) to lane {lane}, got {value}"
        );
    }
}

#[test]
fn cuda_subgroup_max_u32_broadcasts_full_reduction_to_every_lane() {
    // Interleaved so the max (63) is NOT at lane 0 — a reduce-to-lane-0 path
    // would leave most lanes wrong.
    let input: Vec<u32> = (0..LANES).map(|i| (i * 2) ^ 1).collect();
    let expected: u32 = input.iter().copied().max().expect("non-empty");
    assert_eq!(expected, 63, "max of (2i)^1 over 0..32");

    let out = run_reduce_u32(Expr::subgroup_max, &input);

    for (lane, &value) in out.iter().enumerate() {
        assert_eq!(
            value, expected,
            "u32 subgroup_max must broadcast the full max ({expected}) to lane {lane}, got {value}"
        );
    }
}
