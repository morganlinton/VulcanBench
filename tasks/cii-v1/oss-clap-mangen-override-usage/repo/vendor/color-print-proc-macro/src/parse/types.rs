use std::fmt;

use nom::{
    IResult,
    error::{ParseError, FromExternalError, ErrorKind},
};

pub type Input<'a> = &'a str;
pub type Result<'a, V> = IResult<Input<'a>, V, Error<'a>>;

pub trait Parser<'a, V>: FnMut(Input<'a>) -> Result<'a, V> {}

impl<'a, V, F> Parser<'a, V> for F
    where F: FnMut(Input<'a>) -> Result<'a, V>
{}

#[derive(Debug, PartialEq, Clone)]
pub struct ErrorDetail<'a> {
    pub input: &'a str,
    pub message: String,
}

impl<'a> ErrorDetail<'a> {
    pub fn new(input: &'a str, message: impl Into<String>) -> Self {
        let input = &input[..input.len().min(1)];
        Self { input, message: message.into() }
    }
}

impl<'a> fmt::Display for ErrorDetail<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Replacement to [`nom::error::Error`].
#[derive(Debug, PartialEq)]
pub struct Error<'a> {
    pub input: Input<'a>,
    pub code: ErrorKind,
    pub detail: Option<ErrorDetail<'a>>,
}

impl<'a> Error<'a> {
    pub fn new(input: &'a str, code: ErrorKind, detail: Option<ErrorDetail<'a>>) -> Self {
        Error { input, code, detail }
    }

    pub fn with_detail(&self, detail: ErrorDetail<'a>) -> Self {
        Error { input: self.input, code: self.code, detail: Some(detail) }
    }
}

/// Mandatory [`ParseError`] implementation.
impl<'a> ParseError<Input<'a>> for Error<'a> {
    fn from_error_kind(input: Input<'a>, kind: ErrorKind) -> Self {
        Error { input, code: kind, detail: None }
    }

    fn append(_: Input<'a>, _: ErrorKind, other: Self) -> Self {
        other
    }
}

impl<'a, E> FromExternalError<Input<'a>, E> for Error<'a> {
  fn from_external_error(input: Input<'a>, kind: ErrorKind, _e: E) -> Self {
    Error { input, code: kind, detail: None }
  }
}
