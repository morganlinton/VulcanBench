//! This module permits to determine which ANSI sequences have to be added at a given position in
//! the format string, by saving the current tags in a "context". When a new tag is encountered, a
//! diff between the old state and the new state is performed to determine the right ANSI sequences
//! to add.

use std::convert::TryFrom;

use proc_macro2::Span;

use crate::error::{Error, SpanError};

/// Stores all the current open tags encountered in the format string.
#[derive(Debug, PartialEq, Default)]
pub struct Context<'a>(Vec<ColorTag<'a>>);

impl<'a> Context<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies a group of tags to the current context, and returns a list of the terminfo
    /// constants (available in the `color-print` package) to be added as named arguments at the
    /// end of the format arguments.
    ///
    /// For each given tag:
    ///  - if the tag is an open tag, push it into the context;
    ///  - if it's a valid close tag, pop the last open tag.
    #[cfg(feature = "terminfo")]
    pub fn terminfo_apply_tags(
        &mut self,
        tag_group: Vec<ColorTag<'a>>,
    ) -> Result<Vec<String>, SpanError> {
        let state_diff = self.apply_tags_and_get_diff(tag_group)?;
        Ok(state_diff.terminfo_token_streams())
    }

    /// Applies a group of tags to the current context, and returns the ANSI sequences to be
    /// added into the format string.
    ///
    /// For each given tag:
    ///  - if the tag is an open tag, push it into the context;
    ///  - if it's a valid close tag, pop the last open tag.
    #[cfg(not(feature = "terminfo"))]
    pub fn ansi_apply_tags(&mut self, tag_group: Vec<ColorTag<'a>>) -> Result<String, SpanError> {
        let state_diff = self.apply_tags_and_get_diff(tag_group)?;
        Ok(state_diff.ansi_string())
    }

    /// Applies a group of tags to the current context, with no return on success. Used by the
    /// macro `untagged!()`.
    ///
    /// For each given tag:
    ///  - if the tag is an open tag, push it into the context;
    ///  - if it's a valid close tag, pop the last open tag.
    pub fn apply_tags(&mut self, tag_group: Vec<ColorTag<'a>>) -> Result<(), SpanError> {
        self.apply_tags_and_get_diff(tag_group).map(|_| ())
    }

    /// Returns the actual color/style state, which is the result of the changes made by each tag
    /// sequentially.
    pub fn state(&self) -> State {
        let mut state = State::default();
        for tag in &self.0 {
            if let Some(ref color) = tag.change_set.foreground {
                state.foreground = ExtColor::Color(color.clone());
            }
            if let Some(ref color) = tag.change_set.background {
                state.background = ExtColor::Color(color.clone());
            }
            state.bold |= tag.change_set.bold;
            state.dim |= tag.change_set.dim;
            state.underline |= tag.change_set.underline;
            state.italics |= tag.change_set.italics;
            state.blink |= tag.change_set.blink;
            state.strike |= tag.change_set.strike;
            state.reverse |= tag.change_set.reverse;
            state.conceal |= tag.change_set.conceal;
        }
        state
    }

    #[allow(rustdoc::broken_intra_doc_links)]
    /// Common code betwwen [Self::terminfo_apply_tag()] and [Self::ansi_apply_tag()].
    fn apply_tags_and_get_diff(&mut self, tags: Vec<ColorTag<'a>>) -> Result<StateDiff, SpanError> {
        let old_state = self.state();

        for tag in tags {
            if tag.is_close {
                let last_tag = self.0.last()
                    .ok_or_else(|| SpanError::new(Error::NoTagToClose, tag.span))?;
                // If the tag is "void" (it is a "</>" tag), we don't need to check if the change
                // sets are matching:
                if !tag.change_set.is_void() && last_tag.change_set != tag.change_set {
                    let (last_src, src) = (
                        // We can unwrap the last tag source, because we know that all the tags
                        // stored inside the context are *open tags*, and open tag are always taken
                        // from the source input:
                        last_tag.source.unwrap(),
                        // We can unwrap the source of the tag currently being processed, because
                        // we just checked above that the tag is not void, and non-void tags are
                        // always taken from the source input:
                        tag.source.unwrap(),
                    );
                    return Err(SpanError::new(
                        Error::MismatchCloseTag(last_src.to_owned(), src.to_owned()),
                        tag.span,
                    ));
                }
                self.0.pop().unwrap();
            } else {
                self.0.push(tag);
            }
        }

        let new_state = self.state();
        Ok(StateDiff::from_diff(&old_state, &new_state))
    }
}

