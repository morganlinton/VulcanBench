#[cfg(feature = "cpu-oracle")]
use std::collections::HashMap;

#[cfg(feature = "cpu-oracle")]
mod comments;
#[cfg(feature = "cpu-oracle")]
mod define_parse;
#[cfg(feature = "cpu-oracle")]
mod expr;
#[cfg(feature = "cpu-oracle")]
mod ident;
#[cfg(feature = "cpu-oracle")]
mod macro_expand;
#[cfg(feature = "cpu-oracle")]
mod macro_params;
#[cfg(feature = "cpu-oracle")]
mod reference;

#[cfg(feature = "cpu-oracle")]
pub(super) use comments::strip_directive_comments;
#[cfg(feature = "cpu-oracle")]
pub(super) use define_parse::parse_define;
#[cfg(feature = "cpu-oracle")]
use expr::eval_preproc_expr;
#[cfg(feature = "cpu-oracle")]
use ident::{is_ident_continue, is_ident_start};
#[cfg(feature = "cpu-oracle")]
use macro_expand::expand_line_macros;
#[cfg(feature = "cpu-oracle")]
use macro_params::{parse_macro_args, replace_macro_params};
#[cfg(feature = "cpu-oracle")]
pub use reference::reference_expand_preprocessor_macros;

#[cfg(feature = "cpu-oracle")]
const MAX_MACRO_EXPANSION_DEPTH: u32 = 32;

#[cfg(feature = "cpu-oracle")]
#[derive(Clone, Debug)]
pub(super) struct MacroDef {
    pub(super) params: Option<Vec<String>>,
    #[cfg(feature = "cpu-oracle")]
    pub(super) variadic: Option<String>,
    pub(super) replacement: String,
}

#[cfg(feature = "cpu-oracle")]
#[derive(Clone, Copy, Debug)]
struct ConditionalFrame {
    parent_active: bool,
    branch_taken: bool,
    current_active: bool,
    saw_else: bool,
}

#[cfg(feature = "cpu-oracle")]
pub(super) fn eval_preprocessor_condition(expr: &str, macros: &HashMap<String, MacroDef>) -> bool {
    eval_preproc_expr(expr, macros)
}
