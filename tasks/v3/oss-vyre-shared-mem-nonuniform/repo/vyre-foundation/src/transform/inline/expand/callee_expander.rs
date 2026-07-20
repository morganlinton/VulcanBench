use crate::ir::Expr;
use crate::ir::Ident;
use crate::transform::inline::InlineCtx;
use rustc_hash::FxHashMap as HashMap;

pub(crate) struct CalleeExpander<'a> {
    pub(crate) ctx: &'a mut InlineCtx,
    pub(crate) prefix: String,
    pub(crate) vars: HashMap<Ident, String>,
    pub(crate) input_args: HashMap<Ident, Expr>,
    pub(crate) output_name: Ident,
    pub(crate) result_name: String,
    pub(crate) saw_output: bool,
}
