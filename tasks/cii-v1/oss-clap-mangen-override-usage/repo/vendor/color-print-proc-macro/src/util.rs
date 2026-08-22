use std::ops::RangeBounds;

use proc_macro2::{Ident, Span};
use syn::LitStr;

/// Joins the arguments with `&&` operators.
macro_rules! and {
    ($($expr:expr),* $(,)?) => {
        $($expr)&&*
    };
}

/// Joins the arguments with `||` operators.
#[cfg(feature = "terminfo")]
macro_rules! or {
    ($($expr:expr),* $(,)?) => {
        $($expr)||*
    };
}

/// Creates a new [`Ident`] which can be tokenized.
pub fn ident(s: &str) -> Ident {
    Ident::new(s, Span::call_site())
}

/// Creates a new [`struct@LitStr`] which can be tokenized.
pub fn literal_string(s: &str) -> LitStr {
    LitStr::new(s, Span::call_site())
}

/// Unfortunately, unless a nightly compiler is used, this function will actually only return the
/// original input span.
///
/// Returns the subspan corresponding to the range of `inside` inside `input`, considering that:
///  - `input` is exactly `&input_lit_str.value()`,
///  - `inside` is a subslice of `input`,
///
/// Warning: may panic if the conditions are not met.
/// TODO: improve safety
pub fn inner_span<'a>(input: &'a str, input_lit_str: &LitStr, inside: &'a str) -> Span {
    let input_offset = (inside.as_ptr() as usize) - (input.as_ptr() as usize);
    let range = input_offset + 1..input_offset + inside.len() + 1;
    subspan(input_lit_str.span(), range).unwrap_or_else(|| input_lit_str.span())
}

/// Returns a subspan of the given span.
///
/// TODO: the implementation is really... wtf! But i didn't find a better way to do it.
fn subspan<R: RangeBounds<usize>>(span: Span, range: R) -> Option<Span> {
    let mut lit = proc_macro2::Literal::i8_suffixed(0); // wtf...
    lit.set_span(span);
    lit.subspan(range)
}
