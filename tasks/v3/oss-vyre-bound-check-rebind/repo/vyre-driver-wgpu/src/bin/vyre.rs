#![deny(unsafe_code)]
//! Installable `vyre` command-line entry point.
//!
//! The `demo` subcommand builds a minimal vyre IR Program (write the
//! value 42 into an output buffer), dispatches it via the wgpu
//! backend, and prints the resulting u32. This is the canonical
//! "vyre works on this machine" smoke test  -  deliberately uses
//! vyre's IR + Program + VyreBackend surface, NOT raw wgpu. If the
//! demo ever needs raw WGSL to work, that's a failure of vyre's
//! abstraction, not the demo's shape.

use std::process::ExitCode;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--version") | Some("-V") => {
            println!("vyre {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("demo") => {
            let value = run_demo()?;
            println!("vyre demo gpu_u32={value}");
            Ok(())
        }
        Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!(
            "unknown vyre command `{other}`. Fix: use `vyre --version` or `vyre demo`."
        )),
    }
}

fn print_help() {
    println!("vyre {}", env!("CARGO_PKG_VERSION"));
    println!("usage: vyre --version | vyre demo");
    println!();
    println!("  demo       build a minimal vyre IR Program, dispatch on the local GPU,");
    println!("             and print the resulting u32 (expected: 42).");
}

fn run_demo() -> Result<u32, String> {
    // Pure vyre IR: one read-write u32 buffer, one Store node writing
    // the literal 42 at index 0. No WGSL, no naga, no hand-written
    // kernel  -  the backend lowers this to a compute pipeline and
    // returns the bytes.
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_output_byte_range(0..4),
        ],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    );

    let backend = vyre_driver_wgpu::WgpuBackend::acquire().map_err(|error| {
        format!(
            "failed to acquire wgpu backend: {error}. Fix: install a compatible GPU driver \
             (Vulkan / Metal / DX12) or run on a host with GPU access."
        )
    })?;

    // Input buffer list mirrors the program's non-output buffers; our
    // demo has only one read-write output so inputs is empty.
    let outputs = backend
        .dispatch(&program, &[], &DispatchConfig::default())
        .map_err(|error| {
            format!("demo dispatch failed: {error}. Fix: inspect the GPU driver logs.")
        })?;

    let bytes = outputs
        .first()
        .ok_or_else(|| "demo returned no output buffers; expected one.".to_string())?;
    let slice: [u8; 4] = bytes[..4]
        .try_into()
        .map_err(|_| "demo output buffer shorter than 4 bytes.".to_string())?;
    Ok(u32::from_le_bytes(slice))
}