/// Describes the state of each color and style attributes at a given position in the format
/// string. Two states can be compared together by creating a [`StateDiff`] instance.
#[derive(Debug, PartialEq, Default)]
pub struct State {
    foreground: ExtColor,
    background: ExtColor,
    bold: bool,
    dim: bool,
    underline: bool,
    italics: bool,
    blink: bool,
    strike: bool,
    reverse: bool,
    conceal: bool,
}

/// The result of the comparison between two [`State`]s.
///
/// Each field is an [`Action`], which indicates if the given value has to be changed or left
/// unchanged in order to reach the new state.
#[derive(Debug)]
pub struct StateDiff {
    foreground: Action<ExtColor>,
    background: Action<ExtColor>,
    bold: Action<bool>,
    dim: Action<bool>,
    underline: Action<bool>,
    italics: Action<bool>,
    blink: Action<bool>,
    #[cfg(not(feature = "terminfo"))]
    strike: Action<bool>,
    reverse: Action<bool>,
    #[cfg(not(feature = "terminfo"))]
    conceal: Action<bool>,
}

impl StateDiff {
    /// Creates a new [`StateDiff`] by comparing two [`State`]s.
    pub fn from_diff(old: &State, new: &State) -> Self {
        StateDiff {
            foreground: Action::from_diff(Some(old.foreground.clone()), Some(new.foreground.clone())),
            background: Action::from_diff(Some(old.background.clone()), Some(new.background.clone())),
            bold: Action::from_diff(Some(old.bold), Some(new.bold)),
            dim: Action::from_diff(Some(old.dim), Some(new.dim)),
            underline: Action::from_diff(Some(old.underline), Some(new.underline)),
            italics: Action::from_diff(Some(old.italics), Some(new.italics)),
            blink: Action::from_diff(Some(old.blink), Some(new.blink)),
            #[cfg(not(feature = "terminfo"))]
            strike: Action::from_diff(Some(old.strike), Some(new.strike)),
            reverse: Action::from_diff(Some(old.reverse), Some(new.reverse)),
            #[cfg(not(feature = "terminfo"))]
            conceal: Action::from_diff(Some(old.conceal), Some(new.conceal)),
        }
    }

    /// Returns the list of terminfo constants (available in the `color-print` package) which have
    /// to be used in order to reach the new state.
    #[cfg(feature = "terminfo")]
    pub fn terminfo_token_streams(&self) -> Vec<String> {
        let mut constants = vec![];

        macro_rules! push_constant {
            ($s:expr) => {{
                constants.push($s.to_owned());
            }};
        }

        let have_to_reset = or!(
            matches!(self.foreground, Action::Change(ExtColor::Normal)),
            matches!(self.background, Action::Change(ExtColor::Normal)),
            matches!(self.bold, Action::Change(false)),
            matches!(self.dim, Action::Change(false)),
            matches!(self.blink, Action::Change(false)),
            matches!(self.reverse, Action::Change(false)),
        );

        if have_to_reset {
            push_constant!("CLEAR");
            if let Some(ExtColor::Color(Color::Color16(color))) = self.foreground.actual_value() {
                push_constant!(color.terminfo_constant(true));
            }
            if let Some(ExtColor::Color(Color::Color16(color))) = self.background.actual_value() {
                push_constant!(color.terminfo_constant(false));
            }
            if matches!(self.bold.actual_value(), Some(true)) {
                push_constant!("BOLD");
            }
            if matches!(self.dim.actual_value(), Some(true)) {
                push_constant!("DIM");
            }
            if matches!(self.blink.actual_value(), Some(true)) {
                push_constant!("BLINK");
            }
            if matches!(self.underline.actual_value(), Some(true)) {
                push_constant!("UNDERLINE");
            }
            if matches!(self.italics.actual_value(), Some(true)) {
                push_constant!("ITALICS");
            }
            if matches!(self.reverse.actual_value(), Some(true)) {
                push_constant!("REVERSE");
            }
        } else {
            if let Action::Change(ExtColor::Color(Color::Color16(ref color))) = self.foreground {
                push_constant!(color.terminfo_constant(true));
            }
            if let Action::Change(ExtColor::Color(Color::Color16(ref color))) = self.background {
                push_constant!(color.terminfo_constant(false));
            }
            if let Action::Change(true) = self.bold {
                push_constant!("BOLD");
            }
            if let Action::Change(true) = self.dim {
                push_constant!("DIM");
            }
            if let Action::Change(true) = self.blink {
                push_constant!("BLINK");
            }
            if let Action::Change(true) = self.reverse {
                push_constant!("REVERSE");
            }
            if let Action::Change(underline) = self.underline {
                let constant = if underline { "UNDERLINE" } else { "NO_UNDERLINE" };
                push_constant!(constant);
            }
            if let Action::Change(italics) = self.italics {
                let constant = if italics { "ITALICS" } else { "NO_ITALICS" };
                push_constant!(constant);
            }
        }

        constants
    }

