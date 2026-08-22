//! Implements the [`crate::untagged!()`] proc macro.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::color_context::Context;
use crate::error::{SpanError, Error};
use crate::format_args::{
    parse_args, get_format_string, parse_format_string, Node
};

/// Transforms a string literal by removing all its color tags.
pub fn get_untagged(input: TokenStream) -> Result<TokenStream2, SpanError> {
    let args = parse_args(input)?;
    let format_string_token = get_format_string(args.first())?;
    let format_string = format_string_token.value();

    if args.len() > 1 {
        return Err(SpanError::new(Error::TooManyArgs, None));
    }

    // Split the format string into a list of nodes; each node is either a string literal (text),
    // or a color code; `format!`-like placeholders will be parsed indenpendently, but as they are
    // put back unchanged into the format string, it's not a problem:
    let format_nodes = parse_format_string(&format_string, &format_string_token)?;

    // The final, modified format string which will be given to the `format!`-like macro:
    let mut format_string = String::new();
    // Stores which colors and attributes are set while processing the format string:
    let mut color_context = Context::new();

    // Generate the final format string:
    for node in format_nodes {
        match node {
            Node::Text(s) | Node::Placeholder(s) => {
                format_string.push_str(s);
            }
            Node::ColorTagGroup(tag_group) => {
                // Don't add the ansi codes into the final format string, but still apply to tags
                // to the context in order to keep the error handling:
                color_context.apply_tags(tag_group)?;
            }
        }
    }

    Ok(quote! { #format_string })
}
