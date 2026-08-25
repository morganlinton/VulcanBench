//! Validation: load rustc's real `-Znll-facts` dump for a Rust function and
//! check that our NLL loan-liveness verdict ([`RustcNllFacts::accepts`]) agrees
//! with rustc's own borrow-check verdict. This proves the producer runs the
//! borrow check on real MIR-level facts, not just our nano-subset's AST.
//!
//! Requires a nightly rustc (the only toolchain that emits `-Znll-facts`); the
//! producer fundamentally depends on it, so absence is a loud failure, not a
//! silent skip.

#![forbid(unsafe_code)]

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use vyre_libs::borrowck::rustc_facts::load_facts;

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn read_relation(fn_dir: &Path, name: &str) -> String {
    std::fs::read_to_string(fn_dir.join(format!("{name}.facts"))).unwrap_or_default()
}

/// Dump rustc nll-facts for `src`, returning `(rustc_accepts, our_accepts)`.
/// `our_accepts` is true iff every dumped function borrow-checks under our NLL
/// loan-liveness rule.
fn verdicts(src: &str) -> (bool, bool) {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("vyre_nll_{}_{}", std::process::id(), n));
    let facts_dir = base.join("facts");
    std::fs::create_dir_all(&facts_dir).expect("create temp facts dir");
    let src_path = base.join("lib.rs");
    std::fs::write(&src_path, src).expect("write temp source");

    let output = std::process::Command::new("rustc")
        .arg("+nightly")
        .args([
            "--edition",
            "2021",
            "--crate-type",
            "lib",
            "--cap-lints",
            "allow",
        ])
        .arg("-Znll-facts")
        .arg(format!("-Znll-facts-dir={}", facts_dir.display()))
        .args(["--emit", "metadata"])
        .arg("-o")
        .arg(base.join("out.rmeta"))
        .arg(&src_path)
        .output()
        .expect("Fix: rustc must be on PATH; the nll-facts producer test needs it");
    let rustc_accepts = output.status.success();

    let function_dirs: Vec<_> = std::fs::read_dir(&facts_dir)
        .expect("read facts dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    assert!(
        !function_dirs.is_empty(),
        "rustc emitted no nll-facts directories; is the active rustc a nightly with -Znll-facts? \
         stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut our_accepts = true;
    for fn_dir in &function_dirs {
        let facts = load_facts(|name| read_relation(fn_dir, name));
        if !facts.accepts() {
            our_accepts = false;
        }
    }

    let _ = std::fs::remove_dir_all(&base);
    (rustc_accepts, our_accepts)
}

/// Type-correct functions whose accept/reject is purely a borrow-check decision,
/// so rustc's verdict is exactly what our NLL rule must reproduce.
const ACCEPT: &[&str] = &[
    "pub fn f() { let x: i32 = 0; let a: &i32 = &x; let b: &i32 = &x; let _c: i32 = *a + *b; }",
    "pub fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let _p: i32 = *a; let b: &mut i32 = &mut x; let _q: i32 = *b; }",
    "pub fn f() { let mut x: i32 = 0; if true { let a: &mut i32 = &mut x; let _p: i32 = *a; } else { let b: &mut i32 = &mut x; let _q: i32 = *b; } }",
    "pub fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let _b: &mut i32 = &mut x; let _c: i32 = *_b; }",
    // NLL across a loop: each iteration's &mut is dead before the next.
    "pub fn f() { let mut x: i32 = 0; let mut i: i32 = 0; while i < 10 { let a: &mut i32 = &mut x; *a = i; i = i + 1; } }",
    // Reborrow of a &mut parameter.
    "pub fn f(p: &mut i32) { let a: &mut i32 = &mut *p; *a = 1; }",
    // Aliasing shared borrows through a call is allowed.
    "pub fn g(a: &i32, b: &i32) -> i32 { return *a + *b; } pub fn f() { let x: i32 = 0; let r: &i32 = &x; let _z: i32 = g(r, r); }",
];

const REJECT: &[&str] = &[
    "pub fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let b: &mut i32 = &mut x; let _c: i32 = *a + *b; }",
    "pub fn f() { let mut x: i32 = 0; let a: &i32 = &x; let b: &mut i32 = &mut x; let _c: i32 = *a + *b; }",
    "pub fn f() { let mut x: i32 = 0; let a: &mut i32 = &mut x; let b: &mut i32 = &mut x; if true { let _p: i32 = *a; } else { let _q: i32 = *b; } }",
    // Assign to a place while it is shared-borrowed and the borrow is later used.
    "pub fn f() { let mut x: i32 = 0; let a: &i32 = &x; x = 1; let _b: i32 = *a; }",
    // Escape: return a reference to a local (E0515) - a region-outlives error.
    "pub fn f() -> &'static i32 { let x: i32 = 0; &x }",
    // Escape: a parameter ref does not outlive 'static (E0521).
    "pub fn f(p: &i32) -> &'static i32 { p }",
];

