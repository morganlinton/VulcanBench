//! JSON/TOML/Markdown semantic inspection for release completion audit evidence.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::paths::{markdown_line_is_release_rule_text, read_text_bounded};

include!("part1.rs");
include!("part2.rs");
include!("part3.rs");
include!("part4.rs");
include!("part5.rs");
include!("part6.rs");
include!("part7.rs");
include!("part8.rs");
include!("part9.rs");
include!("part10.rs");
include!("part11.rs");
include!("part12.rs");
include!("part13.rs");
