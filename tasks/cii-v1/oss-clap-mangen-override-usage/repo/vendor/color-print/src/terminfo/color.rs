//! Lazy constants representing the ANSI codes for setting terminal colors.
//!
//! Terminfo is used internally to guess the right codes.

use lazy_static::lazy_static;
use terminfo_crate::{capability as cap, expand, Capability};

use crate::terminfo::TERMINFO;

#[rustfmt::skip]
lazy_static! {
    pub static ref BLACK: String = foreground(0);
    pub static ref RED: String = foreground(1);
    pub static ref GREEN: String = foreground(2);
    pub static ref YELLOW: String = foreground(3);
    pub static ref BLUE: String = foreground(4);
    pub static ref MAGENTA: String = foreground(5);
    pub static ref CYAN: String = foreground(6);
    pub static ref WHITE: String = foreground(7);

    pub static ref BRIGHT_BLACK: String = foreground(8);
    pub static ref BRIGHT_RED: String = foreground(9);
    pub static ref BRIGHT_GREEN: String = foreground(10);
    pub static ref BRIGHT_YELLOW: String = foreground(11);
    pub static ref BRIGHT_BLUE: String = foreground(12);
    pub static ref BRIGHT_MAGENTA: String = foreground(13);
    pub static ref BRIGHT_CYAN: String = foreground(14);
    pub static ref BRIGHT_WHITE: String = foreground(15);

    pub static ref BG_BLACK: String = background(0);
    pub static ref BG_RED: String = background(1);
    pub static ref BG_GREEN: String = background(2);
    pub static ref BG_YELLOW: String = background(3);
    pub static ref BG_BLUE: String = background(4);
    pub static ref BG_MAGENTA: String = background(5);
    pub static ref BG_CYAN: String = background(6);
    pub static ref BG_WHITE: String = background(7);

    pub static ref BG_BRIGHT_BLACK: String = background(8);
    pub static ref BG_BRIGHT_RED: String = background(9);
    pub static ref BG_BRIGHT_GREEN: String = background(10);
    pub static ref BG_BRIGHT_YELLOW: String = background(11);
    pub static ref BG_BRIGHT_BLUE: String = background(12);
    pub static ref BG_BRIGHT_MAGENTA: String = background(13);
    pub static ref BG_BRIGHT_CYAN: String = background(14);
    pub static ref BG_BRIGHT_WHITE: String = background(15);
}

/// Gets the ANSI code which sets the foreground color to the given color (0 to 15 included).
fn foreground(v: u8) -> String {
    #[cfg(debug_assertions)]
    assert!(v < 16);

    expand1_string::<cap::SetAForeground>(v)
}

/// Gets the ANSI code which sets the background color to the given color (0 to 15 included).
fn background(v: u8) -> String {
    #[cfg(debug_assertions)]
    assert!(v < 16);

    expand1_string::<cap::SetABackground>(v)
}

/// Shortcut function for the `foreground()` and `background()` functions.
fn expand1_string<'a, T>(v: u8) -> String
where
    T: Capability<'a> + AsRef<[u8]>,
{
    expand1::<'a, T>(v).unwrap_or_else(|| String::new())
}

/// Shortcut function for the `foreground()` and `background()` functions.
fn expand1<'a, T>(v: u8) -> Option<String>
where
    T: Capability<'a> + AsRef<[u8]>,
{
    let info = (*TERMINFO).as_ref()?;
    let e = expand!(info.get::<T>()?.as_ref(); v).ok()?;
    let s = std::str::from_utf8(&e).ok()?;
    Some(s.to_owned())
}
