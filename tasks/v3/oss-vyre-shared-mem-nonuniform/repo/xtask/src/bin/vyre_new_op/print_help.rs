#![allow(missing_docs)]
use super::reserved_id_env::RESERVED_ID_ENV;

pub(crate) fn print_help() {
    println!(
        "Usage:\n  vyre new-op <id> --archetype <archetype> [--display-name <text>] [--summary <text>] [--category <A|C>]"
    );
    println!();
    println!("Examples:");
    println!(
        "  cargo_full run -p vyre --bin vyre_new_op -- new-op primitive.arithmetic.test_op --archetype binary-arithmetic"
    );
    println!();
    println!(
        "Reserved prefixes 'internal.' and 'test.' require {}=1.",
        RESERVED_ID_ENV
    );
}
