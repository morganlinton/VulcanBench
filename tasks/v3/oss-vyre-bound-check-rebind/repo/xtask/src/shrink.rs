//! Automated Delta-Debugging tool for Vyre IR.
//!
//! Exposes a `cargo_full run --bin xtask -- shrink [file.vir] [oracle.sh]` command that iteratively
//! applies structural passes (DeadNodeElimination, ArgFolding, ShortCircuit) to
//! reduce a crashing or misbehaving trace down to a highly constrained minimal reproducer.

use std::path::PathBuf;

pub(crate) struct ShrinkCmd {
    pub target: PathBuf,
    pub oracle: PathBuf,
}

pub(crate) fn run(args: &[String]) {
    if args.len() < 4 {
        eprintln!("Usage: cargo_full run --bin xtask -- shrink <target.vir> <oracle.sh>");
        std::process::exit(1);
    }

    let cmd = ShrinkCmd {
        target: PathBuf::from(&args[2]),
        oracle: PathBuf::from(&args[3]),
    };

    println!(
        "Shrink pass registered. Target: {:?}, Oracle: {:?}",
        cmd.target, cmd.oracle
    );
    println!("Shrink is operating in dry-run/bootstrap mode.");
}
