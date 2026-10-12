#![allow(missing_docs)]
//! Command-line scaffolder for new operations.
//!
//! Usage:
//!   `cargo_full run -p vyre --bin vyre_new_op -- new-op <id> --archetype <archetype> [--display-name <text>] [--summary <text>] [--category <A|C>]`

mod allowed_archetypes;
mod generate_mod_rs;
mod generate_readme;
mod generate_required_impl_rs;
mod generate_spec_toml;
mod id_to_title_case;
mod is_maintainer_allowed;
mod max_id_len;
mod print_help;
mod reserved_id_env;
mod run;
mod split_id_into_path;
mod validate_archetype;
mod validate_id;
mod write_scaffold_file;

use std::env;
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    if let Err(error) = run::run(&mut args) {
        eprintln!("{error}");
        process::exit(1);
    }
}
