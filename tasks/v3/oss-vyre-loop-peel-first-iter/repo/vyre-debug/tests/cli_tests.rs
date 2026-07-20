//! Test: cli tests.
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn cli_find_dangling_clean_program_exits_0() {
    let mut cmd = Command::cargo_bin("vyre-dbg").unwrap();
    cmd.arg("find-dangling")
        .arg("--prog")
        .arg("loop_carry_smoke")
        .arg("--num-tokens")
        .arg("8")
        .assert()
        .success() // exit code 0
        .stdout(predicate::str::contains("0 dangling"));
}

#[test]
fn cli_find_dangling_with_json_emits_array() {
    let mut cmd = Command::cargo_bin("vyre-dbg").unwrap();
    let assert = cmd
        .arg("find-dangling")
        .arg("--prog")
        .arg("loop_carry_smoke")
        .arg("--num-tokens")
        .arg("8")
        .arg("--json")
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("Valid JSON");
    assert!(parsed.is_array());
}

#[test]
fn cli_invalid_prog_name_exits_3() {
    let mut cmd = Command::cargo_bin("vyre-dbg").unwrap();
    cmd.arg("dump-descriptor")
        .arg("--prog")
        .arg("nope")
        .assert()
        .code(3)
        .stderr(predicate::str::contains("unknown program"));
}
