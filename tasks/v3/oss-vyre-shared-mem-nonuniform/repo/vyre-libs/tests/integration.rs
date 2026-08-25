//! Integration tests for vyre-libs.
//!
//! Each test proves a public function produces a valid Program
//! through its documented public path (via `use vyre_libs::...`),
//! not via internal module access.

#![cfg(all(
    feature = "math-linalg",
    feature = "math-scan",
    feature = "math-broadcast",
    feature = "nn-activation",
    feature = "nn-linear",
    feature = "matching-substring",
    feature = "crypto-fnv",
))]

use vyre::ir::{BufferAccess, MemoryKind, Program};
use vyre_libs::hash::fnv1a32;
use vyre_libs::math::{broadcast, dot, matmul, scan_prefix_sum};
use vyre_libs::nn::{linear, relu};
use vyre_libs::scan::substring_search;

fn assert_valid(p: &Program) {
    let errors = vyre::ir::validate(p);
    assert!(
        errors.is_empty(),
        "Program failed validation: {:?}",
        errors
            .iter()
            .map(|e| e.message().to_string())
            .collect::<Vec<_>>()
    );
}

fn assert_wrapped_in_region(p: &Program, expected_generator: &str) {
    let entry = p.entry();
    assert_eq!(
        entry.len(),
        1,
        "Every vyre-libs Program has exactly one top-level Node (a Region)."
    );
    match &entry[0] {
        vyre::ir::Node::Region { generator, .. } => {
            assert_eq!(
                generator.as_str(),
                expected_generator,
                "Region generator name must match the fully-qualified module path"
            );
        }
        other => panic!("expected Node::Region, got {other:?}"),
    }
}

#[test]
fn math_dot_produces_valid_region_program() {
    let p = dot("x", "y", "out", 3).unwrap();
    assert_valid(&p);
    assert_wrapped_in_region(&p, "vyre-libs::math::dot");
    assert_eq!(p.buffers().len(), 3);
    assert_eq!(p.buffers()[0].access(), BufferAccess::ReadOnly);
    assert_eq!(p.buffers()[2].access(), BufferAccess::ReadWrite);
}

#[test]
fn math_matmul_produces_valid_program() {
    let p = matmul("a", "b", "c", 4, 4, 4);
    assert_valid(&p);
    assert_wrapped_in_region(&p, "vyre-libs::math::matmul");
    assert_eq!(p.workgroup_size(), [256, 1, 1]);
}

#[test]
fn math_scan_prefix_sum_produces_valid_program() {
    let p = scan_prefix_sum("in", "out", 64);
    assert_valid(&p);
    assert_wrapped_in_region(&p, "vyre-libs::math::scan_prefix_sum");
}

#[test]
fn math_broadcast_produces_valid_program() {
    let p = broadcast("scalar", "wide", 64);
    assert_valid(&p);
    assert_wrapped_in_region(&p, "vyre-libs::math::broadcast");
}

#[test]
fn nn_linear_produces_valid_program() {
    let p = linear("x", "w", "b", "out", 8, 4).unwrap();
    assert_valid(&p);
    assert_wrapped_in_region(&p, "vyre-libs::nn::linear");
    assert_eq!(p.buffers().len(), 4);
}

#[test]
fn nn_relu_produces_valid_program() {
    let p = relu("input", "output", 64);
    assert_valid(&p);
    assert_wrapped_in_region(&p, "vyre-libs::nn::relu");
}

#[test]
fn matching_substring_produces_valid_program() {
    let p = substring_search("haystack", "needle", "matches", 16, 5);
    assert_valid(&p);
    assert_wrapped_in_region(&p, "vyre-libs::matching::substring_search");
}

#[test]
fn crypto_fnv1a32_produces_valid_program() {
    let p = fnv1a32("data", "hash");
    assert_valid(&p);
    assert_wrapped_in_region(&p, "vyre-libs::hash::fnv1a32");
    assert_eq!(p.workgroup_size(), [1, 1, 1]);
}

#[test]
fn every_public_fn_returns_program_with_buffers() {
    // Guards against a future refactor that accidentally returns an
    // empty Program from any public function  -  structural smoke test
    // every program has non-zero buffer count.
    assert!(dot("x", "y", "z", 3).unwrap().buffers().len() >= 2);
    assert!(matmul("a", "b", "c", 2, 2, 2).buffers().len() >= 3);
    assert!(scan_prefix_sum("i", "o", 4).buffers().len() >= 2);
    assert!(broadcast("s", "d", 8).buffers().len() >= 2);
    assert!(linear("x", "w", "b", "o", 2, 2).unwrap().buffers().len() >= 4);
    assert!(relu("i", "o", 8).buffers().len() >= 2);
    assert!(substring_search("h", "n", "m", 8, 1).buffers().len() >= 3);
    assert!(fnv1a32("d", "h").buffers().len() >= 2);
}

#[test]
fn region_bodies_are_nonempty() {
    // Sanity: every Region has at least one Node in its body.
    // Empty bodies would indicate a non-functional composition  -  LAW 1
    // violation hiding behind the Region wrapper.
    let p = dot("x", "y", "z", 3).unwrap();
    let vyre::ir::Node::Region { body, .. } = &p.entry()[0] else {
        panic!("expected Region");
    };
    assert!(!body.is_empty(), "Region body must not be empty");
    // Also confirm every buffer uses a concrete kind.
    for buf in p.buffers() {
        assert!(
            matches!(
                buf.kind(),
                MemoryKind::Readonly | MemoryKind::Global | MemoryKind::Uniform
            ),
            "buffer kind must be concrete: {:?}",
            buf.kind()
        );
    }
}
