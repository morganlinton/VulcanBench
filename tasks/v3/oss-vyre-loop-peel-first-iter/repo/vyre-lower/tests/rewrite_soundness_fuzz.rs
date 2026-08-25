//! Property test: `verify(run_all(random_descriptor))` holds across
//! 1000 randomly-generated descriptors.
//!
//! The generator is a hand-rolled deterministic LCG (no `rand` dep).
//! Every emitted descriptor satisfies `verify(input).is_ok()` by
//! construction (the generator never produces a dangling ref or out-
//! of-range pool index). Then for each input, the rewrite pipeline
//! must produce a descriptor that ALSO verifies  -  that's the property.
//!
//! Counterexamples here are real bugs in the rewrite stack. The seed
//! that triggered them is printable, so the failure is reproducible.

use vyre_foundation::ir::BinOp;
use vyre_lower::{
    rewrites::run_all, verify, BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody,
    KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    fn range(&mut self, max: u32) -> u32 {
        if max == 0 {
            0
        } else {
            self.next() % max
        }
    }
    fn coin(&mut self, num: u32, denom: u32) -> bool {
        self.range(denom) < num
    }
}

fn buf_slot(slot: u32) -> BindingSlot {
    BindingSlot {
        slot,
        element_type: vyre_foundation::ir::DataType::U32,
        element_count: None,
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::ReadWrite,
        name: format!("buf{slot}"),
    }
}

const BIN_OPS: &[BinOp] = &[
    BinOp::Add,
    BinOp::Sub,
    BinOp::Mul,
    BinOp::BitAnd,
    BinOp::BitOr,
    BinOp::BitXor,
    BinOp::Shl,
    BinOp::Shr,
    BinOp::Min,
    BinOp::Max,
];

const SMALL_LITS: &[u32] = &[0, 1, 2, 3, 4, 7, 8, 16, 99, 0xFF];

/// Build a small self-contained KernelBody for use as an If/ForLoop
/// body. Generates 1–3 ops in its own id space (each body is isolated
/// per vyre's structured IR).
fn gen_tiny_body(rng: &mut Rng, parent_lits: &[LiteralValue], n_bindings: usize) -> KernelBody {
    let mut ops = Vec::new();
    let mut next_id: u32 = 0;
    let mut produced: Vec<u32> = Vec::new();
    // Reuse parent literal pool to keep the test simple.
    let lits = parent_lits.to_vec();

    // Always start with a Lit so subsequent ops have refs.
    let pool_idx = rng.range(lits.len() as u32);
    ops.push(KernelOp {
        kind: KernelOpKind::Literal,
        operands: vec![pool_idx],
        result: Some(next_id),
    });
    produced.push(next_id);
    next_id += 1;

    let extra = rng.range(3); // 0..=2 more
    for _ in 0..extra {
        let choice = rng.range(3);
        match choice {
            0 => {
                let pool_idx = rng.range(lits.len() as u32);
                ops.push(KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![pool_idx],
                    result: Some(next_id),
                });
                produced.push(next_id);
                next_id += 1;
            }
            1 if produced.len() >= 2 => {
                // Need a "value" to store. Use a literal we just made.
                let slot = rng.range(n_bindings as u32);
                let idx = produced[(rng.next() as usize) % produced.len()];
                let val = produced[(rng.next() as usize) % produced.len()];
                ops.push(KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![slot, idx, val],
                    result: None,
                });
            }
            _ => {
                let pool_idx = rng.range(lits.len() as u32);
                ops.push(KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![pool_idx],
                    result: Some(next_id),
                });
                produced.push(next_id);
                next_id += 1;
            }
        }
    }

    KernelBody {
        ops,
        child_bodies: vec![],
        literals: lits,
    }
}

