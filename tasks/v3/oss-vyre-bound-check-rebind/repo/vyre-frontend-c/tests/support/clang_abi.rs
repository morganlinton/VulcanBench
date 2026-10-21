//! clang ABI/layout oracle support.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// clang record layout fact for a struct or union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangRecordLayout {
    /// `struct` or `union`.
    pub(crate) kind: String,
    /// Tag name.
    pub(crate) name: String,
    /// Size in bytes.
    pub(crate) size_bytes: u64,
    /// Alignment in bytes.
    pub(crate) align_bytes: u64,
    /// Field layout facts.
    pub(crate) fields: Vec<ClangFieldLayout>,
}

/// clang field layout fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangFieldLayout {
    /// Field name.
    pub(crate) name: String,
    /// Field type spelling.
    pub(crate) ty: String,
    /// Field byte offset.
    pub(crate) byte_offset: u64,
    /// Bitfield low bit when this field is a bitfield.
    pub(crate) bit_start: Option<u32>,
    /// Bitfield high bit when this field is a bitfield.
    pub(crate) bit_end: Option<u32>,
}

/// clang enum representation fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangEnumAbiFact {
    /// Enum tag name.
    pub(crate) name: String,
    /// clang type spelling used for enumerators.
    pub(crate) representation: String,
    /// Enumerator values.
    pub(crate) enumerators: Vec<(String, String)>,
}

/// clang function ABI fact before lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClangFunctionAbiFact {
    /// Function name.
    pub(crate) name: String,
    /// clang function type spelling.
    pub(crate) qual_type: String,
    /// Storage class when present.
    pub(crate) storage_class: Option<String>,
}

