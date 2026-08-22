//! An argument in a `format!`-like macro, parsable and transformable to tokens.

use proc_macro2::TokenStream as TokenStream2;
use quote::ToTokens;
use syn::{
    parse::{Parse, ParseStream, Result},
    token, Expr, Ident, Token,
};

/// An argument in a `format!`-like macro (excluding the first argument aka the format string).
pub struct FormatArg {
    /// The argument name in the case of a named argument, e.g. `foo` inside `foo = 1 + 1`.
    pub arg_name: Option<(Ident, token::Eq)>,
    /// The real argument to be formatted by the macro.
    pub expr: Expr,
}

impl Parse for FormatArg {
    fn parse(input: ParseStream) -> Result<Self> {
        let arg_name: Option<(Ident, token::Eq)> = if input.peek2(Token![=]) {
            Some((input.parse()?, input.parse()?))
        } else {
            None
        };
        let expr: Expr = input.parse()?;

        Ok(FormatArg { arg_name, expr })
    }
}

impl ToTokens for FormatArg {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        if let Some((arg_name, eq)) = &self.arg_name {
            arg_name.to_tokens(tokens);
            eq.to_tokens(tokens);
        }
        self.expr.to_tokens(tokens);
    }
}
