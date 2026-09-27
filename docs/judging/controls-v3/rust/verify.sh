#!/usr/bin/env bash
# Every Rust control must produce identical behaviour; only maintainability differs.
#
# Run: bash docs/judging/controls-v3/rust/verify.sh
#
# For each control-*.rs this script writes a driver into a temp dir that includes
# the control as a module (#[path = "..."] mod ledger;), compiles it with
# `rustc --edition 2021 -D warnings`, runs it, and checks the behavioural vectors
# from MULTILANG.md. It also checks that control 2 is still exactly rustfmt's
# output for control 1 and that control 6 minus its comment lines is control 2.
# Exit 0 only when everything passes. Needs rustc; rustfmt only for the format check.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "$(rustc --version)"
failures=0

# The shared part of every driver. Each driver file starts with a per-control head
# (the #[path] module include, CONTROL, and extra_checks) written by the loop below.
IFS= read -r -d '' DRIVER_BODY <<'RUST' || true
use std::collections::HashMap;

const SAMPLE: &str = "# opening balances
2026-09-01,alpha,card,1000
2026-09-01,beta,bank,500

2026-09-02,alpha,cash,-1200
2026-09-02,gamma,card,10
";

fn is_error<E: std::error::Error>(_: &E) -> bool {
    true
}

fn record(account: &str, kind: &str, amount_cents: i64) -> ledger::Record {
    ledger::Record { account: account.to_string(), kind: kind.to_string(), amount_cents }
}

fn totals(pairs: &[(&str, i64)]) -> HashMap<String, i64> {
    pairs.iter().map(|(account, cents)| (account.to_string(), *cents)).collect()
}

fn expect_err<T: std::fmt::Debug>(
    failures: &mut Vec<String>,
    what: &str,
    result: Result<T, ledger::LedgerError>,
    message: &str,
) {
    match result {
        Err(error) if error.to_string() == message && is_error(&error) => {}
        Err(error) => failures.push(format!("{what}: wrong error {:?}", error.to_string())),
        Ok(value) => failures.push(format!("{what}: expected an error, got {value:?}")),
    }
}

fn main() {
    let mut failures: Vec<String> = Vec::new();

    let records = match ledger::parse_records(SAMPLE) {
        Ok(records) => records,
        Err(error) => {
            println!("FAIL parse_records(SAMPLE) returned {error}");
            std::process::exit(1);
        }
    };
    let parsed: Vec<(String, String, i64)> = records
        .iter()
        .map(|r| (r.account.clone(), r.kind.clone(), r.amount_cents))
        .collect();
    let wanted: Vec<(String, String, i64)> = [
        ("alpha", "card", 1000),
        ("beta", "bank", 500),
        ("alpha", "cash", -1200),
        ("gamma", "card", 10),
    ]
    .iter()
    .map(|(a, k, c)| (a.to_string(), k.to_string(), *c))
    .collect();
    if parsed != wanted {
        failures.push(format!("parse_records(SAMPLE) = {parsed:?}"));
    }

    let expected = totals(&[("alpha", -259), ("beta", 475), ("gamma", -20)]);
    match ledger::balances(&records) {
        Ok(got) if got == expected => {}
        Ok(got) => failures.push(format!("balances(SAMPLE) = {got:?}")),
        Err(error) => failures.push(format!("balances(SAMPLE) failed: {error}")),
    }

    let lines = ledger::render(&expected);
    if lines != ["alpha  (2.59)", "beta  4.75", "gamma  (0.20)"] {
        failures.push(format!("render(totals) = {lines:?}"));
    }

    // Control 9 keeps totals as hidden module state: an empty map renders the last balances.
    let fallback = ledger::render(&HashMap::new());
    if CONTROL == 9 && fallback != lines {
        failures.push(format!("control 9 render(empty) did not fall back: {fallback:?}"));
    }
    if CONTROL != 9 && !fallback.is_empty() {
        failures.push(format!("render(empty) = {fallback:?}"));
    }

    for (kind, amount, fee) in [("card", 1000, 59), ("card", 10, 30), ("bank", 500, 25), ("cash", -1200, 0)] {
        match ledger::fee_cents(kind, amount) {
            Ok(got) if got == fee => {}
            other => failures.push(format!("fee_cents({kind}, {amount}) = {other:?}")),
        }
    }
    expect_err(&mut failures, "fee_cents(crypto)", ledger::fee_cents("crypto", 5), "unknown kind: crypto");

    expect_err(
        &mut failures,
        "parse_records(three fields)",
        ledger::parse_records("2026-09-01,alpha,card"),
        "line 1: expected 4 fields, got 3",
    );
    expect_err(
        &mut failures,
        "balances(crypto)",
        ledger::balances(&[record("x", "crypto", 5)]),
        "unknown kind: crypto",
    );

    let ordered = ledger::render(&totals(&[("_suspense", 100), ("alpha", 1), ("zeta", 2)]));
    let suspense_last = ordered.last().map(String::as_str) == Some("_suspense  1.00");
    let suspense_first = ordered.first().map(String::as_str) == Some("_suspense  1.00");
    if CONTROL == 7 && !suspense_last {
        failures.push(format!("control 7 quirk: render = {ordered:?}"));
    }
    if CONTROL != 7 && !suspense_first {
        failures.push(format!("plain sort expected _suspense first: {ordered:?}"));
    }

    extra_checks(&records, &mut failures);

    if failures.is_empty() {
        println!("ok");
    } else {
        for failure in &failures {
            println!("FAIL {failure}");
        }
        std::process::exit(1);
    }
}
RUST

