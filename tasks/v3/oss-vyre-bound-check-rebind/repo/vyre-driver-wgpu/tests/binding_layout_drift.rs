//! Failure-oriented tests for Naga lowering binding layout drift.
//!
//! The `bind_group_for` policy in `lowering/mod.rs` is the sole
//! authority on group placement. These tests verify that the emitted
//! WGSL, the pipeline layout builder, and the reflection scanner all
//! agree on `(group, binding)` assignments and that Naga array strides
//! match the IR element layout (FINDING-52 class).

use naga::{ArraySize, ResourceBinding, TypeInner};
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver::DispatchConfig;
use vyre_driver_wgpu::emit::{self, WgpuProgram};
use vyre_driver_wgpu::runtime::device::EnabledFeatures;

fn lower_wgsl(program: &Program) -> String {
    emit::lower(program).expect("Fix: test program must lower to WGSL")
}

fn lower_bir(program: &Program) -> WgpuProgram {
    WgpuProgram::from_program(
        program,
        &DispatchConfig::default(),
        &EnabledFeatures::default(),
    )
    .expect("Fix: test program must lower to WgpuProgram backend IR")
}

fn bindings_from_bir(bir: &WgpuProgram) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = bir
        .bindings
        .iter()
        .map(|assignment| (assignment.group, assignment.binding))
        .collect();
    out.sort_unstable();
    out
}

/// Naga `(group, binding, stride, runtime_sized)` for every emitted resource.
fn naga_resource_layouts(module: &naga::Module) -> Vec<(u32, u32, u32, bool)> {
    let mut out = Vec::new();
    for global in module.global_variables.iter().map(|(_, global)| global) {
        let Some(ResourceBinding { group, binding }) = global.binding else {
            continue;
        };
        let TypeInner::Array { stride, size, .. } = &module.types[global.ty].inner else {
            continue;
        };
        let runtime_sized = matches!(size, ArraySize::Dynamic);
        out.push((group, binding, *stride, runtime_sized));
    }
    out.sort_unstable();
    out
}

fn naga_stride_for_binding(module: &naga::Module, group: u32, binding: u32) -> Option<u32> {
    naga_resource_layouts(module)
        .into_iter()
        .find(|(g, b, _, _)| *g == group && *b == binding)
        .map(|(_, _, stride, _)| stride)
}

fn assert_naga_strides_match_ir(bir: &WgpuProgram) {
    for assignment in &bir.bindings {
        let expected_stride = assignment.element.min_bytes().max(4) as u32;
        let actual = naga_stride_for_binding(&bir.module, assignment.group, assignment.binding)
            .unwrap_or_else(|| {
                panic!(
                    "Fix: Naga module must declare array stride for @group({}) @binding({}) buffer `{}`. bindings={:?}",
                    assignment.group,
                    assignment.binding,
                    assignment.name,
                    bir.bindings,
                )
            });
        assert_eq!(
            actual, expected_stride,
            "Fix: array stride for `{}` ({:?}) must match IR element layout (FINDING-52). \
             @group({}) @binding({}) naga_stride={actual} expected={expected_stride}",
            assignment.name, assignment.element, assignment.group, assignment.binding,
        );
    }
}

/// Cross-check IR slot policy, WgpuProgram assignments, WGSL reflection,
/// and Naga array strides in one oracle.
fn assert_emit_binding_cross_check(program: &Program, expected: &[(u32, u32)]) {
    let bir = lower_bir(program);
    let wgsl = lower_wgsl(program);
    let parsed = parse_bindings(&wgsl);
    let from_bir = bindings_from_bir(&bir);
    let expected_vec: Vec<(u32, u32)> = expected.to_vec();

    assert_eq!(
        from_bir, expected_vec,
        "Fix: WgpuProgram binding assignments must match IR slot policy. \
         from_bir={from_bir:?} expected={expected_vec:?}\nWGSL:\n{wgsl}"
    );
    assert_eq!(
        parsed, expected_vec,
        "Fix: emitted WGSL @group/@binding pairs must match IR slot policy (reflection drift). \
         parsed={parsed:?} expected={expected_vec:?}\nWGSL:\n{wgsl}"
    );
    assert_naga_strides_match_ir(&bir);
}

