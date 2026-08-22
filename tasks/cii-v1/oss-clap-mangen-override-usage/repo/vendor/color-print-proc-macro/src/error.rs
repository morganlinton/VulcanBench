use std::fmt;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::ToTokens;

/// An error with an optional span. Most errors will have a span, the only exception is on a
/// [`Error::Parse`], which can occur in the [`get_args_and_format_string()`] function.
///
/// [`get_args_and_format_string()`]: crate::format_args::get_args_and_format_string()
#[derive(Debug, Clone)]
pub struct SpanError {
    pub err: Error,
    pub span: Option<Span>,
}

impl SpanError {
    /// Creates a new `SpanError`.
    pub fn new(err: Error, span: Option<Span>) -> Self {
        Self { err, span }
    }
}

/// Manual implementation because [`Span`] is not [`PartialEq`] and can be ignored when comparing.
impl PartialEq for SpanError {
    fn eq(&self, other: &SpanError) -> bool {
        self.err == other.err
    }
}

impl From<Error> for SpanError {
    fn from(err: Error) -> SpanError {
        SpanError { err, span: None }
    }
}

impl ToTokens for SpanError {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let span = self.span.unwrap_or_else(Span::call_site);
        let token_stream_err = syn::Error::new(span, self.err.clone()).to_compile_error();
        token_stream_err.to_tokens(tokens);
    }
}

/// All possible errors which can occur when calling one of the public macros.
#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    /// Error during the initial parsing of the macro arguments.
    Parse(String),
    /// The first macro argument is not a string literal.
    MustBeStringLiteral,
    /// Unable to parse a tag.
    UnableToParseTag(String),
    /// An error occured while parsing a color tag.
    ParseTag(String),
    /// A "{" character has not been closed in the format string.
    UnclosedPlaceholder,
    /// A "<" character has not been closed in the format string.
    UnclosedTag,
    /// Trying to close a previous tag, while there are no open tag.
    NoTagToClose,
    /// Trying to close a previous tag which does not match, like `<red>...</blue>`.
    MismatchCloseTag(String, String),
    /// Only one argument is allowed for the `cstr!()` and `untagged!()` macros.
    #[cfg(not(feature = "terminfo"))]
    TooManyArgs,
    /// Only one argument is allowed for the '`untagged!()` macro.
    #[cfg(feature = "terminfo")]
    TooManyArgs,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            Self::Parse(msg) => msg.clone(),
            Self::MustBeStringLiteral => "Format string must be a string literal".to_owned(),
            Self::UnableToParseTag(tag) => format!("Unable to parse the tag {}", tag),
            Self::ParseTag(detail) => detail.clone(),
            Self::UnclosedPlaceholder => "Unclosed placeholder".to_owned(),
            Self::UnclosedTag => "Unclosed color tag".to_owned(),
            Self::NoTagToClose => "No color tag to close".to_owned(),
            Self::MismatchCloseTag(tag1, tag2) => {
                format!("Mismatch close tag between {} and {}", tag1, tag2)
            }
            Self::TooManyArgs => "Too many arguments".to_owned(),
        };
        write!(f, "{}", msg)
    }
}
