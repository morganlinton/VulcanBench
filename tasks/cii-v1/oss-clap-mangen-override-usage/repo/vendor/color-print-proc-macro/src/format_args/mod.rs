//! Parses the `format!`-like arguments and the format string.

mod format_arg;

use proc_macro::TokenStream;
use syn::spanned::Spanned;
use syn::{
    self, parse::Parser, punctuated::Punctuated, token::Comma, Expr, ExprLit, Lit, LitStr, Token,
};

use crate::color_context::ColorTag;
use crate::error::{Error, SpanError};
use crate::parse;
use crate::util::{self, inner_span};
use format_arg::FormatArg;

/// Retrieves the original format string and arguments given to the public macros.
pub fn get_args_and_format_string(
    input: TokenStream,
) -> Result<(LitStr, Punctuated<FormatArg, Comma>), SpanError> {
    let args = parse_args(input)?;
    let format_string = get_format_string(args.first())?;
    Ok((format_string, args))
}

/// Parses the arguments of a `format!`-like macro.
pub fn parse_args(input: TokenStream) -> Result<Punctuated<FormatArg, Comma>, SpanError> {
    let parser = Punctuated::<FormatArg, Token![,]>::parse_terminated;
    parser
        .parse(input)
        .map_err(|e| Error::Parse(e.to_string()).into())
}

/// Gets the format string.
pub fn get_format_string(arg: Option<&FormatArg>) -> Result<LitStr, SpanError> {
    match arg {
        Some(FormatArg {
            expr: Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }),
            ..
        }) => Ok(s.to_owned()),
        Some(bad_arg) => {
            Err(SpanError::new(Error::MustBeStringLiteral, Some(bad_arg.span()),))
        }
        None => Ok(util::literal_string(""))
    }
}

/// A node inside a format string. The two variants `Text` and `Placeholder` represent the usual
/// kind of nodes that can be found in `format!`-like macros.
///
/// E.g., the format string `"Hello, {:?}, happy day"` will have 3 nodes:
///  - `Text("Hello, ")`,
///  - `Placeholder("{:?}")`,
///  - `Text(", happy day")`.
///
/// The third kind of node: `Color(&str)`, represents a color code to apply.
///
/// E.g., the format string `"This is a <blue>{}<clear> idea"` will have 7 nodes:
///  - `Text("This is a ")`
///  - `Color("blue")`
///  - `Placeholder("{}")`
///  - `Color("clear")`
///  - `Text(" idea")`
#[derive(Debug)]
pub enum Node<'a> {
    Text(&'a str),
    Placeholder(&'a str),
    ColorTagGroup(Vec<ColorTag<'a>>),
}

/// Parses a format string which may contain usual format placeholders (`{...}`) as well as color
/// codes like `"<red>"`, `"<blue,bold>"`.
pub fn parse_format_string<'a>(
    input: &'a str,
    lit_str: &LitStr,
) -> Result<Vec<Node<'a>>, SpanError> {
    /// Representation of the parsing context. Each variant's argument is the start offset of the
    /// given parse context.
    enum Context {
        /// The char actually parsed is a textual character:
        Text(usize),
        /// The char actually parsed is part of a `format!`-like placeholder:
        Placeholder(usize),
        /// The char actually parsed is part of a color tag, like `<red>`:
        Color(usize),
    }

    macro_rules! span {
        ($inside:expr) => { inner_span(input, lit_str, $inside) };
    }
    macro_rules! err {
        ([$inside:expr] $($e:tt)*) => { SpanError::new($($e)*, Some(span!($inside))) };
        ($($e:tt)*) => { SpanError::new($($e)*, Some(lit_str.span())) };
    }

    let mut context = Context::Text(0);
    let mut nodes = vec![];
    let mut close_angle_bracket_idx: Option<usize> = None;
    let mut nb_open_tags: isize = 0;

    for (i, c) in input.char_indices() {
        match context {
            Context::Text(text_start) => {
                let mut push_text = false;
                if c == '{' {
                    // The start of a placeholder:
                    context = Context::Placeholder(i);
                    push_text = true;
                } else if c == '<' {
                    // The start of a color code:
                    context = Context::Color(i);
                    push_text = true;
                } else if c == '>' {
                    // Double close angle brackets ">>":
                    if let Some(idx) = close_angle_bracket_idx {
                        if i == idx + 1 {
                            context = Context::Text(i + 1);
                            push_text = true;
                        }
                        close_angle_bracket_idx = None;
                    } else {
                        close_angle_bracket_idx = Some(i);
                    }
                };
                if push_text && text_start != i {
                    nodes.push(Node::Text(&input[text_start..i]));
                }
            }
            Context::Placeholder(ph_start) => {
                if c == '{' && i == ph_start + 1 {
                    // Double curly brackets "{{":
                    context = Context::Text(ph_start);
                } else if c == '}' {
                    // The end of a placeholder:
                    nodes.push(Node::Placeholder(&input[ph_start..i + 1]));
                    context = Context::Text(i + 1);
                }
            }
            Context::Color(tag_start) => {
                if c == '<' && i == tag_start + 1 {
                    // Double open angle brackets "<<":
                    context = Context::Text(tag_start + 1);
                } else if c == '>' {
                    // The end of a color code:
                    let tag_input = &input[tag_start..i + 1];
                    let mut tag = parse::color_tag(tag_input)
                        .map_err(|e| {
                            use nom::Err;
                            let (input, error) = match e {
                                Err::Error(parse::Error { detail: Some(d), .. }) |
                                Err::Failure(parse::Error { detail: Some(d), .. }) => {
                                    (d.input, Error::ParseTag(d.message))
                                }
                                // Should never happen:
                                _ => (tag_input, Error::UnableToParseTag(tag_input.to_string())),
                            };
                            err!([input] error)
                        })?
                        .1;
                    tag.set_span(span!(tag_input));
                    nb_open_tags += if tag.is_close { -1 } else { 1 };
                    // Group consecutive tags into one group, in order to improve optimization
                    // (e.g., "<blue><green>" will be optimized by removing the useless "<blue>"
                    // ANSI sequence):
                    if let Some(Node::ColorTagGroup(last_tag_group)) = nodes.last_mut() {
                        last_tag_group.push(tag);
                    } else {
                        nodes.push(Node::ColorTagGroup(vec![tag]));
                    }
                    context = Context::Text(i + 1);
                }
            }
        }
    }

    // Process the end of the string:
    match context {
        Context::Text(text_start) => {
            if text_start != input.len() {
                nodes.push(Node::Text(&input[text_start..]));
            }

            // Auto-close remaining open tags:
            if nb_open_tags > 0 {
                let tags = (0..nb_open_tags)
                    .map(|_| ColorTag::new_close())
                    .collect::<Vec<_>>();
                nodes.push(Node::ColorTagGroup(tags));
            }

            Ok(nodes)
        }
        Context::Placeholder(start) => Err(err!([&input[start..]] Error::UnclosedPlaceholder)),
        Context::Color(start) => Err(err!([&input[start..]] Error::UnclosedTag)),
    }
}
