use std::env::{self, VarError};
use std::fs;
use std::path::Path;
use std::process;

const TEMPLATE: &str = "\
/// Produces the value of TARGET as a string literal.
#[macro_export]
macro_rules! target {
    () => {
        \"{TARGET}\"
    };
}

/// Produces the value of HOST as a string literal.
#[macro_export]
macro_rules! host {
    () => {
        \"{HOST}\"
    };
}
";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(host_os, values(\"windows\"))");

    let target = env_var("TARGET");
    let host = env_var("HOST");
    let out_dir = env_var("OUT_DIR");
    let out = Path::new(&out_dir).join("macros.rs");
    let macros = TEMPLATE
        .replace("{TARGET}", &target.escape_debug().to_string())
        .replace("{HOST}", &host.escape_debug().to_string());
    fs::write(out, macros).unwrap();

    if let Some("windows") = host.split('-').nth(2) {
        println!("cargo:rustc-cfg=host_os=\"windows\"");
    }
}

fn env_var(key: &str) -> String {
    match env::var(key) {
        Ok(value) => value,
        Err(VarError::NotPresent) => {
            eprintln!("Environment variable ${key} is not set during execution of build script");
            process::exit(1);
        }
        Err(VarError::NotUnicode(_)) => {
            eprintln!("Environment variable ${key} is non-UTF8 during execution of build script");
            process::exit(1);
        }
    }
}
