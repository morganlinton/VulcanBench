//! `cargo_full run --bin xtask -- trace-f32 <op_id>`  -  run an op's registered fixture on the
//! pure-Rust reference interpreter and dump output bytes as the exact
//! `Vec<Vec<Vec<u8>>>` literal that paste into `OpEntry::expected_output`.
//!
//! Purpose: mechanise V7-TEST-002/003/004 (softmax / attention / layer_norm
//! f32 fixtures) and any future op whose expected outputs are too unwieldy
//! to hand-compute. The reference is the canonical CPU oracle; whatever it
//! emits is the spec.
//!
//! Output format:
//! ```text
//! Some(|| vec![
//!     vec![
//!         vec![0xab, 0xcd, ...],   // input set 0, buffer 0
//!         vec![0xff, 0x00, ...],   // input set 0, buffer 1
//!     ],
//!     vec![ ... ],                  // input set 1
//! ])
//! ```

use std::process;

use vyre::ir::Program;
use vyre_intrinsics::harness::OpEntry as IntrinsicsEntry;
use vyre_libs::harness::OpEntry as LibsEntry;
use vyre_reference::reference_eval;
use vyre_reference::value::Value;

/// Entry point for the `trace-f32` subcommand.
pub(crate) fn run_cmd(args: &[String]) {
    let op_id = match args.get(2) {
        Some(s) => s.as_str(),
        None => {
            eprintln!(
                "Fix: usage: cargo_full run --bin xtask -- trace-f32 <op_id>\n\
                 Walks vyre-libs + vyre-intrinsics inventories, finds the registered \
                 OpEntry, runs its `test_inputs()` against `vyre_reference::reference_eval`, \
                 and prints the byte-identical expected-output literal."
            );
            process::exit(1);
        }
    };

    let (program, inputs_per_run) = match resolve(op_id) {
        Some(t) => t,
        None => {
            eprintln!(
                "Fix: op id '{op_id}' not registered, or registered without `test_inputs`. \
                 Add `test_inputs: Some(|| vec![vec![/* input bytes per buffer */]])` \
                 to its `OpEntry` first; this tool then computes the expected outputs."
            );
            process::exit(1);
        }
    };

    println!("Some(|| vec![");
    for (run_idx, input_set) in inputs_per_run.iter().enumerate() {
        let values: Vec<Value> = input_set
            .iter()
            .map(|bytes| Value::Bytes(bytes.clone().into()))
            .collect();
        let outputs = match reference_eval(&program, &values) {
            Ok(out) => out
                .into_iter()
                .map(|v| v.to_bytes())
                .collect::<Vec<Vec<u8>>>(),
            Err(error) => {
                eprintln!(
                    "Fix: vyre-reference rejected input set {run_idx} for `{op_id}`: {error}. \
                     Either repair the program or replace the offending input."
                );
                process::exit(2);
            }
        };
        println!("    vec![                                           // run {run_idx}");
        for (buf_idx, bytes) in outputs.iter().enumerate() {
            print!("        vec![");
            for (i, byte) in bytes.iter().enumerate() {
                if i > 0 && i % 16 == 0 {
                    print!("\n             ");
                }
                print!("0x{byte:02x}, ");
            }
            println!("],   // output buffer {buf_idx} ({} bytes)", bytes.len());
        }
        println!("    ],");
    }
    println!("])");
}

fn resolve(op_id: &str) -> Option<(Program, Vec<Vec<Vec<u8>>>)> {
    if let Some(entry) = vyre_libs::harness::all_entries().find(|e: &&LibsEntry| e.id == op_id) {
        let inputs = entry.test_inputs?;
        return Some(((entry.build)(), (inputs)()));
    }
    if let Some(entry) =
        vyre_intrinsics::harness::all_entries().find(|e: &&IntrinsicsEntry| e.id == op_id)
    {
        let inputs = entry.test_inputs?;
        return Some(((entry.build)(), (inputs)()));
    }
    None
}