fn module_uses_runtime_array_length(module: &naga::Module) -> bool {
    module
        .entry_points
        .first()
        .map(|entry| {
            entry
                .function
                .expressions
                .iter()
                .any(|(_, expr)| matches!(expr, naga::Expression::ArrayLength(_)))
        })
        .unwrap_or(false)
}

/// Parse every `@group(N) @binding(M)` pair out of WGSL.
fn parse_bindings(wgsl: &str) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut rest = wgsl;
    while let Some(g_pos) = rest.find("@group(") {
        rest = &rest[g_pos + "@group(".len()..];
        let Some(g_end) = rest.find(')') else { break };
        let Ok(group) = rest[..g_end].trim().parse::<u32>() else {
            rest = &rest[g_end + 1..];
            continue;
        };
        rest = &rest[g_end + 1..];
        let Some(b_pos) = rest.find("@binding(") else {
            continue;
        };
        rest = &rest[b_pos + "@binding(".len()..];
        let Some(b_end) = rest.find(')') else { break };
        if let Ok(binding) = rest[..b_end].trim().parse::<u32>() {
            if !out.contains(&(group, binding)) {
                out.push((group, binding));
            }
        }
        rest = &rest[b_end + 1..];
    }
    out.sort_unstable();
    out
}

#[test]
fn uniform_buffer_uses_group_one_in_emitted_wgsl() {
    let program = Program::wrapped(
        vec![
            BufferDecl::uniform("params", 0, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let wgsl = lower_wgsl(&program);
    let bindings = parse_bindings(&wgsl);
    assert!(
        bindings.contains(&(1, 0)),
        "Fix: Uniform buffer must be emitted in group 1 to match bind_group_for policy. bindings={bindings:?}\nWGSL:\n{wgsl}"
    );
}

#[test]
fn storage_buffer_uses_group_zero_in_emitted_wgsl() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("in", 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::output("out", 1, DataType::U32),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("in", Expr::u32(0)),
        )],
    );
    let wgsl = lower_wgsl(&program);
    let bindings = parse_bindings(&wgsl);
    assert!(
        bindings.contains(&(0, 0)),
        "Fix: ReadOnly storage buffer must be emitted in group 0. bindings={bindings:?}\nWGSL:\n{wgsl}"
    );
    assert!(
        bindings.contains(&(0, 1)),
        "Fix: Output/Global buffer must be emitted in group 0. bindings={bindings:?}\nWGSL:\n{wgsl}"
    );
}

