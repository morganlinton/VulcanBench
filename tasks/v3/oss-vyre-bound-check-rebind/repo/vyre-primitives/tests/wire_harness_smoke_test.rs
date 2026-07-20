//! Integration test that exercises the `wire_harness_smoke` example as
//! a real subprocess - the same way an agent harness would invoke it.
//!
//! Locks the user-visible CLI contract (stdin/stdout shape, exit code,
//! determinism) so the harness can build against a frozen interface.

use std::io::Write;
use std::process::{Command, Stdio};

fn example_path() -> std::path::PathBuf {
    // `cargo test --example` puts the binary under `target/<profile>/examples`.
    let mut path = std::env::current_exe().expect("current_exe");
    // current_exe is .../target/<profile>/deps/<testname>-<hash>; pop twice + descend examples.
    path.pop();
    path.pop();
    path.push("examples");
    path.push("wire_harness_smoke");
    path
}

fn run_harness(stdin_input: &str) -> (String, String, Option<i32>) {
    let path = example_path();
    if !path.exists() {
        panic!(
            "wire_harness_smoke not built. Run `cargo build --example wire_harness_smoke -p vyre-primitives` first. Looked at: {}",
            path.display()
        );
    }
    let mut child = Command::new(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn harness");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin_input.as_bytes())
        .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().expect("wait");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.code(),
    )
}

#[test]
fn pack_u32_round_trip_via_subprocess() {
    let (stdout, stderr, code) = run_harness("pack-u32 1,2,3\n");
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    assert_eq!(code, Some(0));
    assert_eq!(stdout.trim(), "010000000200000003000000");
}

#[test]
fn unpack_u32_decodes_to_original_values() {
    let (stdout, stderr, code) = run_harness("unpack-u32 010000000200000003000000 3\n");
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    assert_eq!(code, Some(0));
    assert_eq!(stdout.trim(), "1,2,3");
}

#[test]
fn pack_f32_uses_le_byte_order() {
    let (stdout, stderr, code) = run_harness("pack-f32 1.0,-0.0\n");
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    assert_eq!(code, Some(0));
    // 1.0_f32 = 0x3f800000 (LE: 00 00 80 3f); -0.0_f32 = 0x80000000 (LE: 00 00 00 80).
    assert_eq!(stdout.trim(), "0000803f00000080");
}

#[test]
fn unknown_command_writes_err_and_nonzero_exit() {
    let (stdout, stderr, code) = run_harness("rotate 7\n");
    assert_eq!(code, Some(1));
    assert!(stderr.contains("unknown command"), "stderr: {stderr}");
    assert_eq!(stdout.trim(), "ERR");
}

#[test]
fn deterministic_across_repeated_runs() {
    let input = "pack-u32 7,11,13\npack-f32 3.14,2.718\npack-u32 0\n";
    let (a_out, _, a_code) = run_harness(input);
    let (b_out, _, b_code) = run_harness(input);
    assert_eq!(a_code, Some(0));
    assert_eq!(b_code, Some(0));
    assert_eq!(a_out, b_out, "harness output must be deterministic");
}
