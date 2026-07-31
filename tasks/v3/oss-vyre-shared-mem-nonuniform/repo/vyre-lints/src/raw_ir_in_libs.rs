//! `raw_ir_in_libs`: forbid raw `Node::*` / `Expr::*` construction in
//! `vyre-libs/src/**`. Construction sites  -  struct literals, tuple
//! constructors, and associated-function calls  -  are flagged. Pattern
//! matching against the same enum variants is allowed (it's read, not
//! construct). Test modules are allowed (`#[cfg(test)]` or `mod tests`).

use crate::allowlist::Allowlist;
use crate::{Violation, ViolationKind};
use anyhow::{Context, Result};
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;

const FORBIDDEN_TYPES: &[&str] = &["Node", "Expr"];

pub fn scan_tree(root: &Path, allow: &Allowlist) -> Result<Vec<Violation>> {
    let mut all = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let workspace_rel = workspace_relative(path);
        if allow.contains(&workspace_rel) {
            continue;
        }
        all.extend(scan_file(path, &workspace_rel)?);
    }
    Ok(all)
}

fn workspace_relative(path: &Path) -> String {
    // Drop everything up to and including "vyre-libs/" so the path is
    // workspace-relative regardless of CWD.
    let s = path.to_string_lossy();
    if let Some(idx) = s.find("vyre-libs/") {
        s[idx..].to_string()
    } else {
        path.to_string_lossy().to_string()
    }
}

fn scan_file(path: &Path, workspace_rel: &str) -> Result<Vec<Violation>> {
    let source = crate::read_source_bounded(path)?;
    let file = syn::parse_file(&source).with_context(|| format!("parse {}", path.display()))?;

    let mut visitor = LegoBlockVisitor {
        file: workspace_rel.to_string(),
        in_test_depth: 0,
        violations: Vec::new(),
    };
    visitor.visit_file(&file);
    Ok(visitor.violations)
}

struct LegoBlockVisitor {
    file: String,
    in_test_depth: usize,
    violations: Vec<Violation>,
}

impl LegoBlockVisitor {
    fn record(&mut self, span: proc_macro2::Span, ty: &str, what: &str) {
        if self.in_test_depth > 0 {
            return;
        }
        let kind = match ty {
            "Node" => ViolationKind::RawNodeConstruction,
            "Expr" => ViolationKind::RawExprConstruction,
            _ => return,
        };
        let start = span.start();
        self.violations.push(Violation {
            file: self.file.clone(),
            line: start.line as u32,
            column: start.column as u32,
            kind,
            message: format!("raw {ty}::{what} construction in vyre-libs"),
        });
    }
}

fn last_segment(path: &syn::Path) -> Option<String> {
    path.segments.last().map(|s| s.ident.to_string())
}

/// `Node::Foo` or `Expr::Bar`  -  exactly two segments where the first
/// is a forbidden type. Sub-paths like `vyre::ir::Node::Foo` also count.
fn forbidden_path(path: &syn::Path) -> Option<(String, String)> {
    if path.segments.len() < 2 {
        return None;
    }
    // The forbidden type must be the SECOND-TO-LAST segment.
    let last = last_segment(path)?;
    let penultimate_ident = path.segments[path.segments.len() - 2].ident.to_string();
    if FORBIDDEN_TYPES.contains(&penultimate_ident.as_str()) {
        Some((penultimate_ident, last))
    } else {
        None
    }
}

fn is_test_attr(attr: &syn::Attribute) -> bool {
    if attr.path().is_ident("test") {
        return true;
    }
    if attr.path().is_ident("cfg") {
        let mut found = false;
        if attr
            .parse_nested_meta(|m| {
                if m.path.is_ident("test") {
                    found = true;
                }
                Ok(())
            })
            .is_err()
        {
            return false;
        }
        return found;
    }
    if attr.path().is_ident("cfg_attr") {
        let mut found = false;
        if attr
            .parse_nested_meta(|m| {
                if m.path.is_ident("test") {
                    found = true;
                }
                Ok(())
            })
            .is_err()
        {
            return false;
        }
        return found;
    }
    false
}

fn module_is_test(item_mod: &syn::ItemMod) -> bool {
    if item_mod.attrs.iter().any(is_test_attr) {
        return true;
    }
    item_mod.ident == "tests" || item_mod.ident == "test"
}

impl<'ast> Visit<'ast> for LegoBlockVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let test_mod = module_is_test(node);
        if test_mod {
            self.in_test_depth += 1;
        }
        syn::visit::visit_item_mod(self, node);
        if test_mod {
            self.in_test_depth -= 1;
        }
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let is_test = node.attrs.iter().any(is_test_attr);
        if is_test {
            self.in_test_depth += 1;
        }
        syn::visit::visit_item_fn(self, node);
        if is_test {
            self.in_test_depth -= 1;
        }
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if let Some((ty, variant)) = forbidden_path(&node.path) {
            self.record(node.path.span(), &ty, &variant);
        }
        syn::visit::visit_expr_struct(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(p) = &*node.func {
            if let Some((ty, what)) = forbidden_path(&p.path) {
                self.record(p.path.span(), &ty, &what);
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_path(&mut self, _node: &'ast syn::ExprPath) {
        // Tuple-variant constructors used as values (e.g. as a function
        // pointer): `let f = Node::Store;`  -  also a construction site.
        // But `Node::Store` standalone is rare; we rely on visit_expr_call
        // and visit_expr_struct for the common cases. Skip here to avoid
        // false positives on type-paths used in turbofish or trait calls.
    }
}

