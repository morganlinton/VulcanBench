//! End-to-end execution oracle for the Rust nano-subset: lower the AST to a
//! Vyre `Program`, run it on the pure-Rust reference interpreter, and check the
//! result against two independent oracles:
//!   1. a direct AST tree-walk interpreter (independent of lowering), and
//!   2. real rustc: compile the module + a `main` that prints `entry(args)`,
//!      run it, and compare stdout.
//!
//! Programs are generated overflow-free and division-free so i32 semantics are
//! unambiguous across all three (no Rust debug-overflow panic, no div-by-zero).

#![forbid(unsafe_code)]

#[path = "rust_lower_exec_oracle/support.rs"]
mod support;

use support::{
    ast_interp, gen_for_program, gen_inputs, gen_program, gen_while_program, ir_exec,
    ir_exec_batched, rustc_run,
};

#[test]
fn lowered_while_matches_ast_and_rustc() {
    let mut checked = 0;
    for seed in 0..400u64 {
        let (src, nparams) = gen_while_program(seed);
        let inputs = gen_inputs(seed.wrapping_mul(11).wrapping_add(2), nparams);
        let ast = ast_interp(&src, &inputs);
        let ir = ir_exec(&src, &inputs);
        assert_eq!(
            ir, ast,
            "while: lowered IR diverged from AST interp:\n  {src}\n  inputs {inputs:?}"
        );
        if seed < 60 {
            if let Some(rustc) = rustc_run(&src, &inputs) {
                assert_eq!(
                    ir, rustc,
                    "while: lowered IR diverged from rustc:\n  {src}\n  inputs {inputs:?}"
                );
                checked += 1;
            }
        }
    }
    assert!(
        checked >= 30,
        "expected most while programs to compile+run under rustc, got {checked}"
    );
}

