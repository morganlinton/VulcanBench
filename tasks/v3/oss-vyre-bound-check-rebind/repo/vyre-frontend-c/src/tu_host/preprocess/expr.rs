use std::collections::HashMap;

use super::{is_ident_continue, is_ident_start, MacroDef};

mod literals;
mod parser;
#[cfg(test)]
mod tests;
mod tokenize;

use literals::{parse_preproc_char_literal, parse_preproc_integer_literal};
use tokenize::{tokenize_preproc_expr, ExprTok};

pub(super) use parser::eval_preproc_expr;
