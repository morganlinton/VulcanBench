//! Integration test crate for the containing Vyre package.

#![cfg(feature = "python-parser")]

use crate::parsing::python::lex::python312_lexer;
use crate::parsing::python::parse::calls::python312_extract_calls;
use crate::parsing::python::parse::decorators::python312_extract_decorators;
use crate::parsing::python::parse::structure::{
    python312_extract_imports, python312_extract_structure, python312_extract_with_blocks,
};

#[test]
fn python_programs_validate() {
    for program in [
        python312_lexer(
            "haystack",
            "tok_types",
            "tok_starts",
            "tok_lens",
            "counts",
            64,
        ),
        python312_extract_structure(
            "tok_types",
            "tok_starts",
            "tok_lens",
            "defs",
            "def_counts",
            64,
        ),
        python312_extract_imports(
            "tok_types",
            "tok_starts",
            "tok_lens",
            "imports",
            "import_counts",
            64,
        ),
        python312_extract_with_blocks(
            "tok_types",
            "tok_starts",
            "tok_lens",
            "withs",
            "with_counts",
            64,
        ),
        python312_extract_calls(
            "tok_types",
            "tok_starts",
            "tok_lens",
            "calls",
            "call_counts",
            "kwargs",
            "kw_counts",
            64,
        ),
        python312_extract_decorators(
            "tok_types",
            "tok_starts",
            "tok_lens",
            "decorators",
            "decorator_counts",
            64,
        ),
    ] {
        let errors = vyre::validate(&program);
        assert!(
            errors.is_empty(),
            "python parser program must validate: {errors:?}"
        );
    }
}
