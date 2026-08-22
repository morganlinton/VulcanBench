use nom::{
    Err,
    sequence::{delimited, preceded},
    character::complete::{multispace0, alpha1},
    bytes::complete::tag,
    combinator::{map, opt},
    error::ErrorKind,
};

use super::{Parser, Result, Input, Error, ErrorDetail};

/// Transforms an error into a failure, while adding a message in the error detail.
pub fn with_failure_message<'a, P, V>(mut parser: P, message: &'a str) -> impl Parser<'a, V>
where
    P: Parser<'a, V>,
{
    move |input: Input<'a>| parser(input).map_err(
        |nom_err: Err<Error>| match nom_err {
            Err::Error(e) => {
                Err::Failure(e.with_detail(ErrorDetail::new(input, message)))
            }
            e => e,
        }
    )
}

/// Checks if the first parser succeeds, then parses the input with the second parser. If an error
/// is encountered with the second parser, then a failure message is thrown.
pub fn check_parser_before_failure<'a, C, CV, P, PV>(
    mut check_parser: C,
    mut parser: P,
    failure_msg: &'a str
) -> impl Parser<'a, PV>
where
    C: Parser<'a, CV>,
    P: Parser<'a, PV>,
{
    move |input| {
        check_parser(input)?;
        with_failure_message(|input| { parser(input) }, failure_msg)
        (input)
    }
}

/// Creates a parser which accpets spaces around the original parsed input.
pub fn spaced<'a, P, V>(parser: P) -> impl Parser<'a, V>
where
    P: Parser<'a, V>,
{
    delimited(
        multispace0,
        parser,
        multispace0,
    )
}

/// Parsed a spaced tag.
pub fn stag(s: &str) -> impl Parser<'_, &str> {
    spaced(tag(s))
}

/// Creates a parser which makes the parser optional and returns true if the parse was successful.
pub fn is_present<'a, P, V>(parser: P) -> impl Parser<'a, bool>
where
    P: Parser<'a, V>,
{
    map(opt(parser), |v| v.is_some())
}

/// Creates a parser which parses a function call.
pub fn function<'a, PV, N, P>(word_parser: N, parser: P) -> impl Parser<'a, PV>
where
    N: Parser<'a, &'a str>,
    P: Parser<'a, PV>,
{
    preceded(
        word(word_parser),
        delimited(
            with_failure_message(stag("("), "Missing opening brace"),
            parser,
            with_failure_message(stag(")"), "Missing closing brace")
        )
    )
}

/// Parses a word made only by alpha characters ('a' => 'z' and 'A' => 'Z'), and checks if this
/// word matches exactly the given parser.
pub fn word<'a, P>(mut word_parser: P) -> impl Parser<'a, &'a str>
where
    P: Parser<'a, &'a str>,
{
    move |input| {
        let (input, word) = alpha1(input)?;
        match word_parser(word) {
            Ok((_, parsed_word)) => {
                if word == parsed_word {
                    Ok((input, word))
                } else {
                    Err(Err::Error(Error::new(input, ErrorKind::Alpha, None)))
                }
            }
            Err(e) => Err(e),
        }
    }
}

/// Parses an uppercase word.
pub fn uppercase_word(input: Input<'_>) -> Result<'_, &str> {
    let (input, word) = alpha1(input)?;
    if word.chars().all(|c| c.is_ascii_uppercase()) {
        Ok((input, word))
    } else {
        Err(Err::Error(Error::new(input, ErrorKind::Alpha, None)))
    }
}

/// Parses a lowercase word.
pub fn lowercase_word(input: Input<'_>) -> Result<'_, &str> {
    let (input, word) = alpha1(input)?;
    if word.chars().all(|c| c.is_ascii_lowercase()) {
        Ok((input, word))
    } else {
        Err(Err::Error(Error::new(input, ErrorKind::Alpha, None)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uppercase_word() {
        let input = "foo";
        assert!(uppercase_word(input).is_err());
        let input = "FOOfoo";
        assert!(uppercase_word(input).is_err());
        let input = "FOO";
        assert!(uppercase_word(input).is_ok());
        let input = "FOO;;";
        assert!(uppercase_word(input).is_ok());
    }
}
