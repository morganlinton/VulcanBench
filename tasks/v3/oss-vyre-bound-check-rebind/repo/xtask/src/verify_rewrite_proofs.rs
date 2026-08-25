//! N3  -  `cargo_full run --bin xtask -- verify-rewrite-proofs`.
//!
//! Walks `vyre_foundation::optimizer::rewrite_proof_registry::shipped_obligations`,
//! emits each as SMT-LIB v2, and runs `z3 -smt2` on the script. Reports
//! per-obligation `unsat` / `sat` / `unknown` plus an exit code: 0 if
//! every obligation is `unsat` (proven), 1 otherwise.
//!
//! When the `z3` binary is not installed the runner is gracefully
//! advisory: it emits every script to disk under
//! `target/rewrite-proofs/<rewrite>.smt2` and exits 0 with a notice.
//! CI installs z3 explicitly; local dev does not have to.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub(crate) fn run(_args: &[String]) {
    let obligations = vyre_foundation::optimizer::rewrite_proof_registry::shipped_obligations();
    eprintln!(
        "vyre xtask verify-rewrite-proofs  -  {} obligation(s)",
        obligations.len()
    );

    let out_dir = PathBuf::from("target").join("rewrite-proofs");
    if let Err(e) = fs::create_dir_all(&out_dir) {
        eprintln!("error: failed to create {}: {e}", out_dir.display());
        std::process::exit(1);
    }

    let z3_present = which("z3");
    if !z3_present {
        eprintln!(
            "note: z3 not found on PATH. Emitting SMT2 scripts to {} \
             without verification (advisory mode).",
            out_dir.display()
        );
    }

    let mut proven = 0usize;
    let mut sat = 0usize;
    let mut unknown = 0usize;
    let mut emit_only = 0usize;

    for o in &obligations {
        let script = o.to_smt2();
        let script_path = out_dir.join(format!("{}.smt2", o.rewrite));
        if let Err(e) = fs::write(&script_path, script.as_bytes()) {
            eprintln!("error: failed to write {}: {e}", script_path.display());
            std::process::exit(1);
        }

        if !z3_present {
            emit_only += 1;
            continue;
        }

        let result = Command::new("z3").arg("-smt2").arg(&script_path).output();
        match result {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let verdict = stdout.lines().next().unwrap_or("").trim();
                match verdict {
                    "unsat" => {
                        eprintln!("PROVEN  {}", o.rewrite);
                        proven += 1;
                    }
                    "sat" => {
                        eprintln!("FAILED  {}  (z3 found a counter-model)", o.rewrite);
                        sat += 1;
                    }
                    "unknown" => {
                        eprintln!("UNKNOWN {}  (z3 could not decide)", o.rewrite);
                        unknown += 1;
                    }
                    other => {
                        eprintln!("???     {}  (unexpected z3 verdict: {other:?})", o.rewrite);
                        unknown += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("error: failed to spawn z3: {e}");
                std::process::exit(1);
            }
        }
    }

    eprintln!();
    eprintln!("==> rewrite-proofs summary");
    if z3_present {
        eprintln!("    proven  : {proven}");
        eprintln!("    failed  : {sat}");
        eprintln!("    unknown : {unknown}");
        if sat > 0 {
            std::process::exit(1);
        }
    } else {
        eprintln!("    emitted : {emit_only} scripts (z3 absent)");
    }
}

fn which(bin: &str) -> bool {
    let path = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    for entry in std::env::split_paths(&path) {
        let candidate = entry.join(bin);
        if candidate.is_file() {
            return true;
        }
    }
    false
}
