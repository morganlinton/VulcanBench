//! Include-acceleration state and evidence for `#pragma once` and guards.

use std::path::{Path, PathBuf};

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use super::{ClassifiedTokens, DirectivePayload};

/// Include acceleration reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncludeAccelerationKind {
    /// `#pragma once` path cache.
    PragmaOnce,
    /// Classic `#ifndef GUARD` / `#define GUARD` cache.
    IncludeGuard,
}

/// Include acceleration evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeAccelerationEvent {
    /// Header path affected by the event.
    pub path: PathBuf,
    /// Acceleration kind.
    pub kind: IncludeAccelerationKind,
    /// Guard macro for include guards; empty for `#pragma once`.
    pub guard_macro: Vec<u8>,
    /// Whether this event skipped a repeated include.
    pub skipped_include: bool,
    /// Whether this event came from GPU directive rows/payloads.
    pub gpu_directive_derived: bool,
}

/// Mutable include acceleration state threaded through a TU preprocess walk.
#[derive(Debug, Default)]
pub(super) struct IncludeAccelerationState {
    pragma_once_paths: HashSet<PathBuf>,
    guard_macros_by_path: HashMap<PathBuf, Vec<u8>>,
}

impl IncludeAccelerationState {
    pub(super) fn observe_file(
        &mut self,
        path: &Path,
        classified: &ClassifiedTokens,
        payloads: &[DirectivePayload],
        events: &mut Vec<IncludeAccelerationEvent>,
    ) {
        if file_has_pragma_once(classified, payloads)
            && self.pragma_once_paths.insert(path.to_path_buf())
        {
            events.push(IncludeAccelerationEvent {
                path: path.to_path_buf(),
                kind: IncludeAccelerationKind::PragmaOnce,
                guard_macro: Vec::new(),
                skipped_include: false,
                gpu_directive_derived: true,
            });
        }
        if let Some(guard) = file_include_guard(payloads) {
            let changed = self.guard_macros_by_path.get(path).map(Vec::as_slice) != Some(guard);
            if changed {
                self.guard_macros_by_path
                    .insert(path.to_path_buf(), guard.to_vec());
                events.push(IncludeAccelerationEvent {
                    path: path.to_path_buf(),
                    kind: IncludeAccelerationKind::IncludeGuard,
                    guard_macro: guard.to_vec(),
                    skipped_include: false,
                    gpu_directive_derived: true,
                });
            }
        }
    }

    pub(super) fn skip_event(
        &self,
        path: &Path,
        macro_index: &HashMap<Vec<u8>, usize>,
    ) -> Option<IncludeAccelerationEvent> {
        if self.pragma_once_paths.contains(path) {
            return Some(IncludeAccelerationEvent {
                path: path.to_path_buf(),
                kind: IncludeAccelerationKind::PragmaOnce,
                guard_macro: Vec::new(),
                skipped_include: true,
                gpu_directive_derived: true,
            });
        }
        let guard = self.guard_macros_by_path.get(path)?;
        if macro_index.contains_key(guard.as_slice()) {
            return Some(IncludeAccelerationEvent {
                path: path.to_path_buf(),
                kind: IncludeAccelerationKind::IncludeGuard,
                guard_macro: guard.clone(),
                skipped_include: true,
                gpu_directive_derived: true,
            });
        }
        None
    }
}

fn file_has_pragma_once(classified: &ClassifiedTokens, payloads: &[DirectivePayload]) -> bool {
    payloads.iter().enumerate().any(|(idx, payload)| {
        matches!(payload, DirectivePayload::Other)
            && classified.directive_kinds.get(idx).copied()
                == Some(crate::parsing::c::lex::tokens::TOK_PP_PRAGMA)
            && directive_row_bytes(classified, idx)
                .map(|row| trim_ascii(row).eq_ignore_ascii_case(b"#pragma once"))
                .unwrap_or(false)
    })
}

fn file_include_guard(payloads: &[DirectivePayload]) -> Option<&[u8]> {
    let mut saw_opening_ifndef = false;
    for payload in payloads
        .iter()
        .filter(|payload| !matches!(payload, DirectivePayload::None))
        .take(32)
    {
        match (saw_opening_ifndef, payload) {
            (false, DirectivePayload::Other) => {}
            (
                false,
                DirectivePayload::Ifdef {
                    value: _,
                    negated: true,
                },
            ) => saw_opening_ifndef = true,
            (true, DirectivePayload::Other) => {}
            (true, DirectivePayload::Define { name, .. }) if !name.is_empty() => {
                return Some(name);
            }
            _ => return None,
        }
    }
    None
}

fn directive_row_bytes(classified: &ClassifiedTokens, idx: usize) -> Option<&[u8]> {
    let start = *classified.tok_starts.get(idx)? as usize;
    let len = *classified.tok_lens.get(idx)? as usize;
    classified.source.get(start..start.checked_add(len)?)
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0_usize;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &bytes[start..end]
}