    /// Returns the ANSI sequence(s) which has to added to the format string in order to reach the
    /// new state.
    #[cfg(not(feature = "terminfo"))]
    pub fn ansi_string(&self) -> String {
        use crate::ansi_constants::*;

        let mut output = String::new();

        macro_rules! push_code {
            ($($codes:expr),*) => { output.push_str(&generate_ansi_code(&[$($codes),*])) };
        }

        if let Action::Change(ref ext_color) = self.foreground {
            match ext_color {
                ExtColor::Normal => push_code!(DEFAULT_FOREGROUND),
                ExtColor::Color(Color::Color16(color)) => match color.intensity {
                    Intensity::Normal => {
                        push_code!(SET_FOREGROUND_BASE + color.base_color.index())
                    }
                    Intensity::Bright => {
                        push_code!(SET_BRIGHT_FOREGROUND_BASE + color.base_color.index())
                    }
                },
                ExtColor::Color(Color::Color256(color)) => {
                    push_code!(SET_FOREGROUND, 5, color.0);
                },
                ExtColor::Color(Color::ColorRgb(color)) => {
                    push_code!(SET_FOREGROUND, 2, color.r, color.g, color.b);
                },
            }
        }

        if let Action::Change(ref ext_color) = self.background {
            match ext_color {
                ExtColor::Normal => push_code!(DEFAULT_BACKGROUND),
                ExtColor::Color(Color::Color16(color)) => match color.intensity {
                    Intensity::Normal => {
                        push_code!(SET_BACKGROUND_BASE + color.base_color.index())
                    }
                    Intensity::Bright => {
                        push_code!(SET_BRIGHT_BACKGROUND_BASE + color.base_color.index())
                    }
                },
                ExtColor::Color(Color::Color256(color)) => {
                    push_code!(SET_BACKGROUND, 5, color.0);
                },
                ExtColor::Color(Color::ColorRgb(color)) => {
                    push_code!(SET_BACKGROUND, 2, color.r, color.g, color.b);
                },
            }
        }

        macro_rules! handle_attr {
            ($attr:expr, $true_val:expr, $false_val:expr) => {
                match $attr {
                    Action::Change(true) => push_code!($true_val),
                    Action::Change(false) => push_code!($false_val),
                    _ => (),
                }
            };
        }

        handle_attr!(self.bold, BOLD, NO_BOLD);
        handle_attr!(self.dim, DIM, NO_BOLD);
        handle_attr!(self.underline, UNDERLINE, NO_UNDERLINE);
        handle_attr!(self.italics, ITALIC, NO_ITALIC);
        handle_attr!(self.blink, BLINK, NO_BLINK);
        handle_attr!(self.strike, STRIKE, NO_STRIKE);
        handle_attr!(self.reverse, REVERSE, NO_REVERSE);
        handle_attr!(self.conceal, CONCEAL, NO_CONCEAL);

        output
    }
}

/// The action to be performed on a given color/style attribute in order to reach a new state.
#[derive(Debug, PartialEq)]
pub enum Action<T> {
    /// Nothing has to be done, because this value was never modified.
    None,
    /// This attribute has to be kept the same.
    /// With the terminfo implementation, it's not possible to reset each style/color
    /// independently, so we have to keep track of the values, even with the `Keep` variant.
    Keep(T),
    /// This attribute value has to be changed.
    Change(T),
}

#[cfg(feature = "terminfo")]
impl<T> Action<T> {
    pub fn actual_value(&self) -> Option<&T> {
        match self {
            Action::Keep(val) | Action::Change(val) => Some(val),
            Action::None => None,
        }
    }
}

impl<T> Action<T>
where
    T: PartialEq,
{
    /// Creates a new [`Action`].
    pub fn from_diff(old: Option<T>, new: Option<T>) -> Self {
        let eq = old == new;
        match (old, new, eq) {
            (Some(old_val), Some(_), true) | (Some(old_val), None, _) => Action::Keep(old_val),
            (_, Some(new_val), _) => Action::Change(new_val),
            _ => Action::None,
        }
    }
}

