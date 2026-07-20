/// Bound on `#if` expression recursion depth. The preprocessor conditional
/// evaluator is recursive descent: `parse_conditional` self-recurses for the
/// ternary `?:` operator, and `parse_unary` self-recurses for `! ~ + -` chains
/// and routes back to `parse_conditional` for parenthesized sub-expressions.
/// Without a bound, hostile input like `#if ((((...))))`, `#if !!!!...1`, or
/// `#if 1?1:1?1:...` overflows the native stack and aborts the process (an
/// uncatchable SIGABRT, not a recoverable error). 256 levels is far beyond any
/// legitimate header's nesting while keeping native stack usage safe under the
/// default 8 MiB thread stack (each level descends ~11 precedence frames).
pub(crate) const MAX_PP_EXPR_DEPTH: usize = 256;

pub(crate) struct PreprocessorExprParser<'src, 'defs, 'name> {
    pub(crate) bytes: &'src [u8],
    pub(crate) index: usize,
    pub(crate) base_offset: usize,
    pub(crate) defined_macros: &'defs [&'name [u8]],
    /// Current recursion depth of the conditional-expression evaluator; bounded
    /// by [`MAX_PP_EXPR_DEPTH`] so malformed/hostile `#if` expressions fail
    /// closed with a `CPreprocessorError` instead of overflowing the stack.
    pub(crate) depth: usize,
}
