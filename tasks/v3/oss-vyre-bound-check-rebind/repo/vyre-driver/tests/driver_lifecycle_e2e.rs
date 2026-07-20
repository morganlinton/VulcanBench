//! Driver lifecycle E2E and PersistentEngine integration stress.
//!
//! Exercises the full driver path (build Program → binding plan → launch plan
//! → pipeline compile → dispatch → readback) through the cpu-ref backend, and
//! a 16×16 producer/consumer PersistentEngine stress run at 100k items.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use vyre_driver::persistent::{PersistentEngine, PersistentWorkItem};
use vyre_driver::pipeline::compile;
use vyre_driver::validation::LaunchGeometryLimits;
use vyre_driver::{BindingPlan, DispatchConfig, LaunchPlan, VyreBackend};
use vyre_driver_reference::CpuRefBackend;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_reference::value::Value;

const PERSISTENT_PRODUCERS: usize = 16;
const PERSISTENT_CONSUMERS: usize = 16;
const PERSISTENT_TOTAL_ITEMS: u32 = 100_000;
const PERSISTENT_RING_SIZE: u32 = 1024;

/// Minimal multi-op Program: `out = (a + b) * (a - b)` on u32 inputs.
fn multi_op_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(1),
            BufferDecl::read("b", 1, DataType::U32).with_count(1),
            BufferDecl::output("out", 2, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind(
                "sum",
                Expr::add(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
            ),
            Node::let_bind(
                "diff",
                Expr::sub(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
            ),
            Node::let_bind("product", Expr::mul(Expr::var("sum"), Expr::var("diff"))),
            Node::store("out", Expr::u32(0), Expr::var("product")),
        ],
    )
}

fn launch_limits(backend: &dyn VyreBackend) -> LaunchGeometryLimits {
    LaunchGeometryLimits {
        backend: backend.id(),
        max_threads_per_block: backend.max_compute_invocations_per_workgroup(),
        max_block_dim: backend.max_workgroup_size(),
        max_grid_dim: [
            backend.max_compute_workgroups_per_dimension(),
            backend.max_compute_workgroups_per_dimension(),
            backend.max_compute_workgroups_per_dimension(),
        ],
    }
}

fn reference_bytes(program: &Program, inputs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let values: Vec<Value> = inputs.iter().cloned().map(Value::from).collect();
    vyre_reference::reference_eval(program, &values)
        .expect("Fix: lifecycle oracle program must execute on reference interpreter")
        .into_iter()
        .map(|value| value.to_bytes())
        .collect()
}

#[test]
fn driver_lifecycle_e2e_parse_plan_emit_dispatch_readback() {
    let program = multi_op_program();
    let a_bytes = 13_u32.to_le_bytes().to_vec();
    let b_bytes = 7_u32.to_le_bytes().to_vec();
    let inputs = vec![a_bytes.clone(), b_bytes.clone()];

    let binding_plan = BindingPlan::build(&program).expect("Fix: lifecycle program must bind");
    vyre_driver::validate_program_for_backend(&CpuRefBackend, &program, &DispatchConfig::default())
        .expect("Fix: lifecycle program must pass backend validation");

    let launch = LaunchPlan::from_bindings(
        &program,
        &binding_plan.bindings,
        &DispatchConfig::default(),
        launch_limits(&CpuRefBackend),
    )
    .expect("Fix: lifecycle launch plan must prepare geometry");

    assert_eq!(launch.element_count, 1);
    assert!(!launch.param_words.is_empty());

    let backend: Arc<dyn VyreBackend> = Arc::new(CpuRefBackend);
    let pipeline = compile(Arc::clone(&backend), &program, &DispatchConfig::default())
        .expect("Fix: lifecycle pipeline compile must succeed");

    let outputs = pipeline
        .dispatch(&inputs, &DispatchConfig::default())
        .expect("Fix: lifecycle pipeline dispatch must succeed");

    let expected = reference_bytes(&program, &inputs);
    assert_eq!(
        outputs, expected,
        "Fix: driver lifecycle readback must match reference_eval bytes"
    );

    let sum = 13_u32.wrapping_add(7);
    let diff = 13_u32.wrapping_sub(7);
    assert_eq!(outputs[0], (sum.wrapping_mul(diff)).to_le_bytes().to_vec());
}

struct PersistentWaitGroup {
    producers_done: AtomicBool,
}

impl PersistentWaitGroup {
    fn new() -> Self {
        Self {
            producers_done: AtomicBool::new(false),
        }
    }

    fn mark_producers_done(&self) {
        self.producers_done.store(true, Ordering::Release);
    }
}