/// A parsed color/style tag.
#[derive(Debug, Default)]
pub struct ColorTag<'a> {
    /// Source of the tag in the format string.
    pub source: Option<&'a str>,
    /// Span of the tag in the format string.
    pub span: Option<Span>,
    /// Is it a close tag like `</red>`.
    pub is_close: bool,
    /// The changes that are implied by this tag.
    pub change_set: ChangeSet,
}

impl<'a> PartialEq for ColorTag<'a> {
    fn eq(&self, other: &ColorTag<'a>) -> bool {
        and!(
            self.source == other.source,
            self.is_close == other.is_close,
            self.change_set == other.change_set,
        )
    }
}

impl<'a> ColorTag<'a> {
    /// Creates a new close tag; only used in order to auto-close unclosed tags at the end of the
    /// format string.
    pub fn new_close() -> Self {
        ColorTag {
            source: None,
            span: None,
            is_close: true,
            change_set: ChangeSet::default(),
        }
    }

    /// Sets the span of the tag.
    pub fn set_span(&mut self, span: Span) {
        self.span = Some(span);
    }
}

/// The changes that are implied by a tag.
#[derive(Debug, PartialEq, Default)]
pub struct ChangeSet {
    /// If it is `Some`, then the foreground color has to be changed.
    pub foreground: Option<Color>,
    /// If it is `Some`, then the background color has to be changed.
    pub background: Option<Color>,
    /// If it is `true`, then the bold attribute has to be set (or unset for a close tag).
    pub bold: bool,
    /// If it is `true`, then the dim attribute has to be set (or unset for a close tag).
    pub dim: bool,
    /// If it is `true`, then the underline attribute has to be set (or unset for a close tag).
    pub underline: bool,
    /// If it is `true`, then the italics attribute has to be set (or unset for a close tag).
    pub italics: bool,
    /// If it is `true`, then the blink attribute has to be set (or unset for a close tag).
    pub blink: bool,
    /// If it is `true`, then the strike attribute has to be set (or unset for a close tag).
    pub strike: bool,
    /// If it is `true`, then the reverse attribute has to be set (or unset for a close tag).
    pub reverse: bool,
    /// If it is `true`, then the conceal attribute has to be set (or unset for a close tag).
    pub conceal: bool,
}

impl ChangeSet {
    /// Checks if there is nothing to change (used to detect the `</>` tag).
    pub fn is_void(&self) -> bool {
        and!(
            self.foreground.is_none(),
            self.background.is_none(),
            !self.bold,
            !self.dim,
            !self.underline,
            !self.italics,
            !self.blink,
            !self.strike,
            !self.reverse,
            !self.conceal,
        )
    }
}

impl From<&[Change]> for ChangeSet {
    fn from(changes: &[Change]) -> ChangeSet {
        let mut change_set = ChangeSet::default();
        for change in changes {
            match change {
                Change::Foreground(color) => change_set.foreground = Some(color.clone()),
                Change::Background(color) => change_set.background = Some(color.clone()),
                Change::Bold => change_set.bold = true,
                Change::Dim => change_set.dim = true,
                Change::Underline => change_set.underline = true,
                Change::Italics => change_set.italics = true,
                Change::Blink => change_set.blink = true,
                Change::Strike => change_set.strike = true,
                Change::Reverse => change_set.reverse = true,
                Change::Conceal => change_set.conceal = true,
            }
        }
        change_set
    }
}

/// A single change to be done inside a tag. Tags with multiple keywords like `<red;bold>` will
/// have multiple [`Change`]s.
#[derive(Debug, PartialEq, Clone)]
pub enum Change {
    Foreground(Color),
    Background(Color),
    Bold,
    Dim,
    Underline,
    Italics,
    Blink,
    Strike,
    Reverse,
    Conceal,
}

impl TryFrom<&str> for Change {
    type Error = ();

