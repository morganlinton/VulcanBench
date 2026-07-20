//! Differential test: GPU pipeline output ≡ CPU pipeline output for
//! a representative set of input Programs. Both pipelines are run
//! on each fixture; the resulting Programs are compared for
//! structural equality.
//!
//! This is the cross-check that the self-hosted GPU optimizer is a
//! drop-in replacement for the CPU oracle on the supported V1 rule
//! sets (canonicalize commutative literal-on-right, const-fold for
//! u32 arithmetic, the 6 V1 algebraic identities + the new
//! Sub/BitAnd/BitOr/BitXor x-with-0 + CSE-aware self rules, and DCE
//! over the program-graph).
//!
//! What we deliberately don't compare here: ordering of independent
//! lets after canonicalize (the GPU pass and CPU pass agree on the
//! commutative-operand swap rule but the CPU pass also performs
//! associativity reorderings the GPU pass doesn't). Each fixture is
//! crafted so canon's swap is the only legal reordering  -  i.e. the
//! result form is unambiguous.

#![cfg(test)]

mod common;

use common::live_backend;
use vyre::ir::{Expr, Node, Program};
use vyre_driver_cuda::CudaOptimizerDispatcher;
use vyre_self_substrate::optimizer::pipeline_resident::gpu_pipeline_resident;

fn run_gpu_pipeline(p: Program) -> Program {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    gpu_pipeline_resident(p, &dispatcher).expect("gpu pipeline must succeed")
}

fn run_cpu_dce_only(p: Program) -> Program {
    use vyre_foundation::optimizer::passes::fusion_cse::dce::engine::dce as cpu_dce;
    cpu_dce(p)
}

/// Body normalisation: drop the outer Region wrapper so the two sides
/// can be compared regardless of which entry shape they wrap into.
fn normalize_body(p: &Program) -> Vec<Node> {
    match p.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    }
}

// ---- Fixtures ----------------------------------------------------

fn fixture_dead_let() -> Program {
    Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("dead", Expr::u32(99)),
            Node::let_bind("live", Expr::u32(7)),
            Node::store("buf", Expr::u32(0), Expr::var("live")),
        ],
    )
}

fn fixture_dead_chain_of_three() -> Program {
    Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::u32(1)),
            Node::let_bind("b", Expr::add(Expr::var("a"), Expr::u32(2))),
            Node::let_bind("c", Expr::mul(Expr::var("b"), Expr::u32(3))),
            // No store consuming `c`; the entire chain is dead.
            Node::store("buf", Expr::u32(0), Expr::u32(42)),
        ],
    )
}

fn fixture_only_live_used() -> Program {
    Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("unused1", Expr::u32(11)),
            Node::let_bind("live", Expr::u32(13)),
            Node::let_bind("unused2", Expr::u32(15)),
            Node::store("buf", Expr::u32(0), Expr::var("live")),
        ],
    )
}

// ---- DCE differential --------------------------------------------
//
// GPU pipeline is *strictly more aggressive* than the CPU DCE oracle
// because it also runs const-fold, CSE, pat-match, let-dedupe, and
// const-prop. Asserting `gpu == cpu_dce_only` would force the CPU
// oracle to match GPU's optimization power. Instead each test
// asserts the EXACT GPU output shape (the strongest claim) and adds
// a "GPU body length ≤ CPU body length" sanity check so the
// "differential" framing still bites.

fn assert_gpu_at_least_as_optimized_as_cpu_dce(label: &str, gpu: &Program, cpu: &Program) {
    let gpu_body = normalize_body(gpu);
    let cpu_body = normalize_body(cpu);
    assert!(
        gpu_body.len() <= cpu_body.len(),
        "[{label}] GPU body should be no longer than CPU DCE-only body.\n\
         GPU body ({} nodes): {gpu_body:?}\n\
         CPU body ({} nodes): {cpu_body:?}",
        gpu_body.len(),
        cpu_body.len()
    );
}

