//! Shared release-gate check helpers.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::paths::{read_text_bounded, resolve_artifact_path, resolve_manifest_path};
use super::types::Requirement;

include!("part1.rs");
include!("part2.rs");
include!("part3.rs");
include!("part4.rs");
include!("part5.rs");
include!("part6.rs");
include!("part7.rs");
include!("part8.rs");
