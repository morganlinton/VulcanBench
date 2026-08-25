#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]

pub(crate) fn eval_or(args: &[u32]) -> u32 {
    match args {
        [left, right] => left | right,
        _ => panic!("Fix: eval_or expected exactly 2 args, got {}", args.len()),
    }
}
