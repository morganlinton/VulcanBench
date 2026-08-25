//! Shared helpers for the rustc differential tests: our frontend's accept/reject
//! verdict, and real rustc's. Used by both the curated corpus and the fuzzer.

#![allow(dead_code)]

use std::sync::atomic::{AtomicU32, Ordering};

use vyre_libs::parsing::rust::lex::lexer::core::lex;
use vyre_libs::parsing::rust::parse::parse;
use vyre_libs::parsing::rust::sema::{
    check_conflicts, check_escape, check_mutability, resolve, typeck,
};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Our verdict: does the implemented frontend accept this program?
/// (lex + parse + resolve + typeck + mutability + escape + conflicts all succeed)
pub(crate) fn ours_accepts(src: &str) -> bool {
    let bytes = src.as_bytes();
    let tokens = match lex(bytes) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let module = match parse(bytes, &tokens) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let resolution = match resolve(&module, bytes) {
        Ok(r) => r,
        Err(_) => return false,
    };
    typeck(&module, bytes, &resolution).is_ok()
        && check_mutability(&module, &resolution).is_ok()
        && check_escape(&module, &resolution).is_ok()
        && check_conflicts(&module, &resolution).is_ok()
}

/// rustc verdict: does `rustc --crate-type lib` accept this program?
/// Lints capped to allow so only hard errors reject; current_dir is the temp
/// dir so the workspace rust-toolchain.toml does not force an uninstalled
/// toolchain.
pub(crate) fn rustc_accepts(src: &str) -> bool {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir();
    let stem = format!("vyre_diff_{}_{}", std::process::id(), n);
    let rs = dir.join(format!("{stem}.rs"));
    let meta = dir.join(format!("{stem}.rmeta"));
    std::fs::write(&rs, src)
        .expect("Fix: must be able to write a temp source file for the rustc differential");
    let output = std::process::Command::new("rustc")
        .current_dir(&dir)
        .args([
            "--edition",
            "2021",
            "--crate-type",
            "lib",
            "--cap-lints",
            "allow",
            "--emit",
            "metadata",
        ])
        .arg("-o")
        .arg(&meta)
        .arg(&rs)
        .output()
        .expect("Fix: rustc must be available on PATH; it is the project toolchain");
    let _ = std::fs::remove_file(&rs);
    let _ = std::fs::remove_file(&meta);
    output.status.success()
}