fn producer_correlation_range(producer_id: usize) -> (u32, u32) {
    let total = usize::try_from(PERSISTENT_TOTAL_ITEMS)
        .expect("Fix: persistent stress item count must fit usize");
    let base = total / PERSISTENT_PRODUCERS;
    let extra = total % PERSISTENT_PRODUCERS;
    let start = producer_id * base + producer_id.min(extra);
    let count = base + usize::from(producer_id < extra);
    let end = start + count;
    (
        u32::try_from(start).expect("Fix: persistent stress start index must fit u32"),
        u32::try_from(end).expect("Fix: persistent stress end index must fit u32"),
    )
}

fn persistent_work_item(correlation: u32) -> PersistentWorkItem {
    PersistentWorkItem {
        input_offset: correlation.wrapping_mul(64),
        input_len: 64,
        rule_set_id: correlation % 8,
        correlation,
    }
}

#[test]
fn persistent_engine_stress_16_prod_16_cons_100k_items() {
    let engine = Arc::new(PersistentEngine::new(PERSISTENT_RING_SIZE));
    assert_eq!(engine.ring_size(), PERSISTENT_RING_SIZE);

    let wait = Arc::new(PersistentWaitGroup::new());
    let shared_consumed = Arc::new(Mutex::new(Vec::new()));
    let enqueued = Arc::new(AtomicU32::new(0));
    let total_items = usize::try_from(PERSISTENT_TOTAL_ITEMS)
        .expect("Fix: persistent stress item count must fit usize");

    let mut producer_handles = Vec::with_capacity(PERSISTENT_PRODUCERS);
    for producer_id in 0..PERSISTENT_PRODUCERS {
        let engine = Arc::clone(&engine);
        let enqueued = Arc::clone(&enqueued);
        producer_handles.push(thread::spawn(move || {
            let (start, end) = producer_correlation_range(producer_id);
            for correlation in start..end {
                let item = persistent_work_item(correlation);
                loop {
                    if engine.enqueue(item).is_ok() {
                        enqueued.fetch_add(1, Ordering::Relaxed);
                        break;
                    }
                    thread::yield_now();
                }
            }
        }));
    }

    let mut consumer_handles = Vec::with_capacity(PERSISTENT_CONSUMERS);
    for _ in 0..PERSISTENT_CONSUMERS {
        let engine = Arc::clone(&engine);
        let wait = Arc::clone(&wait);
        let shared_consumed = Arc::clone(&shared_consumed);
        consumer_handles.push(thread::spawn(move || {
            let mut local = Vec::with_capacity(total_items / PERSISTENT_CONSUMERS);
            loop {
                if let Some(item) = engine.claim() {
                    local.push(item.correlation);
                    continue;
                }
                if wait.producers_done.load(Ordering::Acquire) && engine.in_flight() == 0 {
                    break;
                }
                thread::yield_now();
            }
            shared_consumed.lock().unwrap().extend(local);
        }));
    }

    for handle in producer_handles {
        handle
            .join()
            .expect("Fix: persistent stress producer must not panic");
    }
    assert_eq!(
        enqueued.load(Ordering::Relaxed),
        PERSISTENT_TOTAL_ITEMS,
        "Fix: persistent stress producers must enqueue the full workload"
    );
    wait.mark_producers_done();

    for handle in consumer_handles {
        handle
            .join()
            .expect("Fix: persistent stress consumer must not panic");
    }

    let mut consumed = Arc::try_unwrap(shared_consumed)
        .expect("Fix: persistent stress consumers must join before merge")
        .into_inner()
        .expect("Fix: persistent stress consumed mutex must not be poisoned");
    while let Some(item) = engine.claim() {
        consumed.push(item.correlation);
    }
    let raw_count = consumed.len();
    consumed.sort_unstable();
    consumed.dedup();
    assert_eq!(
        raw_count, total_items,
        "Fix: persistent stress claim count must match enqueued workload before dedup"
    );
    assert_eq!(
        consumed.len(),
        total_items,
        "Fix: persistent stress must consume every enqueued item exactly once"
    );
    assert!(
        consumed
            .iter()
            .enumerate()
            .all(|(index, correlation)| *correlation == index as u32),
        "Fix: persistent stress must observe correlation ids 0..={} without gaps or duplicates",
        PERSISTENT_TOTAL_ITEMS - 1
    );
    assert_eq!(engine.head_counter(), u64::from(PERSISTENT_TOTAL_ITEMS));
    assert_eq!(engine.tail_counter(), u64::from(PERSISTENT_TOTAL_ITEMS));
    assert_eq!(engine.in_flight(), 0);
}
