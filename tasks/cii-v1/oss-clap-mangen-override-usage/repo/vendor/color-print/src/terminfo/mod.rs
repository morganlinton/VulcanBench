mod color;
mod style;

pub use color::*;
pub use style::*;

use lazy_static::lazy_static;
use terminfo_crate::Database;

lazy_static! {
    /// The terminfo database.
    static ref TERMINFO: Option<Database> = Database::from_env().ok();
}
