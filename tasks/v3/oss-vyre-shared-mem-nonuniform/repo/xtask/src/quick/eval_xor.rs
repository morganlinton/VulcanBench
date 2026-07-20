pub(crate) fn eval_xor(args: &[u32]) -> u32 {
    match args {
        [left, right] => left ^ right,
        _ => panic!("Fix: eval_xor expected exactly 2 args, got {}", args.len()),
    }
}