    /// Tries to convert a keyword like `red`, `bold` into a [`Change`] instance.
    #[rustfmt::skip]
    fn try_from(input: &str) -> Result<Self, Self::Error> {
        macro_rules! color16 {
            ($kind:ident $intensity:ident $base_color:ident) => {
                Change::$kind(Color::Color16(Color16::new(
                    BaseColor::$base_color,
                    Intensity::$intensity,
                )))
            };
        }

        let change = match input {
            "s" | "strong" | "bold" | "em" => Change::Bold,
            "dim" => Change::Dim,
            "u" | "underline" => Change::Underline,
            "i" | "italic" | "italics" => Change::Italics,
            "blink" => Change::Blink,
            "strike" => Change::Strike,
            "reverse" | "rev" => Change::Reverse,
            "conceal" | "hide" => Change::Conceal,

            "k" | "black"   => color16!(Foreground Normal Black),
            "r" | "red"     => color16!(Foreground Normal Red),
            "g" | "green"   => color16!(Foreground Normal Green),
            "y" | "yellow"  => color16!(Foreground Normal Yellow),
            "b" | "blue"    => color16!(Foreground Normal Blue),
            "m" | "magenta" => color16!(Foreground Normal Magenta),
            "c" | "cyan"    => color16!(Foreground Normal Cyan),
            "w" | "white"   => color16!(Foreground Normal White),

            "k!" | "black!" | "bright-black"     => color16!(Foreground Bright Black),
            "r!" | "red!" | "bright-red"         => color16!(Foreground Bright Red),
            "g!" | "green!" | "bright-green"     => color16!(Foreground Bright Green),
            "y!" | "yellow!" | "bright-yellow"   => color16!(Foreground Bright Yellow),
            "b!" | "blue!" | "bright-blue"       => color16!(Foreground Bright Blue),
            "m!" | "magenta!" | "bright-magenta" => color16!(Foreground Bright Magenta),
            "c!" | "cyan!" | "bright-cyan"       => color16!(Foreground Bright Cyan),
            "w!" | "white!" | "bright-white"     => color16!(Foreground Bright White),

            "K" | "bg-black"   => color16!(Background Normal Black),
            "R" | "bg-red"     => color16!(Background Normal Red),
            "G" | "bg-green"   => color16!(Background Normal Green),
            "Y" | "bg-yellow"  => color16!(Background Normal Yellow),
            "B" | "bg-blue"    => color16!(Background Normal Blue),
            "M" | "bg-magenta" => color16!(Background Normal Magenta),
            "C" | "bg-cyan"    => color16!(Background Normal Cyan),
            "W" | "bg-white"   => color16!(Background Normal White),

            "K!" | "bg-black!" | "bg-bright-black"     => color16!(Background Bright Black),
            "R!" | "bg-red!" | "bg-bright-red"         => color16!(Background Bright Red),
            "G!" | "bg-green!" | "bg-bright-green"     => color16!(Background Bright Green),
            "Y!" | "bg-yellow!" | "bg-bright-yellow"   => color16!(Background Bright Yellow),
            "B!" | "bg-blue!" | "bg-bright-blue"       => color16!(Background Bright Blue),
            "M!" | "bg-magenta!" | "bg-bright-magenta" => color16!(Background Bright Magenta),
            "C!" | "bg-cyan!" | "bg-bright-cyan"       => color16!(Background Bright Cyan),
            "W!" | "bg-white!" | "bg-bright-white"     => color16!(Background Bright White),

            _ => return Err(()),
        };

        Ok(change)
    }
}

/// Which "kind" of color has to be changed.
#[derive(Debug, PartialEq, Clone)]
pub enum ColorKind {
    Background,
    Foreground,
}

impl ColorKind {
    pub fn to_change(&self, color: Color) -> Change {
        match self {
            Self::Foreground => Change::Foreground(color),
            Self::Background => Change::Background(color),
        }
    }
}

/// An "extended" color, which can be either a real color or the "normal", default color.
#[derive(Debug, PartialEq, Clone)]
pub enum ExtColor {
    Normal,
    Color(Color),
}

impl Default for ExtColor {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, PartialEq, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum Color {
    Color16(Color16),
    Color256(Color256),
    ColorRgb(ColorRgb),
}

/// A terminal color.
#[derive(Debug, PartialEq, Clone)]
pub struct Color16 {
    base_color: BaseColor,
    intensity: Intensity,
}

impl Color16 {
    pub fn new(base_color: BaseColor, intensity: Intensity) -> Self {
        Self { base_color, intensity }
    }

    /// Converts a color to a terminfo constant name (available in the `color-print` package).
    #[cfg(feature = "terminfo")]
    pub fn terminfo_constant(&self, is_foreground: bool) -> String {
        let mut constant = if is_foreground {
            String::new()
        } else {
            "BG_".to_string()
        };

        if matches!(self.intensity, Intensity::Bright) {
            constant.push_str("BRIGHT_");
        }

        constant.push_str(self.base_color.uppercase_str());

        constant
    }
}

/// The intensity of a terminal color.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Intensity {
    Normal,
    Bright,
}

