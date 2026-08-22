use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::LitStr;

use crate::color_context::Context;
use crate::error::SpanError;
use crate::format_args::{get_args_and_format_string, parse_format_string, Node};
use crate::util;

/// Common code shared between the public macros, terminfo implementation.
pub fn get_format_args(input: TokenStream) -> Result<TokenStream2, SpanError> {
    let (format_string_token, args) = get_args_and_format_string(input)?;
    let format_string = format_string_token.value();

    // Split the format string into a list of nodes; each node is either a string literal (text), a
    // placeholder for a `format!()` related macro, or a color code:
    let format_nodes = parse_format_string(&format_string, &format_string_token)?;

    // The final, modified format string which will be given to a `format!()`-like macro:
    let mut final_format_string = String::new();

    // Stores which colors and attributes are set while processing the format string:
    let mut color_context = Context::new();

    // Used to generate extra named arguments:
    let mut current_color_idx = 0;
    // The list of the extra named arguments to add at the end of the `format!`-like macro:
    let mut color_format_args: Vec<TokenStream2> = vec![];

    // Generate the final format string, and construct the list of the extra named parameters at
    // the same time:
    for node in format_nodes {
        match node {
            Node::Text(s) | Node::Placeholder(s) => {
                final_format_string.push_str(s);
            }
            Node::ColorTagGroup(tag_group) => {
                let constants = color_context
                    .terminfo_apply_tags(tag_group)?
                    .iter()
                    .map(|s| constant_to_token_stream(s))
                    .collect::<Vec<_>>();
                for constant in constants {
                    // Add "{}" to the format string, and add the right ANSI sequence as a format
                    // argument:
                    let varname = format!("__color_print__color_{}", current_color_idx);
                    final_format_string.push_str(&format!("{{{}}}", varname));
                    current_color_idx += 1;
                    let varname_ident = util::ident(&varname);
                    let token_stream = quote! { #varname_ident = #constant }.into();
                    color_format_args.push(token_stream);
                }
            }
        }
    }

    // Group all the final arguments into a single iterator:
    let format_string_span = format_string_token.span();
    let final_format_string =
        LitStr::new(&final_format_string, format_string_span).to_token_stream();
    let final_args = std::iter::once(final_format_string)
        .chain(args.iter().map(|arg| arg.to_token_stream()).skip(1))
        .chain(color_format_args.into_iter());

    Ok((quote! { #(#final_args),* }).into())
}

/// Creates a new terminfo constant (available in the `color-print` package) as a token stream.
fn constant_to_token_stream(constant: &str) -> TokenStream2 {
    let constant_ident = util::ident(constant);
    (quote! { *color_print::#constant_ident }).into()
}
