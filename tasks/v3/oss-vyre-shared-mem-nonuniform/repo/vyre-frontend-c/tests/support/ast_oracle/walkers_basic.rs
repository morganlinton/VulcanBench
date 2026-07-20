use std::path::Path;

use serde_json::Value;

use super::facts::ClangDeclarationFact;
use super::source::{declaration_location, paths_match};

pub(super) fn walk_clang_nodes(
    node: &Value,
    target: &Path,
    sticky_file: &mut Option<String>,
    kinds: &mut Vec<String>,
) {
    let Some(obj) = node.as_object() else { return };
    let kind = obj.get("kind").and_then(|v| v.as_str()).unwrap_or("");
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
    let in_user_file = sticky_file
        .as_deref()
        .map(|f| paths_match(f, target))
        .unwrap_or_else(|| {
            // The TranslationUnitDecl root has no file; we don't count it but
            // we do recurse so its children pick up file inheritance.
            kind == "TranslationUnitDecl"
        });
    let count_self = match kind {
        "" | "TranslationUnitDecl" => false,
        _ => in_user_file,
    };
    if count_self {
        kinds.push(kind.to_string());
    }
    if let Some(inner) = obj.get("inner").and_then(|v| v.as_array()) {
        // Each child resets its own sticky file inheritance from the parent's
        // current value. Carry the parent's sticky_file as the starting point.
        let parent_sticky = sticky_file.clone();
        for child in inner {
            *sticky_file = parent_sticky.clone();
            walk_clang_nodes(child, target, sticky_file, kinds);
        }
        *sticky_file = parent_sticky;
    }
}

pub(super) fn walk_clang_declarations(
    node: &Value,
    target: &Path,
    sticky_file: &mut Option<String>,
    declarations: &mut Vec<ClangDeclarationFact>,
) {
    let Some(obj) = node.as_object() else { return };
    let kind = obj.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let loc_file = obj
        .get("loc")
        .and_then(|loc| loc.as_object())
        .and_then(|loc_obj| loc_obj.get("file"))
        .and_then(|v| v.as_str());
    if let Some(file) = loc_file {
        *sticky_file = Some(file.to_string());
    }
    let in_user_file = sticky_file
        .as_deref()
        .map(|f| paths_match(f, target))
        .unwrap_or_else(|| kind == "TranslationUnitDecl");
    let loc = declaration_location(obj, sticky_file.as_deref().unwrap_or_default());
    if in_user_file
        && kind.ends_with("Decl")
        && kind != "TranslationUnitDecl"
        && (loc.1.is_some() || loc.2.is_some())
    {
        declarations.push(ClangDeclarationFact {
            kind: kind.to_string(),
            name: obj
                .get("name")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned),
            qual_type: obj
                .get("type")
                .and_then(|v| v.as_object())
                .and_then(|type_obj| type_obj.get("qualType"))
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned),
            file: loc.0,
            line: loc.1,
            column: loc.2,
        });
    }
    if let Some(inner) = obj.get("inner").and_then(|v| v.as_array()) {
        let parent_sticky = sticky_file.clone();
        for child in inner {
            walk_clang_declarations(child, target, sticky_file, declarations);
        }
        *sticky_file = parent_sticky;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn walk_filters_to_user_file_only() {
        let target = PathBuf::from("/tmp/userfile.c");
        let json = serde_json::json!({
            "kind": "TranslationUnitDecl",
            "inner": [
                { "kind": "TypedefDecl", "loc": {"file": "/usr/include/foo.h"} },
                { "kind": "FunctionDecl",
                  "loc": {"file": "/tmp/userfile.c", "line": 1, "col": 1},
                  "inner": [
                      { "kind": "ParmVarDecl",
                        "loc": {"line": 1, "col": 5}
                      },
                      { "kind": "CompoundStmt",
                        "loc": {"line": 1, "col": 30},
                        "inner": [
                            { "kind": "ReturnStmt", "loc": {"line": 2, "col": 5} }
                        ]
                      }
                  ]
                }
            ]
        });
        let mut kinds = Vec::new();
        let mut sticky = None;
        walk_clang_nodes(&json, &target, &mut sticky, &mut kinds);
        assert!(!kinds.contains(&"TypedefDecl".to_string()));
        assert!(kinds.contains(&"FunctionDecl".to_string()));
        assert!(kinds.contains(&"ParmVarDecl".to_string()));
        assert!(kinds.contains(&"ReturnStmt".to_string()));
    }
}
