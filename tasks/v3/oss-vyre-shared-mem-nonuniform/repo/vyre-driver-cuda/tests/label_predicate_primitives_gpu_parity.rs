//! Parity test: vyre-primitives label::resolve_family + predicate
//! tag-family wrappers (in_function, in_file, in_package) match
//! their CPU oracles.

#![cfg(test)]

mod common;

use common::{cuda_u32_bitset_output, with_live_backend};
use vyre_primitives::label::resolve_family::{cpu_ref as resolve_family_cpu, resolve_family};
use vyre_primitives::predicate::in_file::{cpu_ref as in_file_cpu, in_file};
use vyre_primitives::predicate::in_function::{cpu_ref as in_func_cpu, in_function};
use vyre_primitives::predicate::in_package::{cpu_ref as in_pkg_cpu, in_package};

fn run_resolve_family(node_tags: &[u32], family: u32) -> Vec<u32> {
    let n = node_tags.len() as u32;
    let program = resolve_family("tags", "out", n, family);
    with_live_backend("resolve label family", |backend| {
        cuda_u32_bitset_output(backend, &program, n, node_tags, "resolve_family")
    })
}

#[test]
fn cuda_resolve_family_matches_cpu() {
    let tags = vec![0x01u32, 0x02, 0x06, 0x04];
    let family = 0x02u32;
    let cpu = resolve_family_cpu(&tags, family);
    let gpu = run_resolve_family(&tags, family);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0b0110]);
}

#[test]
fn cuda_resolve_family_zero_mask_yields_zero() {
    let tags = vec![0x01u32, 0x02];
    let cpu = resolve_family_cpu(&tags, 0x00);
    let gpu = run_resolve_family(&tags, 0x00);
    assert_eq!(gpu, cpu);
    assert_eq!(gpu, vec![0u32]);
}

fn run_predicate<B>(program_builder: B, node_tags: &[u32]) -> Vec<u32>
where
    B: FnOnce(&str, &str, u32) -> vyre::ir::Program,
{
    let n = node_tags.len() as u32;
    let program = program_builder("tags", "out", n);
    with_live_backend("tag-family predicate", |backend| {
        cuda_u32_bitset_output(backend, &program, n, node_tags, "tag-family predicate")
    })
}

#[test]
fn cuda_in_function_matches_cpu() {
    // FUNCTION = tag_family value; cpu_ref uses it. Just compare GPU vs CPU.
    let tags = vec![0x01u32, 0x02, 0x04, 0x08, 0x01, 0x10];
    let cpu = in_func_cpu(&tags);
    let gpu = run_predicate(in_function, &tags);
    assert_eq!(gpu, cpu);
}

#[test]
fn cuda_in_file_matches_cpu() {
    let tags = vec![0x01u32, 0x02, 0x04, 0x08, 0x10, 0x20];
    let cpu = in_file_cpu(&tags);
    let gpu = run_predicate(in_file, &tags);
    assert_eq!(gpu, cpu);
}

#[test]
fn cuda_in_package_matches_cpu() {
    let tags = vec![0x01u32, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40];
    let cpu = in_pkg_cpu(&tags);
    let gpu = run_predicate(in_package, &tags);
    assert_eq!(gpu, cpu);
}