/// Run clang and return record layouts.
pub(crate) fn clang_record_layouts(c_file: &Path) -> Result<Vec<ClangRecordLayout>, String> {
    let output = Command::new("clang")
        .args(["-cc1", "-fdump-record-layouts-complete", "-fsyntax-only"])
        .arg(c_file)
        .output()
        .map_err(|e| format!("clang record-layout oracle invocation failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "clang record-layout oracle exited {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    Ok(parse_record_layout_dump(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// Run clang and return enum and function ABI facts from AST JSON.
pub(crate) fn clang_enum_and_function_abi(
    c_file: &Path,
) -> Result<(Vec<ClangEnumAbiFact>, Vec<ClangFunctionAbiFact>), String> {
    let output = Command::new("clang")
        .args(["-Xclang", "-ast-dump=json", "-fsyntax-only", "-x", "c"])
        .arg(c_file)
        .output()
        .map_err(|e| format!("clang ABI AST oracle invocation failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "clang ABI AST oracle exited {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("clang produced non-JSON ast dump: {e}"))?;
    let mut enums = Vec::new();
    let mut functions = Vec::new();
    walk_abi_ast(&json, &mut enums, &mut functions);
    Ok((enums, functions))
}

fn parse_record_layout_dump(dump: &str) -> Vec<ClangRecordLayout> {
    let mut layouts = Vec::new();
    let mut current: Option<ClangRecordLayout> = None;
    for line in dump.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("***") {
            continue;
        }
        if let Some((kind, name)) = parse_record_header(trimmed) {
            if let Some(layout) = current.take() {
                layouts.push(layout);
            }
            current = Some(ClangRecordLayout {
                kind,
                name,
                size_bytes: 0,
                align_bytes: 0,
                fields: Vec::new(),
            });
        } else if let Some((size, align)) = parse_size_align(trimmed) {
            if let Some(layout) = current.as_mut() {
                layout.size_bytes = size;
                layout.align_bytes = align;
            }
        } else if let Some(field) = parse_field_layout(trimmed) {
            if let Some(layout) = current.as_mut() {
                layout.fields.push(field);
            }
        }
    }
    if let Some(layout) = current {
        layouts.push(layout);
    }
    layouts
}

fn parse_record_header(line: &str) -> Option<(String, String)> {
    let (_, rhs) = line.split_once('|')?;
    let rhs = rhs.trim();
    for kind in ["struct", "union"] {
        if let Some(name) = rhs.strip_prefix(&format!("{kind} ")) {
            return Some((kind.to_string(), name.trim().to_string()));
        }
    }
    None
}

fn parse_size_align(line: &str) -> Option<(u64, u64)> {
    let payload = line.strip_prefix("| [sizeof=")?.strip_suffix(']')?;
    let (size, align_part) = payload.split_once(", align=")?;
    Some((size.parse().ok()?, align_part.parse().ok()?))
}

fn parse_field_layout(line: &str) -> Option<ClangFieldLayout> {
    let (offset, rhs) = line.split_once('|')?;
    let rhs = rhs.trim();
    if rhs.starts_with("struct ") || rhs.starts_with("union ") || rhs.starts_with('[') {
        return None;
    }
    let mut words = rhs.split_whitespace().collect::<Vec<_>>();
    if words.len() < 2 {
        return None;
    }
    let name = words.pop()?.to_string();
    let ty = words.join(" ");
    let offset = offset.trim();
    let (byte_offset, bit_start, bit_end) = if let Some((byte, bits)) = offset.split_once(':') {
        let (start, end) = bits.split_once('-')?;
        (
            byte.trim().parse().ok()?,
            Some(start.parse().ok()?),
            Some(end.parse().ok()?),
        )
    } else {
        (offset.parse().ok()?, None, None)
    };
    Some(ClangFieldLayout {
        name,
        ty,
        byte_offset,
        bit_start,
        bit_end,
    })
}

fn walk_abi_ast(
    node: &Value,
    enums: &mut Vec<ClangEnumAbiFact>,
    functions: &mut Vec<ClangFunctionAbiFact>,
) {
    let Some(obj) = node.as_object() else { return };
    match obj.get("kind").and_then(|v| v.as_str()) {
        Some("EnumDecl") => {
            if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                let mut enumerators = Vec::new();
                let mut representation = None;
                if let Some(inner) = obj.get("inner").and_then(|v| v.as_array()) {
                    for child in inner {
                        if child.get("kind").and_then(|v| v.as_str()) == Some("EnumConstantDecl") {
                            if let Some(enum_obj) = child.as_object() {
                                if let Some(enum_name) =
                                    enum_obj.get("name").and_then(|v| v.as_str())
                                {
                                    representation = enum_obj
                                        .get("type")
                                        .and_then(|v| v.as_object())
                                        .and_then(|type_obj| type_obj.get("qualType"))
                                        .and_then(|v| v.as_str())
                                        .map(ToOwned::to_owned)
                                        .or(representation);
                                    if let Some(value) = first_constant_expr_value(enum_obj) {
                                        enumerators.push((enum_name.to_string(), value));
                                    }
                                }
                            }
                        }
                    }
                }
                enums.push(ClangEnumAbiFact {
                    name: name.to_string(),
                    representation: representation.unwrap_or_else(|| "int".to_string()),
                    enumerators,
                });
            }
        }
        Some("FunctionDecl") => {
            if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                if let Some(qual_type) = obj
                    .get("type")
                    .and_then(|v| v.as_object())
                    .and_then(|type_obj| type_obj.get("qualType"))
                    .and_then(|v| v.as_str())
                {
                    functions.push(ClangFunctionAbiFact {
                        name: name.to_string(),
                        qual_type: qual_type.to_string(),
                        storage_class: obj
                            .get("storageClass")
                            .and_then(|v| v.as_str())
                            .map(ToOwned::to_owned),
                    });
                }
            }
        }
        _ => {}
    }
    if let Some(inner) = obj.get("inner").and_then(|v| v.as_array()) {
        for child in inner {
            walk_abi_ast(child, enums, functions);
        }
    }
}

fn first_constant_expr_value(obj: &serde_json::Map<String, Value>) -> Option<String> {
    obj.get("inner")
        .and_then(|v| v.as_array())
        .and_then(|inner| {
            inner
                .iter()
                .find(|child| child.get("kind").and_then(|v| v.as_str()) == Some("ConstantExpr"))
        })
        .and_then(|constant| constant.get("value"))
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
}
