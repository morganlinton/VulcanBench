use std::path::Path;

use serde_json::Value;

use super::facts::{ClangAstStructureFact, ClangSymbolScopeFact, ClangTypeFact};
use super::source::{declaration_location, paths_match, update_sticky_source};

pub(super) fn walk_clang_symbol_scope_facts(
    node: &Value,
    target: &Path,
    sticky_file: &mut Option<String>,
    sticky_line: &mut Option<u32>,
    owner_stack: &mut Vec<(String, String)>,
    facts: &mut Vec<ClangSymbolScopeFact>,
) {
    let Some(obj) = node.as_object() else { return };
    let kind = obj.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    update_sticky_source(obj, sticky_file, sticky_line);
    let in_user_file = sticky_file
        .as_deref()
        .map(|f| paths_match(f, target))
        .unwrap_or_else(|| kind == "TranslationUnitDecl");
    let mut loc = declaration_location(obj, sticky_file.as_deref().unwrap_or_default());
    if loc.1.is_none() {
        loc.1 = *sticky_line;
    }
    let name = obj.get("name").and_then(|v| v.as_str());
    if in_user_file
        && kind.ends_with("Decl")
        && kind != "TranslationUnitDecl"
        && (loc.1.is_some() || loc.2.is_some())
        && name.is_some()
    {
        let storage_class = obj
            .get("storageClass")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
        let (owner_kind, owner_name) = owner_stack
            .last()
            .map(|(owner_kind, owner_name)| (Some(owner_kind.clone()), Some(owner_name.clone())))
            .unwrap_or((None, None));
        let scope_kind = infer_scope_kind(owner_kind.as_deref(), kind);
        facts.push(ClangSymbolScopeFact {
            kind: kind.to_string(),
            name: name.unwrap_or_default().to_string(),
            storage_class: storage_class.clone(),
            previous_decl: obj
                .get("previousDecl")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned),
            owner_kind,
            owner_name,
            scope_kind: scope_kind.to_string(),
            linkage: infer_linkage(kind, storage_class.as_deref(), scope_kind).to_string(),
            visibility: infer_visibility(storage_class.as_deref(), scope_kind).to_string(),
            file: loc.0,
            line: loc.1,
            column: loc.2,
        });
    }
    let push_owner = kind.ends_with("Decl")
        && name.is_some()
        && matches!(kind, "FunctionDecl" | "RecordDecl" | "EnumDecl");
    if push_owner {
        owner_stack.push((kind.to_string(), name.unwrap_or_default().to_string()));
    }
    if let Some(inner) = obj.get("inner").and_then(|v| v.as_array()) {
        let parent_sticky = sticky_file.clone();
        let parent_line = *sticky_line;
        for child in inner {
            walk_clang_symbol_scope_facts(
                child,
                target,
                sticky_file,
                sticky_line,
                owner_stack,
                facts,
            );
        }
        *sticky_file = parent_sticky;
        *sticky_line = parent_line;
    }
    if push_owner {
        owner_stack.pop();
    }
}

fn infer_scope_kind(owner_kind: Option<&str>, declaration_kind: &str) -> &'static str {
    match owner_kind {
        Some("FunctionDecl") => "function",
        Some("RecordDecl") => "aggregate",
        Some("EnumDecl") => "enum",
        Some(_) => "nested",
        None if declaration_kind == "ParmVarDecl" => "prototype",
        None => "file",
    }
}

fn infer_linkage(kind: &str, storage_class: Option<&str>, scope_kind: &str) -> &'static str {
    if scope_kind != "file" {
        return "none";
    }
    match storage_class {
        Some("static") => "internal",
        Some("extern") => "external",
        _ if matches!(kind, "FunctionDecl" | "VarDecl") => "external",
        _ => "none",
    }
}

fn infer_visibility(storage_class: Option<&str>, scope_kind: &str) -> &'static str {
    match (scope_kind, storage_class) {
        ("file", Some("static")) => "translation-unit",
        ("file", _) => "external",
        ("function", _) => "function-local",
        ("aggregate", _) => "aggregate-member",
        ("enum", _) => "enum-member",
        _ => "nested",
    }
}

pub(super) fn walk_clang_type_facts(
    node: &Value,
    target: &Path,
    sticky_file: &mut Option<String>,
    sticky_line: &mut Option<u32>,
    facts: &mut Vec<ClangTypeFact>,
) {
    let Some(obj) = node.as_object() else { return };
    let kind = obj.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    update_sticky_source(obj, sticky_file, sticky_line);
    let in_user_file = sticky_file
        .as_deref()
        .map(|f| paths_match(f, target))
        .unwrap_or_else(|| kind == "TranslationUnitDecl");
    let mut loc = declaration_location(obj, sticky_file.as_deref().unwrap_or_default());
    if loc.1.is_none() {
        loc.1 = *sticky_line;
    }
    if in_user_file && (loc.1.is_some() || loc.2.is_some()) {
        if let Some(type_obj) = obj.get("type").and_then(|v| v.as_object()) {
            if let Some(qual_type) = type_obj.get("qualType").and_then(|v| v.as_str()) {
                let desugared_qual_type = type_obj
                    .get("desugaredQualType")
                    .and_then(|v| v.as_str())
                    .map(ToOwned::to_owned);
                facts.push(type_fact_from_parts(
                    kind,
                    obj.get("name").and_then(|v| v.as_str()),
                    qual_type,
                    desugared_qual_type,
                    type_obj.contains_key("typeAliasDeclId"),
                    loc,
                ));
            }
        } else if let Some(tag_fact) = tag_type_fact_from_node(kind, obj, loc.clone()) {
            facts.push(tag_fact);
        }
    }
    if let Some(inner) = obj.get("inner").and_then(|v| v.as_array()) {
        let parent_sticky = sticky_file.clone();
        let parent_line = *sticky_line;
        for child in inner {
            walk_clang_type_facts(child, target, sticky_file, sticky_line, facts);
        }
        *sticky_file = parent_sticky;
        *sticky_line = parent_line;
    }
}

