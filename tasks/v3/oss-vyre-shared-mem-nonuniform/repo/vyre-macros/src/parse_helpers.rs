//! Shared parsers for proc-macro argument surfaces.

use syn::parse::ParseStream;
use syn::{Expr, ExprArray, Ident, LitStr};

/// Parse a bracketed array whose entries must all be string literals.
pub(crate) fn parse_litstr_array(
    input: ParseStream<'_>,
    error_message: &'static str,
) -> syn::Result<Vec<LitStr>> {
    let array: ExprArray = input.parse()?;
    array
        .elems
        .into_iter()
        .map(|expr| match expr {
            Expr::Lit(lit) => match lit.lit {
                syn::Lit::Str(value) => Ok(value),
                other => Err(syn::Error::new_spanned(other, error_message)),
            },
            other => Err(syn::Error::new_spanned(other, error_message)),
        })
        .collect()
}

/// Parse a bracketed array of Rust expressions.
pub(crate) fn parse_expr_array(input: ParseStream<'_>) -> syn::Result<Vec<Expr>> {
    let array: ExprArray = input.parse()?;
    Ok(array.elems.into_iter().collect())
}

/// Reject repeated top-level macro arguments before a later value can silently
/// override an earlier value.
pub(crate) fn reject_duplicate_key(
    seen: &mut std::collections::BTreeSet<String>,
    key: &Ident,
) -> syn::Result<String> {
    let key_name = key.to_string();
    if !seen.insert(key_name.clone()) {
        return Err(syn::Error::new(
            key.span(),
            format!(
                "duplicate macro argument `{key_name}`. Fix: keep exactly one `{key_name}` entry."
            ),
        ));
    }
    Ok(key_name)
}