/// Deterministically generate a type- and name-correct borrow program from a
/// seed: some `i32` vars (each maybe `mut`), then a sequence of `&`/`&mut`
/// borrows and deref-uses. Every program compiles, so rustc's accept/reject is
/// purely a borrow-check decision - exactly what our Naive rule must reproduce.
fn generate_borrow_program(seed: u64) -> String {
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as u32
    };
    let nvars = 2 + (next() % 3) as usize; // 2..=4
    let var_mut: Vec<bool> = (0..nvars).map(|_| next() % 2 == 0).collect();
    let mut s = String::from("pub fn f() {");
    for (i, &m) in var_mut.iter().enumerate() {
        s.push_str(&format!(
            " let {}v{}: i32 = {};",
            if m { "mut " } else { "" },
            i,
            i
        ));
    }
    let nops = (next() % 8) as usize;
    let mut borrows = 0usize;
    let mut uses = 0u32;
    for _ in 0..nops {
        if next() % 2 == 0 {
            let vk = (next() as usize) % nvars;
            // Only `&mut` a `mut` var, so the program never trips E0596 (a
            // different error class); the verdict stays about conflicting borrows.
            let m = next() % 2 == 0 && var_mut[vk];
            let kw = if m { "mut " } else { "" };
            s.push_str(&format!(" let r{borrows}: &{kw}i32 = &{kw}v{vk};"));
            borrows += 1;
        } else if borrows > 0 {
            let bk = (next() as usize) % borrows;
            s.push_str(&format!(" let u{uses}: i32 = *r{bk};"));
            uses += 1;
        }
    }
    s.push_str(" }");
    s
}

/// Like [`generate_borrow_program`] but the borrows are taken at the top level
/// and then deref-used inside the arms of an `if`/`else`, fuzzing cross-branch
/// liveness: borrows used in separate reachable arms are live across the branch
/// point, so two `&mut` of one place conflict. Exercises the CFG-sensitive part
/// of the ruleset against rustc.
fn generate_branch_program(seed: u64) -> String {
    let mut state = seed ^ 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as u32
    };
    let nvars = 2 + (next() % 3) as usize;
    let var_mut: Vec<bool> = (0..nvars).map(|_| next() % 2 == 0).collect();
    let mut s = String::from("pub fn f() {");
    for (i, &m) in var_mut.iter().enumerate() {
        s.push_str(&format!(
            " let {}v{}: i32 = {};",
            if m { "mut " } else { "" },
            i,
            i
        ));
    }
    let nborrows = 1 + (next() % 4) as usize;
    let mut borrows = 0usize;
    for _ in 0..nborrows {
        let vk = (next() as usize) % nvars;
        let m = next() % 2 == 0 && var_mut[vk];
        let kw = if m { "mut " } else { "" };
        s.push_str(&format!(" let r{borrows}: &{kw}i32 = &{kw}v{vk};"));
        borrows += 1;
    }
    let mut uses = 0u32;
    let arm = |s: &mut String, n: u32, next: &mut dyn FnMut() -> u32, uses: &mut u32| {
        for _ in 0..n {
            if borrows > 0 {
                let bk = (next() as usize) % borrows;
                s.push_str(&format!(" let u{}: i32 = *r{bk};", *uses));
                *uses += 1;
            }
        }
    };
    s.push_str(" if true {");
    arm(&mut s, next() % 3, &mut next, &mut uses);
    s.push_str(" } else {");
    arm(&mut s, next() % 3, &mut next, &mut uses);
    s.push_str(" }; }");
    s
}

fn run_fuzz(cases: u64, gen: impl Fn(u64) -> String) -> Vec<String> {
    let mut mismatches = Vec::new();
    for seed in 0..cases {
        let src = gen(seed);
        let (rustc_accepts, our_accepts) = verdicts(&src);
        if rustc_accepts != our_accepts {
            mismatches.push(format!(
                "seed {seed}: rustc={rustc_accepts} ours={our_accepts}: {src}"
            ));
        }
    }
    mismatches
}

