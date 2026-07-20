use super::*;

#[test]
fn parity_backend_input_plan_accepts_logical_fixture_order_after_output_buffer() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
            BufferDecl::storage("input", 1, BufferAccess::ReadOnly, DataType::U32).with_count(2),
        ],
        [1, 1, 1],
        Vec::<Node>::new(),
    );
    let plan =
        backend_dispatch_plan(&program).expect("Fix: static logical input planning must succeed.");
    let case = vec![vec![1, 0, 0, 0, 2, 0, 0, 0]];
    let mut backend_inputs = Vec::new();

    backend_dispatch_inputs_with_plan_into(&case, &plan, &mut backend_inputs)
        .expect("Fix: logical fixture order must route input bytes even when output buffers precede inputs.");

    assert_eq!(
        backend_inputs,
        vec![case[0].as_slice()],
        "Fix: parity matrix must use logical fixture order, not raw Program::buffers indices."
    );
}

#[test]
fn parity_backend_input_plan_accepts_fixture_backed_runtime_sized_input() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        Vec::<Node>::new(),
    );
    let plan = backend_dispatch_plan(&program)
        .expect("Fix: runtime-sized read-only buffers must be fixture-backed.");
    let case = vec![vec![0xAA; 12]];
    let mut backend_inputs = Vec::new();

    backend_dispatch_inputs_with_plan_into(&case, &plan, &mut backend_inputs)
        .expect("Fix: concrete fixture bytes must satisfy runtime-sized parity inputs.");

    assert_eq!(
        backend_inputs,
        vec![case[0].as_slice()],
        "Fix: dynamic fixture bytes must pass through unchanged."
    );
}

#[test]
fn parity_backend_input_plan_rejects_omitted_runtime_sized_read_write_input() {
    let program = Program::wrapped(
        vec![BufferDecl::storage(
            "scratch",
            0,
            BufferAccess::ReadWrite,
            DataType::U32,
        )],
        [1, 1, 1],
        Vec::<Node>::new(),
    );
    let plan = backend_dispatch_plan(&program)
        .expect("Fix: dynamic read-write input may be fixture-backed.");
    let mut backend_inputs = Vec::new();

    let error = backend_dispatch_inputs_with_plan_into(&[], &plan, &mut backend_inputs)
        .expect_err("Fix: omitted dynamic read-write inputs must not be silently zeroed.");

    assert!(
        error.contains("runtime-sized read-write buffer"),
        "Fix: error must preserve dynamic read-write fixture guidance, got: {error}"
    );
}

#[test]
fn parity_reference_runner_uses_planned_zeroed_read_write_inputs() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("scratch", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "scratch",
            Expr::u32(0),
            Expr::load("input", Expr::u32(0)),
        )],
    );
    let plan = backend_dispatch_plan(&program)
        .expect("Fix: static read-write zero-fill planning must succeed.");
    let runner = BackendRunner {
        id: "reference",
        kind: BackendKind::ReferenceBackend,
    };
    let config = DispatchConfig::default();
    let inputs = vec![1u32.to_le_bytes().to_vec()];
    let mut values = Vec::new();
    let mut borrowed_inputs = Vec::new();

    let outputs = runner
        .dispatch_with_plan(
            &program,
            &inputs,
            &mut values,
            Some(&plan),
            &mut borrowed_inputs,
            &config,
        )
        .expect("Fix: reference parity runner must receive planned zeroed read-write inputs.");

    assert_eq!(
        outputs,
        vec![1u32.to_le_bytes().to_vec()],
        "Fix: reference and backend parity paths must use the same planned input buffer expansion."
    );
}

// Asserts `runners.len() >= 2`, which means at least one dispatch-capable
// backend in addition to vyre-reference must be linked. If the crate is built
// without the `gpu` feature, this test must fail loudly instead of compiling
// out the parity gate.