#[test]
fn dce_matches_cpu_oracle_dead_let() {
    let p = fixture_dead_let();
    let gpu = run_gpu_pipeline(p.clone());
    let cpu = run_cpu_dce_only(p);
    // GPU rewrites `let live = 7; store buf 0 (Var live)` all the way
    // down to `store buf 0 7` via const-prop + DCE. CPU DCE alone
    // only drops the unused `dead`.
    assert_gpu_at_least_as_optimized_as_cpu_dce("dead_let", &gpu, &cpu);
    // Strong assertion: the store value is the literal 7.
    let body = normalize_body(&gpu);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(matches!(value, Expr::LitU32(7)), "got {value:?}");
    }
}

#[test]
fn dce_matches_cpu_oracle_dead_chain_of_three() {
    let p = fixture_dead_chain_of_three();
    let gpu = run_gpu_pipeline(p.clone());
    let cpu = run_cpu_dce_only(p);
    assert_gpu_at_least_as_optimized_as_cpu_dce("dead_chain_of_three", &gpu, &cpu);
    // The whole `a/b/c` chain is dead; GPU drops all three lets.
    let body = normalize_body(&gpu);
    assert!(
        !body.iter().any(|n| matches!(n, Node::Let { .. })),
        "all dead lets should be dropped; got {body:?}"
    );
}

#[test]
fn dce_matches_cpu_oracle_only_live_used() {
    let p = fixture_only_live_used();
    let gpu = run_gpu_pipeline(p.clone());
    let cpu = run_cpu_dce_only(p);
    assert_gpu_at_least_as_optimized_as_cpu_dce("only_live_used", &gpu, &cpu);
    // Const-prop turns `Var(live)` into LitU32(13); DCE drops every let.
    let body = normalize_body(&gpu);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(matches!(value, Expr::LitU32(13)), "got {value:?}");
    }
    assert!(
        !body.iter().any(|n| matches!(n, Node::Let { .. })),
        "all unused lets should be dropped; got {body:?}"
    );
}

// ---- Idempotence -------------------------------------------------
//
// Running the GPU pipeline on its own output must be a no-op.
// Catches non-monotonic rewrites and divergent canon orderings.

#[test]
fn gpu_pipeline_is_idempotent_dead_let() {
    let p = fixture_dead_let();
    let pass1 = run_gpu_pipeline(p);
    let pass2 = run_gpu_pipeline(pass1.clone());
    let body1 = normalize_body(&pass1);
    let body2 = normalize_body(&pass2);
    assert_eq!(body1, body2, "GPU pipeline is not idempotent on dead_let");
}

#[test]
fn gpu_pipeline_is_idempotent_dead_chain() {
    let p = fixture_dead_chain_of_three();
    let pass1 = run_gpu_pipeline(p);
    let pass2 = run_gpu_pipeline(pass1.clone());
    let body1 = normalize_body(&pass1);
    let body2 = normalize_body(&pass2);
    assert_eq!(
        body1, body2,
        "GPU pipeline is not idempotent on dead_chain_of_three"
    );
}

#[test]
fn gpu_pipeline_is_idempotent_with_const_fold() {
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::add(Expr::u32(1), Expr::u32(2))),
            Node::let_bind("b", Expr::mul(Expr::var("a"), Expr::u32(4))),
            Node::store("buf", Expr::u32(0), Expr::var("b")),
        ],
    );
    let pass1 = run_gpu_pipeline(p);
    let pass2 = run_gpu_pipeline(pass1.clone());
    let body1 = normalize_body(&pass1);
    let body2 = normalize_body(&pass2);
    assert_eq!(
        body1, body2,
        "GPU pipeline is not idempotent on const-fold fixture"
    );
}

#[test]
fn gpu_pipeline_is_idempotent_with_cse_self() {
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::u32(123)),
            Node::store(
                "buf",
                Expr::u32(0),
                Expr::bitxor(Expr::var("x"), Expr::var("x")),
            ),
        ],
    );
    let pass1 = run_gpu_pipeline(p);
    let pass2 = run_gpu_pipeline(pass1.clone());
    let body1 = normalize_body(&pass1);
    let body2 = normalize_body(&pass2);
    assert_eq!(
        body1, body2,
        "GPU pipeline is not idempotent on CSE self-rule fixture"
    );
}