#[test]
fn our_nll_verdict_matches_rustc_on_generated_borrow_programs() {
    let mismatches = run_fuzz(300, generate_borrow_program);
    assert!(
        mismatches.is_empty(),
        "Naive verdict diverged from rustc on {} straight-line programs:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn our_nll_verdict_matches_rustc_on_generated_branch_programs() {
    let mismatches = run_fuzz(200, generate_branch_program);
    assert!(
        mismatches.is_empty(),
        "Naive verdict diverged from rustc on {} branch programs:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

/// A realistic, self-contained module of clean functions (rustc compiles it):
/// loops, slices, vecs, iterators, matches, reborrows, and a two-phase borrow.
/// Our rule must accept every function - any rejection is a false positive on
/// real-world borrow patterns.
const REALISTIC: &str = r#"
pub fn sum(v: &[i32]) -> i32 { let mut s = 0; for &x in v { s += x; } s }
pub fn count_pos(v: &[i32]) -> usize { let mut c = 0; for &x in v { if x > 0 { c += 1; } } c }
pub fn fill(v: &mut Vec<i32>, n: i32) { let mut i = 0; while i < n { v.push(i); i += 1; } }
pub fn double(v: &mut [i32]) { for x in v.iter_mut() { *x *= 2; } }
pub fn first_last(v: &[i32]) -> (i32, i32) { (v[0], v[v.len() - 1]) }
pub fn swap_ends(v: &mut Vec<i32>) { let n = v.len(); if n >= 2 { v.swap(0, n - 1); } }
pub fn push_len(v: &mut Vec<usize>) { v.push(v.len()); }
pub fn reborrow(p: &mut i32) { let r = &mut *p; *r += 1; }
pub fn nested(v: &[i32]) -> i32 { let mut s = 0; for &x in v { if x > 0 { let y = &x; s += *y; } } s }
pub fn opt_ref(o: &Option<i32>) -> i32 { if let Some(x) = o { *x } else { 0 } }
pub fn max_val(v: &[i32]) -> i32 { let mut m = i32::MIN; for &x in v { if x > m { m = x; } } m }
pub fn seq_borrow() { let mut x: i32 = 0; let a = &mut x; *a = 1; let b = &mut x; *b = 2; }
"#;

#[test]
fn our_nll_verdict_matches_rustc_on_realistic_module() {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("vyre_nll_real_{}_{}", std::process::id(), n));
    let facts_dir = base.join("facts");
    std::fs::create_dir_all(&facts_dir).expect("create temp facts dir");
    let src_path = base.join("lib.rs");
    std::fs::write(&src_path, REALISTIC).expect("write temp source");

    let output = std::process::Command::new("rustc")
        .arg("+nightly")
        .args([
            "--edition",
            "2021",
            "--crate-type",
            "lib",
            "--cap-lints",
            "allow",
        ])
        .arg("-Znll-facts")
        .arg(format!("-Znll-facts-dir={}", facts_dir.display()))
        .args(["--emit", "metadata"])
        .arg("-o")
        .arg(base.join("out.rmeta"))
        .arg(&src_path)
        .output()
        .expect("rustc must be on PATH");
    assert!(
        output.status.success(),
        "REALISTIC module must compile under rustc: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut rejected = Vec::new();
    let mut checked = 0usize;
    for entry in std::fs::read_dir(&facts_dir).expect("read facts dir") {
        let fn_dir = entry.expect("dir entry").path();
        if !fn_dir.is_dir() {
            continue;
        }
        checked += 1;
        let facts = load_facts(|name| read_relation(&fn_dir, name));
        if !facts.accepts() {
            let name = fn_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            rejected.push(name);
        }
    }
    let _ = std::fs::remove_dir_all(&base);

    assert!(
        checked >= 12,
        "expected facts for all functions, got {checked}"
    );
    assert!(
        rejected.is_empty(),
        "our NLL rule false-positived on real code rustc accepts: {rejected:?}"
    );
}

#[test]
fn our_nll_verdict_matches_rustc_on_accept_corpus() {
    for (i, src) in ACCEPT.iter().enumerate() {
        let (rustc_accepts, our_accepts) = verdicts(src);
        assert!(rustc_accepts, "ACCEPT[{i}] must compile under rustc: {src}");
        assert!(
            our_accepts,
            "ACCEPT[{i}]: our NLL rule rejected what rustc accepts: {src}"
        );
    }
}

#[test]
fn our_nll_verdict_matches_rustc_on_reject_corpus() {
    for (i, src) in REJECT.iter().enumerate() {
        let (rustc_accepts, our_accepts) = verdicts(src);
        assert!(
            !rustc_accepts,
            "REJECT[{i}] must be rejected by rustc: {src}"
        );
        assert!(
            !our_accepts,
            "REJECT[{i}]: our NLL rule accepted what rustc rejects: {src}"
        );
    }
}