#[test]
fn naga_emit_and_pipeline_layout_agree_on_max_group() {
    // A program with both uniform (group 1) and storage (group 0) buffers
    // must produce WGSL referencing group 1 so the pipeline layout builder
    // creates bind_group_layouts for [0, 1]. If the WGSL only references
    // group 0 but a buffer is assigned to group 1, pipeline creation fails.
    let program = Program::wrapped(
        vec![
            BufferDecl::uniform("params", 0, DataType::U32).with_count(1),
            BufferDecl::storage("in", 1, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::output("out", 2, DataType::U32),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let wgsl = lower_wgsl(&program);
    let bindings = parse_bindings(&wgsl);
    let max_group = bindings.iter().map(|(g, _)| *g).max().unwrap_or(0);
    assert_eq!(
        max_group, 1,
        "Fix: with Uniform in group 1, emitted WGSL must reference group 1 so pipeline layout creates 2 bind groups. bindings={bindings:?}\nWGSL:\n{wgsl}"
    );
    assert_eq!(
        bindings,
        vec![(0, 1), (0, 2), (1, 0)],
        "Fix: binding layout must be deterministic and complete. bindings={bindings:?}\nWGSL:\n{wgsl}"
    );
}

#[test]
fn workgroup_memory_is_not_reflected_as_binding() {
    // Workgroup (shared) memory must not produce a @group/@binding declaration
    // in WGSL, and the pipeline layout must not reserve a slot for it.
    let program = Program::wrapped(
        vec![
            BufferDecl::workgroup("scratch", 64, DataType::U32),
            BufferDecl::output("out", 0, DataType::U32),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let wgsl = lower_wgsl(&program);
    let bindings = parse_bindings(&wgsl);
    for &(g, b) in &bindings {
        assert!(
            g == 0 && b == 0,
            "Fix: workgroup memory must not appear as a resource binding. Found ({g},{b}) in WGSL:\n{wgsl}"
        );
    }
}

#[test]
fn three_storage_buffers_binding_indices_follow_decl_slots() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("in", 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage("tmp", 1, BufferAccess::ReadWrite, DataType::U32),
            BufferDecl::output("out", 2, DataType::U32),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("in", Expr::u32(0)),
        )],
    );
    assert_emit_binding_cross_check(&program, &[(0, 0), (0, 1), (0, 2)]);
}

#[test]
fn dual_uniform_slots_emit_group_one_bindings() {
    let program = Program::wrapped(
        vec![
            BufferDecl::uniform("params_a", 0, DataType::U32).with_count(1),
            BufferDecl::uniform("params_b", 1, DataType::U32).with_count(4),
            BufferDecl::storage("out", 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    assert_emit_binding_cross_check(&program, &[(0, 2), (1, 0), (1, 1)]);
}

#[test]
fn unbounded_storage_binding_slot_matches_decl_and_runtime_array_size() {
    // No `with_count`: storage stays runtime-sized in Naga regardless of
    // the IR binding slot. @binding(3) must still mirror BufferDecl::binding.
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("input", 3, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("input", Expr::u32(0)),
        )],
    );
    assert_emit_binding_cross_check(&program, &[(0, 0), (0, 3)]);

    let bir = lower_bir(&program);
    let layouts = naga_resource_layouts(&bir.module);
    assert!(
        layouts
            .iter()
            .any(|(g, b, _, runtime)| *g == 0 && *b == 3 && *runtime),
        "Fix: unbounded storage @binding(3) must lower to a runtime-sized Naga array. layouts={layouts:?}"
    );
}

#[test]
fn counted_storage_binding_slot_matches_decl_with_dynamic_naga_array() {
    // Static `with_count` on storage does not pin Naga array size; buf_len
    // must still emit ArrayLength so dispatch-time range matches the bound
    // buffer. @binding(5) is independent of the static count hint.
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("input", 5, BufferAccess::ReadOnly, DataType::U32).with_count(64),
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::buf_len("input"))],
    );
    assert_emit_binding_cross_check(&program, &[(0, 0), (0, 5)]);

    let bir = lower_bir(&program);
    let input_layout = naga_resource_layouts(&bir.module)
        .into_iter()
        .find(|(g, b, _, _)| *g == 0 && *b == 5)
        .expect("Fix: counted storage input must appear in Naga resource layouts");
    assert!(
        input_layout.3,
        "Fix: counted storage must remain runtime-sized in Naga so per-dispatch binding range is authoritative. layout={input_layout:?}"
    );
    assert!(
        module_uses_runtime_array_length(&bir.module),
        "Fix: buf_len on counted storage must emit naga::Expression::ArrayLength for WGSL arrayLength()."
    );
}

#[test]
fn f32_storage_naga_array_stride_matches_element_layout() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("values", 2, BufferAccess::ReadOnly, DataType::F32),
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(0))],
    );
    assert_emit_binding_cross_check(&program, &[(0, 0), (0, 2)]);

    let bir = lower_bir(&program);
    assert_eq!(
        naga_stride_for_binding(&bir.module, 0, 2),
        Some(4),
        "Fix: F32 storage arrays must use a 4-byte Naga stride."
    );
}

#[test]
fn u8_storage_naga_array_stride_is_four_bytes_despite_one_byte_elements() {
    // U8 elements are byte-addressed in IR but stored as array<u32> in WGSL.
    // Stride must still be max(min_bytes(), 4) = 4 (FINDING-52 guard).
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("bytes", 4, BufferAccess::ReadOnly, DataType::U8),
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(0))],
    );
    assert_emit_binding_cross_check(&program, &[(0, 0), (0, 4)]);

    let bir = lower_bir(&program);
    assert_eq!(
        naga_stride_for_binding(&bir.module, 0, 4),
        Some(4),
        "Fix: U8-backed storage arrays must use a 4-byte Naga stride (not 1)."
    );
}

#[test]
fn non_contiguous_read_only_storage_binding_slots_are_reflected() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("a", 1, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage("b", 7, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage("c", 9, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(
                Expr::load("a", Expr::u32(0)),
                Expr::add(Expr::load("b", Expr::u32(0)), Expr::load("c", Expr::u32(0))),
            ),
        )],
    );
    assert_emit_binding_cross_check(&program, &[(0, 0), (0, 1), (0, 7), (0, 9)]);
}
