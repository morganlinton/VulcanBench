//! Standalone entry point for the shape-test audit.
mod lint_shape_tests {
    include!("../lint_shape_tests.rs");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    lint_shape_tests::run(&args);
}
