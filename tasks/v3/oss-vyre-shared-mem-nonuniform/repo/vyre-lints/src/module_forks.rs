//! Same-name module fork scanner.
//!
//! Exact duplicate hashes miss the common architectural failure mode:
//! two crates keep files with the same domain name and drift independently.
//! This scanner is intentionally root-scoped so callers can compare domain
//! roots such as `vyre-primitives/src/graph` and `vyre-self-substrate/src`
//! without flagging every generic `mod.rs` or `tests.rs` in the workspace.

use crate::{paths::workspace_relative, Violation, ViolationKind};
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const EXEMPT_BASENAMES: &[&str] = &[
    "lib.rs", "main.rs", "mod.rs", "tests.rs", "error.rs", "types.rs",
];

pub fn scan_roots(roots: &[&Path]) -> Result<Vec<Violation>> {
    let mut by_basename: BTreeMap<String, Vec<ModuleHit>> = BTreeMap::new();
    for (root_index, root) in roots.iter().enumerate() {
        scan_root(root_index, root, &mut by_basename)?;
    }

    let mut violations = Vec::new();
    for (basename, mut paths) in by_basename {
        let distinct_roots = paths
            .iter()
            .map(|hit| hit.root_index)
            .collect::<BTreeSet<_>>();
        if distinct_roots.len() <= 1 {
            continue;
        }
        paths.sort_by(|a, b| a.path.cmp(&b.path));
        let joined = paths
            .iter()
            .map(|hit| hit.path.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let shared = shared_symbol_summary(&paths);
        for hit in paths {
            violations.push(Violation {
                file: hit.path,
                line: 1,
                column: 1,
                kind: ViolationKind::ModuleFork,
                message: format!(
                    "module filename `{basename}` appears in multiple scanned roots: {joined}; {shared}. Fix: pick one authority module and make the other crate a thin adapter or give the adapter a domain-specific name."
                ),
            });
        }
    }
    violations.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    Ok(violations)
}

#[derive(Clone, Debug)]
struct ModuleHit {
    root_index: usize,
    path: String,
    symbols: BTreeSet<String>,
}

fn scan_root(
    root_index: usize,
    root: &Path,
    by_basename: &mut BTreeMap<String, Vec<ModuleHit>>,
) -> Result<()> {
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let basename = path
            .file_name()
            .and_then(|name| name.to_str())
            .with_context(|| format!("read module filename {}", path.display()))?;
        if EXEMPT_BASENAMES.contains(&basename) {
            continue;
        }
        by_basename
            .entry(basename.to_string())
            .or_default()
            .push(ModuleHit {
                root_index,
                path: workspace_relative(path),
                symbols: function_symbols(&crate::read_source_bounded(path)?),
            });
    }
    Ok(())
}

fn shared_symbol_summary(paths: &[ModuleHit]) -> String {
    let mut roots_by_symbol: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    for hit in paths {
        for symbol in &hit.symbols {
            roots_by_symbol
                .entry(symbol.as_str())
                .or_default()
                .insert(hit.root_index);
        }
    }
    let shared = roots_by_symbol
        .into_iter()
        .filter_map(|(symbol, roots)| (roots.len() > 1).then_some(symbol))
        .take(8)
        .collect::<Vec<_>>();
    if shared.is_empty() {
        "shared Rust symbols: none detected".to_string()
    } else {
        format!("shared Rust symbols: {}", shared.join(", "))
    }
}

fn function_symbols(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(function_symbol)
        .map(str::to_string)
        .collect()
}

fn function_symbol(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    if rest.starts_with("//") {
        return None;
    }
    if let Some(after_pub) = rest.strip_prefix("pub ") {
        rest = after_pub.trim_start();
    } else if let Some(after_pub_scope) = rest.strip_prefix("pub(") {
        let close = after_pub_scope.find(')')?;
        rest = after_pub_scope[close + 1..].trim_start();
    }
    loop {
        if let Some(after) = rest.strip_prefix("async ") {
            rest = after.trim_start();
        } else if let Some(after) = rest.strip_prefix("const ") {
            rest = after.trim_start();
        } else if let Some(after) = rest.strip_prefix("unsafe ") {
            rest = after.trim_start();
        } else if let Some(after) = rest.strip_prefix("extern ") {
            rest = after.trim_start();
            if let Some(after_abi) = rest.strip_prefix('"') {
                let close = after_abi.find('"')?;
                rest = after_abi[close + 1..].trim_start();
            }
        } else {
            break;
        }
    }
    let after_fn = rest.strip_prefix("fn ")?;
    let end = after_fn
        .find(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .unwrap_or(after_fn.len());
    (end > 0).then_some(&after_fn[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exempt_generic_module_names() {
        assert!(EXEMPT_BASENAMES.contains(&"mod.rs"));
        assert!(EXEMPT_BASENAMES.contains(&"tests.rs"));
    }

    #[test]
    fn extracts_function_symbols_without_comments_or_visibility_noise() {
        let source = r#"
            pub fn public_api() {}
            pub(crate) const fn scoped_const() {}
            async fn async_local() {}
            unsafe extern "C" fn ffi_entry() {}
            // fn commented_out() {}
            let not_a_function = 1;
        "#;
        let symbols = function_symbols(source);
        assert!(symbols.contains("public_api"));
        assert!(symbols.contains("scoped_const"));
        assert!(symbols.contains("async_local"));
        assert!(symbols.contains("ffi_entry"));
        assert!(!symbols.contains("commented_out"));
    }
}
