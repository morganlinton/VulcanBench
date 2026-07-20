//! IR aliasing regression tests.

#![cfg(feature = "c-parser")]
use std::collections::HashSet;
use std::sync::Arc;

use vyre::ir::{validate, BufferAccess, Node, Program};
use vyre_foundation::composition::mark_self_exclusive_region;
use vyre_libs::decode::{base64_decode, hex_decode, inflate};
use vyre_libs::hash::{adler32, crc32, fnv1a64};
use vyre_libs::parsing::core::delimiter::core_delimiter_match;

fn rebind_program(program: &Program, binding_base: u32) -> Program {
    let mut next_binding = binding_base;
    let mut buffers = program.buffers().to_vec();
    for buffer in &mut buffers {
        if buffer.access() != BufferAccess::Workgroup {
            buffer.binding = next_binding;
            next_binding += 1;
        }
        buffer.is_output = false;
    }
    Program::wrapped(buffers, program.workgroup_size(), program.entry().to_vec())
}

fn combine_programs(programs: &[Program]) -> Program {
    let mut buffers = Vec::new();
    let mut entry = Vec::new();
    let mut binding_base = 0_u32;

    for program in programs {
        let rebound = rebind_program(program, binding_base);
        binding_base += rebound
            .buffers()
            .iter()
            .filter(|buffer| buffer.access() != BufferAccess::Workgroup)
            .count() as u32;
        buffers.extend(rebound.buffers().iter().cloned());
        entry.extend(rebound.entry().iter().cloned());
    }

    Program::wrapped(buffers, [1, 1, 1], entry)
}

fn assert_unique_buffer_names(program: &Program) {
    let unique = program
        .buffers()
        .iter()
        .map(|buffer| buffer.name().to_string())
        .collect::<HashSet<_>>();
    assert_eq!(unique.len(), program.buffers().len());
}

#[test]
fn fused_decode_programs_keep_generic_buffers_disjoint() {
    let combined = combine_programs(&[
        base64_decode("input", "decoded", 16),
        hex_decode("input", "decoded", 16),
        inflate("input", "decoded", 16),
    ]);

    assert_unique_buffer_names(&combined);
    let errors = validate(&combined);
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn fused_hash_programs_keep_generic_buffers_disjoint() {
    let combined = combine_programs(&[
        adler32("input", "out", 32),
        crc32("input", "out", 32),
        fnv1a64("input", "out"),
    ]);

    assert_unique_buffer_names(&combined);
    let errors = validate(&combined);
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn duplicate_self_exclusive_parser_regions_fail_validation() {
    let parser_a = core_delimiter_match("tok_types_a", "tok_depths_a", 8, 12, 13);
    let parser_b = Program::wrapped(
        parser_a.buffers().to_vec(),
        parser_a.workgroup_size(),
        vec![Node::Region {
            generator: mark_self_exclusive_region("vyre-libs::parsing::core_delimiter_match")
                .into(),
            source_region: None,
            body: Arc::new(vec![Node::Return]),
        }],
    );
    let combined = combine_programs(&[parser_a, parser_b]);

    let errors = validate(&combined);
    assert!(errors
        .iter()
        .any(|error| { error.message.contains("marked non-composable with itself") }));
}