# Control 3 carries the extra total_fees function (the third copy of the fee policy).
IFS= read -r -d '' TOTAL_FEES_CHECK <<'RUST' || true
fn extra_checks(records: &[ledger::Record], failures: &mut Vec<String>) {
    match ledger::total_fees(records) {
        Ok(114) => {}
        other => failures.push(format!("total_fees(SAMPLE) = {other:?}")),
    }
}
RUST
IFS= read -r -d '' NO_EXTRA_CHECKS <<'RUST' || true
fn extra_checks(_records: &[ledger::Record], _failures: &mut Vec<String>) {}
RUST

for control in "$HERE"/control-*.rs; do
  name="$(basename "$control" .rs)"
  index="${name#control-}"
  index="${index%%-*}"
  dir="$WORK/$name"
  mkdir -p "$dir"
  extra="$NO_EXTRA_CHECKS"
  if [ "$index" = "3" ]; then extra="$TOTAL_FEES_CHECK"; fi
  {
    printf '#[path = "%s"]\nmod ledger;\n\nconst CONTROL: u32 = %s;\n\n' "$control" "$index"
    printf '%s\n\n%s\n' "$extra" "$DRIVER_BODY"
  } > "$dir/main.rs"
  if ! rustc --edition 2021 -D warnings -o "$dir/driver" "$dir/main.rs" > "$dir/build.log" 2>&1; then
    echo "FAIL $name: does not compile warning-free"
    sed 's/^/    /' "$dir/build.log"
    failures=$((failures + 1))
    continue
  fi
  if output="$("$dir/driver")"; then
    echo "ok   $name"
  else
    echo "FAIL $name"
    printf '%s\n' "$output" | sed 's/^/    /'
    failures=$((failures + 1))
  fi
done

# Control 2 must be exactly rustfmt 1.9.0 (edition 2021, default config) applied to control 1.
if command -v rustfmt > /dev/null && rustfmt --version | grep -q '^rustfmt 1\.9\.0'; then
  : > "$WORK/rustfmt.toml"
  if (cd "$WORK" && rustfmt --edition 2021 --config-path "$WORK/rustfmt.toml" \
      < "$HERE/control-1-compressed.rs" | cmp -s - "$HERE/control-2-formatted.rs"); then
    echo "ok   control 2 is rustfmt of control 1"
  else
    echo "FAIL control 2 is not rustfmt 1.9.0 output for control 1"
    failures=$((failures + 1))
  fi
else
  echo "skip control 2 format check (needs rustfmt 1.9.0, found: $(rustfmt --version 2>/dev/null || echo none))"
fi

# Control 6 is control 2 with comment lines added and nothing else changed.
if grep -v '^[[:space:]]*//' "$HERE/control-6-narrated.rs" | cmp -s - "$HERE/control-2-formatted.rs"; then
  echo "ok   control 6 code lines equal control 2"
else
  echo "FAIL control 6 code lines differ from control 2"
  failures=$((failures + 1))
fi

if [ "$failures" -eq 0 ]; then
  echo "all Rust controls pass"
else
  echo "$failures failures"
  exit 1
fi
