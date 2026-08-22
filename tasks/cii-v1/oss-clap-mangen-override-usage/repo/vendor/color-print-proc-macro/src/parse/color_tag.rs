use std::borrow::Cow;

use nom::{
    Err,
    branch::alt,
    bytes::complete::{tag, take_while_m_n},
    character::complete::{space0, alphanumeric1, alpha1, u8, digit1},
    combinator::{consumed, map, map_res},
    multi::separated_list1,
    sequence::{tuple, delimited, preceded, pair, terminated}, error::ErrorKind,
};

use super::{Input, Result, Error, Parser, ErrorDetail};
use super::util::*;
use crate::color_context::{
    Change, ChangeSet, Color, Color16, Color256, ColorRgb, ColorTag, ColorKind, BaseColor, Intensity,
};

/// Indicates wether a colored is specified by the prefix "fg:" or "bg:".
#[derive(Debug, Clone, Copy)]
enum Specified {
    True,
    False,
}

impl Specified {
    #[inline]
    fn is_true(&self) -> bool {
        matches!(self, Specified::True)
    }
}

/// Indicates a color has to be searched in its lowercase or uppercase version.
#[derive(Debug, Clone, Copy)]
enum Case {
    Uppercase,
    Lowercase,
}

/// Parses a color tag.
pub fn color_tag(input: Input<'_>) -> Result<'_, ColorTag> {
    let tag = alt((
        map(
            tuple((tag("</"), space0, tag(">"))),
            |_| (true, vec![])
        ),
        delimited(
            tag("<"),
            alt((
                map(
                    preceded(tag("/"), spaced(separated_list1(stag(","), spaced(attr)))),
                    |attrs| (true, attrs)
                ),
                map(
                    separated_list1(stag(","), spaced(attr)),
                    |attrs| (false, attrs)
                ),
            )),
            tag(">"),
        ),
    ));

    with_failure_message(
        map(
            consumed(tag),
            |(source, (is_close, changes))| ColorTag {
                source: Some(source),
                span: None,
                is_close,
                change_set: ChangeSet::from(changes.as_ref()),
            }
        ),
        "Unable to parse this tag"
    )(input)
}

/// Parses any attributes inside a color tag.
fn attr(input: Input<'_>) -> Result<'_, Change> {
    let mut parser = alt((
        style_attr,
        map(tuple((color_kind_specifier, specified_color)), |(kind, color)| kind.to_change(color)),
        map(color_16(Case::Lowercase), |color_16| Change::Foreground(Color::Color16(color_16))),
        map(
            color_256(Specified::False),
            |(color_256, color_kind)| color_kind.unwrap().to_change(Color::Color256(color_256))
        ),
        map(
            color_rgb(Specified::False),
            |(color_rgb, color_kind)| color_kind.unwrap().to_change(Color::ColorRgb(color_rgb))
        ),
        map(color_16(Case::Uppercase), |color_16| Change::Background(Color::Color16(color_16))),
    ));

    parser(input).map_err(|e| {
        match e {
            Err::Error(_) => {
                let msg = match alphanumeric1::<&str, Error>(input) {
                    Ok((_, attr)) => format!("Unknown color attribute: <{attr}>"),
                    Err(_) => "Unable to parse this attribute".to_string(),
                };
                Err::Failure(Error::new(input, ErrorKind::Alpha, Some(ErrorDetail::new(input, msg))))
            }
            e => e
        }
    })
}

/// Parses a style attribute.
fn style_attr(input: Input<'_>) -> Result<'_, Change> {
    let (input, word) = alpha1(input)?;
    let change = match word {
        "s" | "strong" | "bold" | "em" => Change::Bold,
        "dim" => Change::Dim,
        "u" | "underline" => Change::Underline,
        "i" | "italic" | "italics" => Change::Italics,
        "blink" => Change::Blink,
        "strike" => Change::Strike,
        "reverse" | "rev" => Change::Reverse,
        "conceal" | "hide" => Change::Conceal,
        _ => { return Err(Err::Error(Error::new(input, ErrorKind::Tag, None))) }
    };
    Ok((input, change))
}

/// Parses specifiers like `"bg:"`.
fn color_kind_specifier(input: Input<'_>) -> Result<'_, ColorKind> {
    check_parser_before_failure(
        pair(spaced(alpha1), stag(":")),
        terminated(
            alt((
                map(word(alt((tag("fg"), tag("f")))), |_| ColorKind::Foreground),
                map(word(alt((tag("bg"), tag("b")))), |_| ColorKind::Background),
            )),
            stag(":"),
        ),
        "Unknown specifier, allowed specifiers are \"bg\" or \"fg\" (shortcuts: \"b\" or \"f\")"
    )
    (input)
}

/// Parses a color which has been prefixed by a specifier like `"bg:"` or `"fg:"`.
fn specified_color(input: Input<'_>) -> Result<'_, Color> {
    with_failure_message(
        alt((
            map(color_16(Case::Lowercase), Color::Color16),
            map(color_256(Specified::True), |(color, _)| Color::Color256(color)),
            map(color_rgb(Specified::True), |(color, _)| Color::ColorRgb(color)),
        )),
        "Unknown color"
    )
    (input)
}

/// Parses a basic color like `"blue"`, `"b"`, `"blue!"`, `"bright-blue"`, with the given letter
/// case.
fn color_16<'a>(letter_case: Case) -> impl Parser<'a, Color16> {
    move |input| {
        let bright_prefix = match letter_case {
            Case::Uppercase => "BRIGHT-",
            Case::Lowercase => "bright-",
        };
        alt((
            map(
                preceded(tag(bright_prefix), base_color(letter_case)),
                |base_color| Color16::new(base_color, Intensity::Bright)
            ),
            map(
                pair(spaced(base_color(letter_case)), is_present(spaced(tag("!")))),
                |(base_color, is_bright)| Color16::new(base_color, Intensity::new(is_bright))
            )
        ))
        (input)
    }
}

/// Parses a 256-color color, like `"pal(42)"`. If the color to parse is declared as "specified",
/// the only the lowercase functions will be available.
fn color_256<'a>(specified: Specified) -> impl Parser<'a, (Color256, Option<ColorKind>)>
{
    const PALETTE_FAILURE_MESSAGE: &str = "Palette color must a number between 0 and 255";

    fn pal_color(input: Input<'_>) -> Result<'_, u8> {
        with_failure_message(u8, PALETTE_FAILURE_MESSAGE)(input)
    }

    fn pal_fn<'a>(name1: &'a str, name2: &'a str, name3: &'a str) -> impl Parser<'a, u8> {
        let function_names = alt((tag(name1), tag(name2), tag(name3)));
        function(
            function_names,
            with_failure_message(pal_color, PALETTE_FAILURE_MESSAGE)
        )
    }

    fn pal_lower(input: Input<'_>) -> Result<'_, Color256> {
        map(alt((
            pal_fn("palette", "pal", "p"),
            check_parser_before_failure(digit1, u8, PALETTE_FAILURE_MESSAGE)
        )), Color256)(input)
    }

    fn pal_upper(input: Input<'_>) -> Result<'_, Color256> {
        map(pal_fn("PALETTE", "PAL", "P"), Color256)(input)
    }

    if specified.is_true() {
        |input| {
            map(pal_lower, |color| (color, None))
            (input)
        }
    } else {
        |input| {
            alt((
                map(pal_lower, |color| (color, Some(ColorKind::Foreground))),
                map(pal_upper, |color| (color, Some(ColorKind::Background)))
            ))
            (input)
        }
    }
}

/// Parses a true-color color, like `"rgb(10,20,30)"`. If the color to parse is declared as
/// "specified", the only the lowercase functions will be available.
fn color_rgb<'a>(specified: Specified) -> impl Parser<'a, (ColorRgb, Option<ColorKind>)> {
    fn component(input: Input<'_>) -> Result<'_, u8> {
        with_failure_message(u8, "Bad RGB color component: must be a number between 0 and 255")
        (input)
    }

    fn rgb_fn(name: &str) -> impl Parser<'_, ColorRgb> {
        map(
            function(
                tag(name),
                with_failure_message(
                    tuple((component, stag(","), component, stag(","), component)),
                    "Wrong arguments: expects 3 numbers between 0 and 255, separated by commas"
                )
            ),
            |(r, _, g, _, b)| ColorRgb { r, g, b }
        )
    }

    fn rgb_lower(input: Input<'_>) -> Result<'_, ColorRgb> {
        rgb_fn("rgb")(input)
    }

    fn rgb_upper(input: Input<'_>) -> Result<'_, ColorRgb> {
        rgb_fn("RGB")(input)
    }

    if specified.is_true() {
        |input| {
            map(alt((rgb_lower, hex_rgb_color)), |color| (color, None))
            (input)
        }
    } else {
        |input| {
            alt((
                map(rgb_lower, |color| (color, Some(ColorKind::Foreground))),
                map(rgb_upper, |color| (color, Some(ColorKind::Background))),
                map(hex_rgb_color, |color| (color, Some(ColorKind::Foreground))),
            ))
            (input)
        }
    }
}

/// Parses an HTML-like color like `"#aabbcc"`.
fn hex_rgb_color(input: Input<'_>) -> Result<'_, ColorRgb> {
    fn component(input: Input<'_>) -> Result<'_, u8> {
        map_res(
            take_while_m_n(2, 2, |c: char| c.is_ascii_hexdigit()),
            |input| u8::from_str_radix(input, 16)
        )
        (input)
    }

    map(
        preceded(
            tag("#"),
            with_failure_message(
                tuple((component, component, component)),
                "Bad hexadecimal color code"
            )
        ),
        |(r, g ,b)| ColorRgb { r, g, b }
    )
    (input)
}

/// Parses a base color name, like "blue", "red", in the given letter case.
fn base_color<'a>(letter_case: Case) -> impl Parser<'a, BaseColor> {
    move |input| {
        let (input, word) = match letter_case {
            Case::Uppercase => {
                let (input, word) = uppercase_word(input)?;
                (input, Cow::Owned(word.to_ascii_lowercase()))
            }
            Case::Lowercase => {
                let (input, word) = lowercase_word(input)?;
                (input, Cow::Borrowed(word))
            }
        };

        let base_color = match word.as_ref() {
            "k" | "black"   => BaseColor::Black,
            "r" | "red"     => BaseColor::Red,
            "g" | "green"   => BaseColor::Green,
            "y" | "yellow"  => BaseColor::Yellow,
            "b" | "blue"    => BaseColor::Blue,
            "m" | "magenta" => BaseColor::Magenta,
            "c" | "cyan"    => BaseColor::Cyan,
            "w" | "white"   => BaseColor::White,
            _ => { return Err(Err::Error(Error::new(input, ErrorKind::Tag, None))) }
        };
        Ok((input, base_color))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_context::{Color, Color16, BaseColor, Intensity};

    macro_rules! tag {
        ($source:expr, $is_close:expr, $($changes:expr),*) => {
            ColorTag {
                source: Some($source),
                span: None,
                is_close: $is_close,
                change_set: ChangeSet::from(&[$($changes),*][..]),
            }
        };
    }
    macro_rules! open_tag {
        ($source:expr, [ $($changes:expr),* $(,)? ]) => { tag!($source, false, $($changes),*) };
    }
    macro_rules! close_tag {
        ($source:expr, [ $($changes:expr),* $(,)? ]) => { tag!($source, true, $($changes),*) };
    }
    macro_rules! color16 {
        ($base_color:ident, $intensity:ident) => {
            Color::Color16(Color16::new(BaseColor::$base_color, Intensity::$intensity))
        }
    }

    #[test]
    fn parse_change() {
        let change = attr("b").unwrap().1;
        assert_eq!(change, Change::Foreground(color16!(Blue, Normal)));
        let change = attr("s").unwrap().1;
        assert_eq!(change, Change::Bold);
    }

    #[test]
    fn parse_tag() {
        let tag = color_tag("<s>").unwrap().1;
        assert_eq!(tag, open_tag!("<s>", [Change::Bold]));

        let tag = color_tag("<s,y!>...").unwrap().1;
        assert_eq!(
            tag,
            open_tag!(
                "<s,y!>",
                [
                    Change::Bold,
                    Change::Foreground(color16!(Yellow, Bright)),
                ]
            )
        );

        let tag = color_tag("</u,y,k,B>...").unwrap().1;
        assert_eq!(
            tag,
            close_tag!(
                "</u,y,k,B>",
                [
                    Change::Underline,
                    Change::Foreground(color16!(Black, Normal)),
                    Change::Background(color16!(Blue, Normal)),
                ]
            )
        );
    }

    #[test]
    fn parse_color256() {
        let tag = color_tag("<48>").unwrap().1;
        assert_eq!(tag, open_tag!("<48>", [Change::Foreground(Color::Color256(Color256(48)))]));
        let tag = color_tag("<fg:48>").unwrap().1;
        assert_eq!(tag, open_tag!("<fg:48>", [Change::Foreground(Color::Color256(Color256(48)))]));
        let tag = color_tag("<bg:48>").unwrap().1;
        assert_eq!(tag, open_tag!("<bg:48>", [Change::Background(Color::Color256(Color256(48)))]));
        let tag = color_tag("<PAL(48)>").unwrap().1;
        assert_eq!(tag, open_tag!("<PAL(48)>", [Change::Background(Color::Color256(Color256(48)))]));
    }

    #[test]
    fn parse_color_rgb() {
        let tag = color_tag("<rgb(1,2,3)>").unwrap().1;
        assert_eq!(tag, open_tag!("<rgb(1,2,3)>", [
            Change::Foreground(Color::ColorRgb(ColorRgb{ r: 1, g: 2, b: 3}))
        ]));

        let tag = color_tag("<RGB(1,2,3)>").unwrap().1;
        assert_eq!(tag, open_tag!("<RGB(1,2,3)>", [
            Change::Background(Color::ColorRgb(ColorRgb{ r: 1, g: 2, b: 3}))
        ]));

        let tag = color_tag("<rgb( 1 , 2 , 3  )>").unwrap().1;
        assert_eq!(tag, open_tag!("<rgb( 1 , 2 , 3  )>", [
            Change::Foreground(Color::ColorRgb(ColorRgb{ r: 1, g: 2, b: 3}))
        ]));

        let tag = color_tag("<  #102030 >").unwrap().1;
        assert_eq!(tag, open_tag!("<  #102030 >", [
            Change::Foreground(Color::ColorRgb(ColorRgb{ r: 16, g: 32, b: 48}))
        ]));
    }

    #[test]
    fn spaces_in_tag() {
        let tag = color_tag("<s  >").unwrap().1;
        assert_eq!(tag, open_tag!("<s  >", [Change::Bold]));

        let tag = color_tag("<  s>").unwrap().1;
        assert_eq!(tag, open_tag!("<  s>", [Change::Bold]));

        let tag = color_tag("<  s   > ...").unwrap().1;
        assert_eq!(tag, open_tag!("<  s   >", [Change::Bold]));

        let tag = color_tag("<  s  ,   \t y!>...").unwrap().1;
        assert_eq!(
            tag,
            open_tag!(
                "<  s  ,   \t y!>",
                [
                    Change::Bold,
                    Change::Foreground(color16!(Yellow, Bright)),
                ]
            )
        );
    }

    #[test]
    fn empty_tag_is_err() {
        assert!(color_tag("<>").is_err());
        assert!(color_tag("<  >").is_err());
    }
}
