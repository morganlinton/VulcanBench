//! Lazy constants representing the ANSI codes for setting terminal styles, like bold, underline,
//! etc...
//!
//! Terminfo is used internally to guess the right codes.

use lazy_static::lazy_static;
use terminfo_crate::{capability as cap, expand, Capability};

use crate::terminfo::TERMINFO;

lazy_static! {
    pub static ref CLEAR: String = style::<cap::ExitAttributeMode>();
    pub static ref BOLD: String = style::<cap::EnterBoldMode>();
    pub static ref DIM: String = style::<cap::EnterDimMode>();
    pub static ref BLINK: String = style::<cap::EnterBlinkMode>();
    pub static ref ITALICS: String = style::<cap::EnterItalicsMode>();
    pub static ref REVERSE: String = style::<cap::EnterReverseMode>();
    pub static ref UNDERLINE: String = style::<cap::EnterUnderlineMode>();
    pub static ref NO_ITALICS: String = style::<cap::ExitItalicsMode>();
    pub static ref NO_UNDERLINE: String = style::<cap::ExitUnderlineMode>();
}

/// Gets the ANSI code which sets the given style `T`.
fn style<'a, T>() -> String
where
    T: Capability<'a> + AsRef<[u8]>,
{
    expand0::<'a, T>().unwrap_or_else(|| String::new())
}

/// Shortcut function for the `style()` function.
fn expand0<'a, T>() -> Option<String>
where
    T: Capability<'a> + AsRef<[u8]>,
{
    let info = (*TERMINFO).as_ref()?;
    let e = expand!(info.get::<T>()?.as_ref()).ok()?;
    let s = std::str::from_utf8(&e).ok()?;
    Some(s.to_owned())
}
