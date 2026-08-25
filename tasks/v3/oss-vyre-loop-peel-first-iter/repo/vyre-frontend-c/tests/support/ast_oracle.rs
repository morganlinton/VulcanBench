//! S0  -  AST equivalence oracle support.
//!
//! Two ground truths drive every per-feature parser test:
//!
//! 1. **clang** (subprocess `clang -Xclang -ast-dump=json -fsyntax-only -x c
//!    <file>`). Parses the JSON, filters to nodes whose source location lives
//!    in the user file (not transitively-included headers), and returns a
//!    flat sequence of node kinds.
//! 2. **vyrec** (in-process pipeline via `compile_source`). Reads the typed
//!    VAST section out of the `CompiledObject` and returns a flat sequence
//!    of vyrec-side VAST kind labels.
//!
//! A test asserts the **presence** of expected kinds in either stream.
//! Translation between the two label spaces is intentionally not done here;
//! per-feature tests assert both sides contain whatever they expect, which
//! catches "vyrec emits the right node" regressions without overfitting on
//! a translation table that would rot every time clang renames something.
//!
//! The full structural diff (Phase 2) is open work. This module is the
//! kind-presence layer that unblocks every per-feature ticket.
//!
//! `clang` not being on `PATH` is a release-host configuration failure.
//! Parser parity tests must fail loudly rather than silently dropping the
//! external oracle.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

mod facts;
mod source;
mod vast;
mod walkers_basic;
mod walkers_semantic;

pub(crate) use facts::{
    ClangAstStructureFact, ClangDeclarationFact, ClangSymbolScopeFact, ClangTypeFact,
};
pub(crate) use vast::{assert_kinds_contain, vast_kind_label, vyrec_user_kinds};

use source::canonical_path;
use walkers_basic::{walk_clang_declarations, walk_clang_nodes};
use walkers_semantic::{
    walk_clang_structure, walk_clang_symbol_scope_facts, walk_clang_type_facts,
};

