use std::path::{Path, PathBuf};

use serde_json::Value;

pub(super) fn canonical_path(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Walk clang's `-ast-dump=json` tree and collect kinds for nodes whose
/// `loc.file` (or the inherited `loc.file` from the most recent ancestor that
/// supplied one) matches `target`.
///
/// clang's JSON dump is a tree of objects with shape:
/// ```text
/// { "kind": "FunctionDecl", "loc": {"file":"…","line":N,"col":M}, "inner":[…] }
/// ```
/// The `loc.file` field is omitted on every node that shares the previous
/// node's file  -  `sticky_file` carries that inheritance.

pub(super) fn declaration_location(
    obj: &serde_json::Map<String, Value>,
    sticky_file: &str,
) -> (String, Option<u32>, Option<u32>) {
    let loc_obj = obj.get("loc").and_then(|loc| loc.as_object());
    let range_begin = obj
        .get("range")
        .and_then(|range| range.as_object())
        .and_then(|range| range.get("begin"))
        .and_then(|begin| begin.as_object());
    let file = loc_obj
        .and_then(|loc| loc.get("file"))
        .or_else(|| range_begin.and_then(|begin| begin.get("file")))
        .and_then(|v| v.as_str())
        .unwrap_or(sticky_file)
        .to_string();
    let line = loc_obj
        .and_then(|loc| loc.get("line"))
        .or_else(|| range_begin.and_then(|begin| begin.get("line")))
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok());
    let column = loc_obj
        .and_then(|loc| loc.get("col"))
        .or_else(|| range_begin.and_then(|begin| begin.get("col")))
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok());
    (file, line, column)
}

pub(super) fn update_sticky_source(
    obj: &serde_json::Map<String, Value>,
    sticky_file: &mut Option<String>,
    sticky_line: &mut Option<u32>,
) {
    let loc_file = obj
        .get("loc")
        .and_then(|loc| loc.as_object())
        .and_then(|loc_obj| loc_obj.get("file"))
        .and_then(|v| v.as_str());
    let range_begin_file = obj
        .get("range")
        .and_then(|range| range.as_object())
        .and_then(|range| range.get("begin"))
        .and_then(|begin| begin.as_object())
        .and_then(|begin| begin.get("file"))
        .and_then(|v| v.as_str());
    if let Some(file) = loc_file.or(range_begin_file) {
        *sticky_file = Some(file.to_string());
    }
    let explicit_line = obj
        .get("loc")
        .and_then(|loc| loc.as_object())
        .and_then(|loc| loc.get("line"))
        .or_else(|| {
            obj.get("range")
                .and_then(|range| range.as_object())
                .and_then(|range| range.get("begin"))
                .and_then(|begin| begin.as_object())
                .and_then(|begin| begin.get("line"))
        })
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok());
    if explicit_line.is_some() {
        *sticky_line = explicit_line;
    }
}

pub(super) fn paths_match(loc_file: &str, target: &Path) -> bool {
    let loc_canonical = canonical_path(Path::new(loc_file));
    loc_canonical == target
}