fn type_fact_from_parts(
    owner_kind: &str,
    owner_name: Option<&str>,
    qual_type: &str,
    desugared_qual_type: Option<String>,
    has_type_alias_id: bool,
    loc: (String, Option<u32>, Option<u32>),
) -> ClangTypeFact {
    let lower = qual_type.to_ascii_lowercase();
    let is_function =
        owner_kind == "FunctionDecl" || (!lower.contains("typeof") && qual_type.contains(" ("));
    let tag_kind = ["struct", "union", "enum"]
        .into_iter()
        .find(|prefix| lower.starts_with(&format!("{prefix} ")))
        .map(ToOwned::to_owned);
    ClangTypeFact {
        owner_kind: owner_kind.to_string(),
        owner_name: owner_name.map(ToOwned::to_owned),
        qual_type: qual_type.to_string(),
        desugared_qual_type,
        uses_typedef: has_type_alias_id || owner_kind == "TypedefDecl",
        uses_typeof: lower.contains("typeof"),
        is_const: lower.split_whitespace().any(|part| part == "const"),
        is_volatile: lower.split_whitespace().any(|part| part == "volatile"),
        is_restrict: lower.split_whitespace().any(|part| part == "restrict"),
        pointer_depth: qual_type.chars().filter(|c| *c == '*').count() as u32,
        array_depth: qual_type.chars().filter(|c| *c == '[').count() as u32,
        is_function,
        tag_kind,
        file: loc.0,
        line: loc.1,
        column: loc.2,
    }
}

fn tag_type_fact_from_node(
    owner_kind: &str,
    obj: &serde_json::Map<String, Value>,
    loc: (String, Option<u32>, Option<u32>),
) -> Option<ClangTypeFact> {
    let tag_kind = match owner_kind {
        "EnumDecl" => "enum",
        "RecordDecl" => obj
            .get("tagUsed")
            .and_then(|v| v.as_str())
            .unwrap_or("struct"),
        _ => return None,
    };
    let name = obj.get("name").and_then(|v| v.as_str())?;
    Some(ClangTypeFact {
        owner_kind: owner_kind.to_string(),
        owner_name: Some(name.to_string()),
        qual_type: format!("{tag_kind} {name}"),
        desugared_qual_type: None,
        uses_typedef: false,
        uses_typeof: false,
        is_const: false,
        is_volatile: false,
        is_restrict: false,
        pointer_depth: 0,
        array_depth: 0,
        is_function: false,
        tag_kind: Some(tag_kind.to_string()),
        file: loc.0,
        line: loc.1,
        column: loc.2,
    })
}

pub(super) fn walk_clang_structure(
    node: &Value,
    target: &Path,
    sticky_file: &mut Option<String>,
    sticky_line: &mut Option<u32>,
    structure: &mut Vec<ClangAstStructureFact>,
) {
    let Some(obj) = node.as_object() else { return };
    let kind = obj.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    update_sticky_source(obj, sticky_file, sticky_line);
    let in_user_file = sticky_file
        .as_deref()
        .map(|f| paths_match(f, target))
        .unwrap_or_else(|| kind == "TranslationUnitDecl");
    let mut loc = declaration_location(obj, sticky_file.as_deref().unwrap_or_default());
    if loc.1.is_none() {
        loc.1 = *sticky_line;
    }
    if in_user_file && is_statement_or_expression_kind(kind) && (loc.1.is_some() || loc.2.is_some())
    {
        structure.push(ClangAstStructureFact {
            kind: kind.to_string(),
            qual_type: obj
                .get("type")
                .and_then(|v| v.as_object())
                .and_then(|type_obj| type_obj.get("qualType"))
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned),
            referenced_decl_name: obj
                .get("referencedDecl")
                .and_then(|v| v.as_object())
                .and_then(|decl| decl.get("name"))
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned),
            file: loc.0,
            line: loc.1,
            column: loc.2,
        });
    }
    if let Some(inner) = obj.get("inner").and_then(|v| v.as_array()) {
        let parent_sticky = sticky_file.clone();
        let parent_line = *sticky_line;
        for child in inner {
            walk_clang_structure(child, target, sticky_file, sticky_line, structure);
        }
        *sticky_file = parent_sticky;
        *sticky_line = parent_line;
    }
}

fn is_statement_or_expression_kind(kind: &str) -> bool {
    kind.ends_with("Stmt")
        || kind.ends_with("Expr")
        || kind.ends_with("Literal")
        || kind.ends_with("Operator")
        || kind == "CompoundAssignOperator"
        || kind == "UnaryOperator"
        || kind == "ArraySubscriptExpr"
}