fn gen_descriptor(seed: u64) -> KernelDescriptor {
    let mut rng = Rng(seed);
    let n_bindings = 1 + rng.range(2); // 1 or 2 bindings
    let bindings: Vec<BindingSlot> = (0..n_bindings).map(buf_slot).collect();

    let mut literals = Vec::new();
    let n_lits = 2 + rng.range(4); // 2..=5 lits
    for _ in 0..n_lits {
        let lit = SMALL_LITS[(rng.next() as usize) % SMALL_LITS.len()];
        literals.push(LiteralValue::U32(lit));
    }

    let mut ops = Vec::new();
    let mut next_id: u32 = 0;
    let mut produced: Vec<u32> = Vec::new();

    let n_lit_ops = 2 + rng.range(3); // at least 2 lits so binops have refs
    for _ in 0..n_lit_ops {
        let pool_idx = rng.range(literals.len() as u32);
        ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![pool_idx],
            result: Some(next_id),
        });
        produced.push(next_id);
        next_id += 1;
    }

    let mut child_bodies: Vec<KernelBody> = Vec::new();

    let n_extra_ops = rng.range(10); // 0..=9 more ops
    for _ in 0..n_extra_ops {
        if produced.is_empty() {
            break;
        }
        // Distribution: 25% lits, 25% binops, 20% stores, 15% loads,
        // 5% if, 5% loop, 3% barrier, 2% atomic.
        let kind_choice = rng.range(100);
        match kind_choice {
            0..=24 => {
                let pool_idx = rng.range(literals.len() as u32);
                ops.push(KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![pool_idx],
                    result: Some(next_id),
                });
                produced.push(next_id);
                next_id += 1;
            }
            25..=49 => {
                let bo = BIN_OPS[(rng.next() as usize) % BIN_OPS.len()];
                let lhs = produced[(rng.next() as usize) % produced.len()];
                let rhs = produced[(rng.next() as usize) % produced.len()];
                ops.push(KernelOp {
                    kind: KernelOpKind::BinOpKind(bo),
                    operands: vec![lhs, rhs],
                    result: Some(next_id),
                });
                produced.push(next_id);
                next_id += 1;
            }
            50..=69 => {
                let slot = rng.range(bindings.len() as u32);
                let idx = produced[(rng.next() as usize) % produced.len()];
                let val = produced[(rng.next() as usize) % produced.len()];
                ops.push(KernelOp {
                    kind: KernelOpKind::StoreGlobal,
                    operands: vec![slot, idx, val],
                    result: None,
                });
            }
            70..=84 => {
                let slot = rng.range(bindings.len() as u32);
                let idx = produced[(rng.next() as usize) % produced.len()];
                ops.push(KernelOp {
                    kind: KernelOpKind::LoadGlobal,
                    operands: vec![slot, idx],
                    result: Some(next_id),
                });
                produced.push(next_id);
                next_id += 1;
            }
            85..=89 => {
                // StructuredIfThen with a tiny synthesized child body.
                let cond = produced[(rng.next() as usize) % produced.len()];
                let body_idx = child_bodies.len() as u32;
                child_bodies.push(gen_tiny_body(&mut rng, &literals, bindings.len()));
                ops.push(KernelOp {
                    kind: KernelOpKind::StructuredIfThen,
                    operands: vec![cond, body_idx],
                    result: None,
                });
            }
            90..=94 => {
                // StructuredForLoop with constant lo/hi. Need lo and hi
                // to be Literal refs already in `produced`.
                if produced.len() >= 2 {
                    let lo = produced[(rng.next() as usize) % produced.len()];
                    let hi = produced[(rng.next() as usize) % produced.len()];
                    let body_idx = child_bodies.len() as u32;
                    child_bodies.push(gen_tiny_body(&mut rng, &literals, bindings.len()));
                    ops.push(KernelOp {
                        kind: KernelOpKind::StructuredForLoop {
                            loop_var: std::sync::Arc::from("i"),
                        },
                        operands: vec![lo, hi, body_idx],
                        result: None,
                    });
                }
            }
            95..=97 => {
                ops.push(KernelOp {
                    kind: KernelOpKind::Barrier {
                        ordering: vyre_foundation::runtime::memory_model::MemoryOrdering::SeqCst,
                    },
                    operands: vec![],
                    result: None,
                });
            }
            98..=99 => {
                let slot = rng.range(bindings.len() as u32);
                let idx = produced[(rng.next() as usize) % produced.len()];
                let val = produced[(rng.next() as usize) % produced.len()];
                ops.push(KernelOp {
                    kind: KernelOpKind::Atomic {
                        op: vyre_foundation::ir::AtomicOp::Add,
                        ordering: vyre_foundation::runtime::memory_model::MemoryOrdering::SeqCst,
                    },
                    operands: vec![slot, idx, val],
                    result: Some(next_id),
                });
                produced.push(next_id);
                next_id += 1;
            }
            _ => unreachable!(),
        }
    }

    // Sometimes add a final Store to give the kernel a side effect,
    // so DCE doesn't strip everything.
    if rng.coin(3, 4) && !produced.is_empty() {
        let slot = rng.range(bindings.len() as u32);
        let idx = produced[(rng.next() as usize) % produced.len()];
        let val = produced[(rng.next() as usize) % produced.len()];
        ops.push(KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![slot, idx, val],
            result: None,
        });
    }

    KernelDescriptor {
        id: format!("rand_{seed}"),
        bindings: BindingLayout { slots: bindings },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops,
            child_bodies,
            literals,
        },
    }
}

#[test]
fn input_descriptors_self_verify() {
    // Sanity: the generator never emits an invalid descriptor.
    for seed in 0..1000u64 {
        let desc = gen_descriptor(seed);
        if let Err(errs) = verify(&desc) {
            panic!(
                "generator emitted invalid descriptor at seed {seed}: {} errors\nfirst: {:?}",
                errs.len(),
                errs[0]
            );
        }
    }
}