pub(crate) fn clang_user_kinds(c_file: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("clang")
        .args(["-Xclang", "-ast-dump=json", "-fsyntax-only", "-x", "c"])
        .arg(c_file)
        .output()
        .map_err(|e| format!("clang invocation failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("clang exited {}: {}", output.status, stderr.trim()));
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("clang produced non-JSON ast dump: {e}"))?;
    let target = canonical_path(c_file);
    let mut kinds = Vec::new();
    let mut sticky_file: Option<String> = None;
    walk_clang_nodes(&json, &target, &mut sticky_file, &mut kinds);
    Ok(kinds)
}

/// Convenience wrapper for tests that require clang as an external oracle.
pub(crate) fn clang_user_kinds_required(c_file: &Path) -> Vec<String> {
    match clang_user_kinds(c_file) {
        Ok(k) => k,
        Err(why) => panic!(
            "ast_oracle: clang oracle failed for {}: {why}. Fix: install clang or repair clang -Xclang -ast-dump=json parity support.",
            c_file.display()
        ),
    }
}

/// Run clang and return declaration facts whose source location is in the requested user file.
pub(crate) fn clang_user_declarations(c_file: &Path) -> Result<Vec<ClangDeclarationFact>, String> {
    let output = Command::new("clang")
        .args(["-Xclang", "-ast-dump=json", "-fsyntax-only", "-x", "c"])
        .arg(c_file)
        .output()
        .map_err(|e| format!("clang invocation failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("clang exited {}: {}", output.status, stderr.trim()));
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("clang produced non-JSON ast dump: {e}"))?;
    let target = canonical_path(c_file);
    let mut declarations = Vec::new();
    let mut sticky_file: Option<String> = None;
    walk_clang_declarations(&json, &target, &mut sticky_file, &mut declarations);
    Ok(declarations)
}

/// Convenience wrapper for tests that require clang declaration facts.
pub(crate) fn clang_user_declarations_required(c_file: &Path) -> Vec<ClangDeclarationFact> {
    match clang_user_declarations(c_file) {
        Ok(declarations) => declarations,
        Err(why) => panic!(
            "ast_oracle: clang declaration oracle failed for {}: {why}. Fix: install clang or repair clang -Xclang -ast-dump=json declaration extraction.",
            c_file.display()
        ),
    }
}

/// Run clang and return statement/expression structure facts whose source location is in the
/// requested user file.
pub(crate) fn clang_user_structure(c_file: &Path) -> Result<Vec<ClangAstStructureFact>, String> {
    let output = Command::new("clang")
        .args(["-Xclang", "-ast-dump=json", "-fsyntax-only", "-x", "c"])
        .arg(c_file)
        .output()
        .map_err(|e| format!("clang invocation failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("clang exited {}: {}", output.status, stderr.trim()));
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("clang produced non-JSON ast dump: {e}"))?;
    let target = canonical_path(c_file);
    let mut structure = Vec::new();
    let mut sticky_file: Option<String> = None;
    let mut sticky_line: Option<u32> = None;
    walk_clang_structure(
        &json,
        &target,
        &mut sticky_file,
        &mut sticky_line,
        &mut structure,
    );
    Ok(structure)
}

/// Convenience wrapper for tests that require clang statement/expression facts.
pub(crate) fn clang_user_structure_required(c_file: &Path) -> Vec<ClangAstStructureFact> {
    match clang_user_structure(c_file) {
        Ok(structure) => structure,
        Err(why) => panic!(
            "ast_oracle: clang structure oracle failed for {}: {why}. Fix: install clang or repair clang -Xclang -ast-dump=json structure extraction.",
            c_file.display()
        ),
    }
}

/// Run clang and return type facts whose owning AST node location is in the requested user file.
pub(crate) fn clang_user_type_facts(c_file: &Path) -> Result<Vec<ClangTypeFact>, String> {
    let output = Command::new("clang")
        .args(["-Xclang", "-ast-dump=json", "-fsyntax-only", "-x", "c"])
        .arg(c_file)
        .output()
        .map_err(|e| format!("clang invocation failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("clang exited {}: {}", output.status, stderr.trim()));
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("clang produced non-JSON ast dump: {e}"))?;
    let target = canonical_path(c_file);
    let mut facts = Vec::new();
    let mut sticky_file: Option<String> = None;
    let mut sticky_line: Option<u32> = None;
    walk_clang_type_facts(
        &json,
        &target,
        &mut sticky_file,
        &mut sticky_line,
        &mut facts,
    );
    Ok(facts)
}

/// Convenience wrapper for tests that require clang type facts.
pub(crate) fn clang_user_type_facts_required(c_file: &Path) -> Vec<ClangTypeFact> {
    match clang_user_type_facts(c_file) {
        Ok(facts) => facts,
        Err(why) => panic!(
            "ast_oracle: clang type oracle failed for {}: {why}. Fix: install clang or repair clang -Xclang -ast-dump=json type extraction.",
            c_file.display()
        ),
    }
}

/// Run clang and return symbol/scope facts whose declaration location is in the requested user
/// file.
pub(crate) fn clang_user_symbol_scope_facts(
    c_file: &Path,
) -> Result<Vec<ClangSymbolScopeFact>, String> {
    let output = Command::new("clang")
        .args(["-Xclang", "-ast-dump=json", "-fsyntax-only", "-x", "c"])
        .arg(c_file)
        .output()
        .map_err(|e| format!("clang invocation failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("clang exited {}: {}", output.status, stderr.trim()));
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("clang produced non-JSON ast dump: {e}"))?;
    let target = canonical_path(c_file);
    let mut facts = Vec::new();
    let mut sticky_file: Option<String> = None;
    let mut sticky_line: Option<u32> = None;
    let mut owner_stack = Vec::new();
    walk_clang_symbol_scope_facts(
        &json,
        &target,
        &mut sticky_file,
        &mut sticky_line,
        &mut owner_stack,
        &mut facts,
    );
    Ok(facts)
}

/// Convenience wrapper for tests that require clang symbol/scope facts.
pub(crate) fn clang_user_symbol_scope_facts_required(c_file: &Path) -> Vec<ClangSymbolScopeFact> {
    match clang_user_symbol_scope_facts(c_file) {
        Ok(facts) => facts,
        Err(why) => panic!(
            "ast_oracle: clang symbol/scope oracle failed for {}: {why}. Fix: install clang or repair clang -Xclang -ast-dump=json symbol/scope extraction.",
            c_file.display()
        ),
    }
}