#[test]
fn lowered_for_range_matches_ast_and_rustc() {
    let mut checked = 0;
    for seed in 0..400u64 {
        let (src, nparams) = gen_for_program(seed);
        let inputs = gen_inputs(seed.wrapping_mul(17).wrapping_add(5), nparams);
        let ast = ast_interp(&src, &inputs);
        let ir = ir_exec(&src, &inputs);
        assert_eq!(
            ir, ast,
            "for-range: lowered IR diverged from AST interp:\n  {src}\n  inputs {inputs:?}"
        );
        if seed < 80 {
            if let Some(rustc) = rustc_run(&src, &inputs) {
                assert_eq!(
                    ir, rustc,
                    "for-range: lowered IR diverged from rustc:\n  {src}\n  inputs {inputs:?}"
                );
                checked += 1;
            }
        }
    }
    assert!(
        checked >= 50,
        "expected most for-range programs to compile+run under rustc, got {checked}"
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn lowered_ir_matches_ast_interpreter() {
    for seed in 0..2000u64 {
        let (src, nparams) = gen_program(seed);
        for input_seed in 0..3u64 {
            let inputs = gen_inputs(seed.wrapping_mul(7).wrapping_add(input_seed), nparams);
            let expected = ast_interp(&src, &inputs);
            let got = ir_exec(&src, &inputs);
            assert_eq!(
                got, expected,
                "lowered IR diverged from AST interpreter at seed {seed} inputs {inputs:?}:\n  {src}"
            );
        }
    }
}

#[test]
fn curated_programs_execute_correctly() {
    let cases: &[(&str, &[i32], i32)] = &[
        ("fn f(a: i32, b: i32) -> i32 { return a + b; }", &[3, 4], 7),
        ("fn f(a: i32, b: i32) -> i32 { return a - b; }", &[10, 3], 7),
        ("fn f(a: i32) -> i32 { let b: i32 = a * 2; return b + 1; }", &[5], 11),
        ("fn f(a: i32, b: i32) -> i32 { if a < b { return b; } else { return a; } }", &[3, 9], 9),
        ("fn f(a: i32, b: i32) -> i32 { if a < b { return b; } else { return a; } }", &[9, 3], 9),
        ("fn f(a: i32) -> i32 { if a == 0 { return 100; } else { return a; } }", &[0], 100),
        ("fn g(a: i32, b: i32) -> i32 { return a + b; } fn f(a: i32) -> i32 { return g(a, 10); }", &[5], 15),
        ("fn g(a: i32) -> i32 { let d: i32 = a * a; return d - 1; } fn f(a: i32, b: i32) -> i32 { return g(a) + b; }", &[4, 2], 17),
        ("fn f(a: i32) -> i32 { let r: &i32 = &a; return *r + 1; }", &[6], 7),
        ("fn f(a: i32) -> i32 { return *(&a) * 2; }", &[5], 10),
        ("fn d(p: &i32) -> i32 { return *p + 1; } fn f(a: i32) -> i32 { return d(&a); }", &[8], 9),
        ("fn f(a: i32) -> i32 { return a / 3; }", &[7], 2),
        ("fn f(a: i32) -> i32 { return a / 3; }", &[-7], -2), // truncates toward zero
        ("fn f(a: i32) -> i32 { return a % 3; }", &[7], 1),
        ("fn f(a: i32) -> i32 { return a % 3; }", &[-7], -1), // remainder sign follows dividend
        ("fn f(a: i32, b: i32) -> i32 { if a > b { return 1; } else { return 0; } }", &[5, 2], 1),
        ("fn f(a: i32, b: i32) -> i32 { if a <= b { return 1; } else { return 0; } }", &[2, 2], 1),
        ("fn f(a: i32, b: i32) -> i32 { if a >= b { return 1; } else { return 0; } }", &[1, 2], 0),
        ("fn f(a: i32, b: i32) -> i32 { if a != b { return 1; } else { return 0; } }", &[3, 3], 0),
        ("fn f(a: i32, b: i32) -> i32 { if a < b && b < 10 { return 1; } else { return 0; } }", &[3, 5], 1),
        ("fn f(a: i32, b: i32) -> i32 { if a < b && b < 10 { return 1; } else { return 0; } }", &[3, 50], 0),
        ("fn f(a: i32, b: i32) -> i32 { if a == 0 || b == 0 { return 1; } else { return 0; } }", &[0, 7], 1),
        ("fn f(a: i32, b: i32) -> i32 { if !(a < b) { return 1; } else { return 0; } }", &[5, 2], 1),
        ("fn f(a: i32) -> i32 { let mut x: i32 = a; x = x + 1; x = x * 2; return x; }", &[3], 8),
        ("fn f(n: i32) -> i32 { let mut i: i32 = 0; let mut acc: i32 = 0; while i < n { acc = acc + i; i = i + 1; } return acc; }", &[5], 10),
        ("fn f(n: i32) -> i32 { let mut i: i32 = 0; let mut acc: i32 = 0; while i < n { acc = acc + i; i = i + 1; } return acc; }", &[0], 0),
        // Negative bound: rustc runs the loop zero times. The old lowering cast
        // `n` to u32 (-> ~4.29e9 iterations); the clamped trip count must give 0.
        ("fn f(n: i32) -> i32 { let mut i: i32 = 0; let mut acc: i32 = 0; while i < n { acc = acc + i; i = i + 1; } return acc; }", &[-3], 0),
        // Non-zero (and negative) start: `i` must reconstruct as `i0 + lv`, not as
        // the raw u32 loop counter. 2+3+4 = 9.
        ("fn f() -> i32 { let mut i: i32 = 2; let mut acc: i32 = 0; while i < 5 { acc = acc + i; i = i + 1; } return acc; }", &[], 9),
        ("fn f() -> i32 { let mut i: i32 = -2; let mut acc: i32 = 0; while i < 1 { acc = acc + i; i = i + 1; } return acc; }", &[], -3), // (-2)+(-1)+0 = -3
        // Post-loop induction value. After a loop that ran, `i == n`; for a
        // zero-trip loop `i` is left unchanged (the old `i := n` was wrong).
        ("fn f(n: i32) -> i32 { let mut i: i32 = 0; while i < n { i = i + 1; } return i; }", &[4], 4),
        ("fn f() -> i32 { let mut i: i32 = 5; while i < 3 { i = i + 1; } return i; }", &[], 5),
        // Compound assignment value semantics: the differential only checks
        // accept/reject, so a broken `x += e -> x = e` desugar would ship green.
        // Hardcoded expected values pin the read-modify-write (a broken desugar
        // fools the ir-vs-ast cross-check but not the literal oracle below).
        ("fn f(a: i32) -> i32 { let mut x: i32 = a; x += 2; return x; }", &[3], 5),
        ("fn f(a: i32) -> i32 { let mut x: i32 = a; x -= 2; return x; }", &[10], 8),
        ("fn f(a: i32, b: i32) -> i32 { let mut x: i32 = a; x += b * 2; return x; }", &[10, 4], 18),
        // Unary minus value + precedence: `-a + b` must parse as `(-a) + b`, not
        // `-(a + b)`; `-(-a)` is identity. Accept-only differential cannot see this.
        ("fn f(a: i32, b: i32) -> i32 { return -a + b; }", &[5, 3], -2),
        ("fn f(a: i32) -> i32 { return -(-a); }", &[7], 7),
        ("fn f(a: i32, b: i32) -> i32 { return a - -b; }", &[5, 3], 8),
        ("fn f() -> i32 { let mut acc: i32 = 0; for i in 0..5 { acc += i; } return acc; }", &[], 10),
        ("fn f() -> i32 { let mut acc: i32 = 0; for i in -2..2 { acc += i; } return acc; }", &[], -2),
        ("fn f() -> i32 { let mut acc: i32 = 7; for i in 5..3 { acc += i; } return acc; }", &[], 7),
        ("fn f(n: i32) -> i32 { let mut acc: i32 = 0; for i in 0..n { acc += i; } return acc; }", &[4], 6),
        // Bounds are evaluated once before iteration; mutating `n` in the body
        // must not shorten the range after the first trip.
        ("fn f(n: i32) -> i32 { let mut m: i32 = n; let mut acc: i32 = 0; for i in 0..m { m = 0; acc += i; } return acc; }", &[4], 6),
    ];
    for (src, inputs, expected) in cases {
        assert_eq!(ir_exec(src, inputs), *expected, "{src} with {inputs:?}");
        assert_eq!(
            ast_interp(src, inputs),
            *expected,
            "AST interp: {src} with {inputs:?}"
        );
    }
}

#[test]
fn batched_lowering_maps_rust_entry_across_input_buffers() {
    let src = "\
fn f(a: i32, n: i32) -> i32 {
    let mut acc: i32 = a;
    for i in -3..n {
        if i < 0 {
            acc += i * 2;
        } else {
            acc += i + a;
        };
    }
    return acc;
}";
    let lanes = 257usize;
    let a: Vec<i32> = (0..lanes)
        .map(|lane| ((lane as i32 * 5) % 23) - 11)
        .collect();
    let n: Vec<i32> = (0..lanes)
        .map(|lane| ((lane as i32 * 7) % 17) - 4)
        .collect();
    let got = ir_exec_batched(src, &[a.clone(), n.clone()]);
    let expected: Vec<i32> = a
        .iter()
        .zip(n.iter())
        .map(|(&a, &n)| ast_interp(src, &[a, n]))
        .collect();
    assert_eq!(
        got, expected,
        "batched Rust lowering must map scalar entry semantics across every lane"
    );
}

#[test]
fn lowered_ir_matches_rustc_execution() {
    let mut checked = 0;
    for seed in 0..80u64 {
        let (src, nparams) = gen_program(seed);
        let inputs = gen_inputs(seed.wrapping_mul(13).wrapping_add(1), nparams);
        let Some(expected) = rustc_run(&src, &inputs) else {
            continue;
        };
        let got = ir_exec(&src, &inputs);
        assert_eq!(
            got, expected,
            "lowered IR diverged from rustc-run at seed {seed} inputs {inputs:?}:\n  {src}"
        );
        checked += 1;
    }
    assert!(
        checked >= 40,
        "expected most generated programs to compile+run under rustc, got {checked}"
    );
}