impl Intensity {
    pub fn new(is_bright: bool) -> Self {
        if is_bright {
            Self::Bright
        } else {
            Self::Normal
        }
    }
}

/// A "base" terminal color, which has to be completed with an [`Intensity`] in order to describe a
/// whole terminal color.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum BaseColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

impl BaseColor {
    /// Return the index of a color, in the same ordering as the ANSI color sequences.
    #[cfg(not(feature = "terminfo"))]
    pub fn index(&self) -> u8 {
        match self {
            Self::Black => 0,
            Self::Red => 1,
            Self::Green => 2,
            Self::Yellow => 3,
            Self::Blue => 4,
            Self::Magenta => 5,
            Self::Cyan => 6,
            Self::White => 7,
        }
    }

    /// Used to generate terminfo constants, see [`Color16::terminfo_constant()`].
    #[cfg(feature = "terminfo")]
    pub fn uppercase_str(&self) -> &'static str {
        match self {
            Self::Black => "BLACK",
            Self::Red => "RED",
            Self::Green => "GREEN",
            Self::Yellow => "YELLOW",
            Self::Blue => "BLUE",
            Self::Magenta => "MAGENTA",
            Self::Cyan => "CYAN",
            Self::White => "WHITE",
        }
    }
}

/// A color in the 256-color palette.
#[derive(Debug, PartialEq, Clone)]
pub struct Color256(pub u8);

/// An RGB color.
#[derive(Debug, PartialEq, Clone)]
pub struct ColorRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "terminfo")]
    use super::*;
    #[cfg(feature = "terminfo")]
    use crate::parse::color_tag;

    #[test]
    #[cfg(feature = "terminfo")]
    fn terminfo_apply_tag_to_context() {
        let mut context = Context::new();

        macro_rules! apply_tag {
            ($s:expr) => {
                context
                    .terminfo_apply_tags(vec![color_tag($s).unwrap().1])
                    .unwrap()
            };
        }

        let constants = apply_tag!("<r>");
        assert_eq!(constants, ["RED"]);
        let constants = apply_tag!("</r>");
        assert_eq!(constants, ["CLEAR"]);
        let constants = apply_tag!("<r>");
        assert_eq!(constants, ["RED"]);
        let constants = apply_tag!("<s>");
        assert_eq!(constants, ["BOLD"]);
        let constants = apply_tag!("</s>");
        assert_eq!(constants, ["CLEAR", "RED"]);
        let constants = apply_tag!("</r>");
        assert_eq!(constants, ["CLEAR"]);
    }

    #[test]
    #[cfg(feature = "terminfo")]
    fn terminfo_apply_tag_to_context_2() {
        let mut context = Context::new();

        macro_rules! apply_tag {
            ($s:expr) => {
                context
                    .terminfo_apply_tags(vec![color_tag($s).unwrap().1])
                    .unwrap()
            };
        }

        let constants = apply_tag!("<r>");
        assert_eq!(constants, ["RED"]);
        let constants = apply_tag!("<Y>");
        assert_eq!(constants, ["BG_YELLOW"]);
        let constants = apply_tag!("<s>");
        assert_eq!(constants, ["BOLD"]);
        let constants = apply_tag!("<u>");
        assert_eq!(constants, ["UNDERLINE"]);
        let constants = apply_tag!("</u>");
        assert_eq!(constants, ["NO_UNDERLINE"]);
        let constants = apply_tag!("</s>");
        assert_eq!(constants, ["CLEAR", "RED", "BG_YELLOW"]);
    }

    #[test]
    #[cfg(feature = "terminfo")]
    fn terminfo_apply_tag_to_context_3() {
        let mut context = Context::new();

        macro_rules! apply_tag {
            ($s:expr) => {
                context.terminfo_apply_tags(vec![color_tag($s).unwrap().1])
            };
        }

        let res = apply_tag!("</r>");
        assert_eq!(res, Err(SpanError::new(Error::NoTagToClose, None)));
        apply_tag!("<r>").unwrap();
        let res = apply_tag!("</s>");
        assert_eq!(
            res,
            Err(SpanError::new(
                Error::MismatchCloseTag("<r>".to_owned(), "</s>".to_owned()),
                None
            ))
        );
    }
}