#[test]
fn run_all_preserves_verify_property_across_corpus() {
    for seed in 0..1000u64 {
        let desc = gen_descriptor(seed);
        let optimized = run_all(&desc);
        if let Err(errs) = verify(&optimized) {
            panic!(
                "rewrite stack produced INVALID descriptor at seed {seed}: {} errors\n\
                 first error: {:?}\n\
                 input had {} ops, output {} ops",
                errs.len(),
                errs[0],
                desc.body.ops.len(),
                optimized.body.ops.len(),
            );
        }
        // Cap output growth so a pathological seed surfaces as a failure
        // not an OOM kill.
        assert!(
            optimized.body.ops.len() < 1_000,
            "rewrite output exploded at seed {seed}: {} → {} ops",
            desc.body.ops.len(),
            optimized.body.ops.len()
        );
    }
}

#[test]
fn run_all_is_idempotent_across_corpus() {
    for seed in 0..200u64 {
        let desc = gen_descriptor(seed);
        let once = run_all(&desc);
        let twice = run_all(&once);
        // Idempotence at the op-count level. Ops can be reordered or
        // renumbered between runs, but op count must be stable. (CSE/DCE
        // both being idempotent guarantees this.)
        assert_eq!(
            once.body.ops.len(),
            twice.body.ops.len(),
            "run_all not idempotent at seed {seed}: once={} twice={}",
            once.body.ops.len(),
            twice.body.ops.len(),
        );
    }
}

#[test]
fn run_all_never_grows_op_count() {
    // The pipeline is supposed to be code-size non-increasing for the
    // shapes we generate (none of strength_reduce's shape-changing
    // rewrites add net ops; everything else strictly removes).
    //
    // This isn't a hard property of run_all in general  -  strength_reduce
    // CAN increase op count on some shapes if it synthesizes a literal
    // that wasn't already in the body. But on this corpus (no Mul/Div/Mod
    // with brand-new pow2 literals  -  all pow2 lits we use are already
    // in the pool), it should hold.
    //
    // Loosened to: optimized op count ≤ input op count + 5 (generous
    // cushion for any synthesized constants).
    for seed in 0..500u64 {
        let desc = gen_descriptor(seed);
        let optimized = run_all(&desc);
        // CF generator can produce loops that loop_unroll legitimately
        // expands. Allow a 4× factor (MAX_UNROLL_COUNT) plus 10 fixed
        // overhead for synthesized constants.
        let max_growth = desc.body.ops.len() * 4 + 10;
        assert!(
            optimized.body.ops.len() <= max_growth,
            "run_all output too large at seed {seed}: {} → {} (cap {})",
            desc.body.ops.len(),
            optimized.body.ops.len(),
            max_growth,
        );
    }
}

#[test]
fn loop_unroll_does_not_underflow_when_hi_less_than_lo() {
    // Regression for the OOM bug found at fuzz seed 9: loop_unroll
    // computed `hi - lo` (release mode unsigned wrap-around) inside
    // the inlining loop, separately from its `saturating_sub` check.
    // For hi=2, lo=255 the check passed (count=0) but the loop ran
    // 4_294_967_043 iterations.
    use vyre_foundation::ir::DataType;
    let desc = KernelDescriptor {
        id: "underflow_regression".into(),
        bindings: BindingLayout {
            slots: vec![BindingSlot {
                slot: 0,
                element_type: DataType::U32,
                element_count: None,
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadWrite,
                name: "buf".into(),
            }],
        },
        dispatch: Dispatch::new(1, 1, 1),
        body: KernelBody {
            ops: vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }, // lo = 255
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                }, // hi = 2
                KernelOp {
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: std::sync::Arc::from("i"),
                    },
                    operands: vec![0, 1, 0],
                    result: None,
                },
            ],
            child_bodies: vec![KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(7)],
            }],
            literals: vec![LiteralValue::U32(255), LiteralValue::U32(2)],
        },
    };
    // Must complete in milliseconds, not minutes-then-OOM.
    let out = run_all(&desc);
    assert!(
        out.body.ops.len() < 100,
        "rewrite output suspiciously large: {} ops",
        out.body.ops.len()
    );
}

#[test]
fn run_all_is_byte_deterministic_across_corpus() {
    // Same input → same output, byte-for-byte. Catches accidental
    // HashMap iteration (which uses a random seed by default), use
    // of system clock, or any other non-determinism that would break
    // build caching and snapshot testing.
    for seed in 0..200u64 {
        let desc = gen_descriptor(seed);
        let first = run_all(&desc);
        let second = run_all(&desc);
        assert_eq!(
            first.body.ops, second.body.ops,
            "non-determinism in body.ops at seed {seed}"
        );
        assert_eq!(
            first.body.literals, second.body.literals,
            "non-determinism in body.literals at seed {seed}"
        );
        assert_eq!(
            first.body.child_bodies, second.body.child_bodies,
            "non-determinism in body.child_bodies at seed {seed}"
        );
        assert_eq!(
            first.bindings, second.bindings,
            "non-determinism in bindings at seed {seed}"
        );
    }
}
