//! Side-effect metadata for preprocessor directives.

#[cfg(any(test, feature = "cpu-parity"))]
use crate::parsing::c::lex::tokens::TOK_PREPROC;
use crate::parsing::c::lex::tokens::{
    TOK_PP_EFFECT_ERROR_DIAGNOSTIC, TOK_PP_EFFECT_IDENT, TOK_PP_EFFECT_INCLUDE,
    TOK_PP_EFFECT_INCLUDE_NEXT, TOK_PP_EFFECT_LINE, TOK_PP_EFFECT_PRAGMA,
    TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_ERROR, TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_IGNORED,
    TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_POP, TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_PUSH,
    TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_WARNING, TOK_PP_EFFECT_PRAGMA_ONCE, TOK_PP_EFFECT_SCCS,
    TOK_PP_EFFECT_WARNING_DIAGNOSTIC,
};
#[cfg(any(test, feature = "cpu-parity"))]
use crate::parsing::c::preprocess::c_logical_directive_len;
use crate::parsing::c::preprocess::{
    c_directive_payload, c_translation_phase_line_splice, try_classify_preprocessor_directive,
    CPreprocessorDirectiveKind, CPreprocessorError,
};

/// Stable side-effect kind emitted for directives with frontend-visible state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CPreprocessorSideEffectKind {
    /// `#include`.
    Include,
    /// GNU `#include_next`.
    IncludeNext,
    /// Unclassified pragma with implementation-defined payload.
    Pragma,
    /// `#pragma once`.
    PragmaOnce,
    /// `#pragma GCC diagnostic push` or `#pragma clang diagnostic push`.
    PragmaDiagnosticPush,
    /// `#pragma GCC diagnostic pop` or `#pragma clang diagnostic pop`.
    PragmaDiagnosticPop,
    /// `#pragma GCC diagnostic ignored`.
    PragmaDiagnosticIgnored,
    /// `#pragma GCC diagnostic warning`.
    PragmaDiagnosticWarning,
    /// `#pragma GCC diagnostic error`.
    PragmaDiagnosticError,
    /// `#error`.
    ErrorDiagnostic,
    /// GNU `#warning`.
    WarningDiagnostic,
    /// `#ident`.
    Ident,
    /// `#sccs`.
    Sccs,
    /// `#line`.
    Line,
}

impl CPreprocessorSideEffectKind {
    /// Return the stable side-effect metadata token ID.
    #[must_use]
    pub const fn token_id(self) -> u32 {
        match self {
            Self::Include => TOK_PP_EFFECT_INCLUDE,
            Self::IncludeNext => TOK_PP_EFFECT_INCLUDE_NEXT,
            Self::Pragma => TOK_PP_EFFECT_PRAGMA,
            Self::PragmaOnce => TOK_PP_EFFECT_PRAGMA_ONCE,
            Self::PragmaDiagnosticPush => TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_PUSH,
            Self::PragmaDiagnosticPop => TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_POP,
            Self::PragmaDiagnosticIgnored => TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_IGNORED,
            Self::PragmaDiagnosticWarning => TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_WARNING,
            Self::PragmaDiagnosticError => TOK_PP_EFFECT_PRAGMA_DIAGNOSTIC_ERROR,
            Self::ErrorDiagnostic => TOK_PP_EFFECT_ERROR_DIAGNOSTIC,
            Self::WarningDiagnostic => TOK_PP_EFFECT_WARNING_DIAGNOSTIC,
            Self::Ident => TOK_PP_EFFECT_IDENT,
            Self::Sccs => TOK_PP_EFFECT_SCCS,
            Self::Line => TOK_PP_EFFECT_LINE,
        }
    }
}

/// Source-positioned side effect decoded from one directive row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CPreprocessorSideEffect {
    /// Classified side-effect kind.
    pub kind: CPreprocessorSideEffectKind,
    /// Original source offset of the directive payload.
    pub payload_start: usize,
    /// Original byte length of the directive payload.
    pub payload_len: usize,
    /// Original source offset of the actionable detail payload.
    pub detail_start: usize,
    /// Original byte length of the actionable detail payload.
    pub detail_len: usize,
}

/// Parallel metadata streams for preprocessor side effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CPreprocessorSideEffectMetadata {
    /// Side-effect token ID per input token, or zero for no side effect.
    pub kinds: Vec<u32>,
    /// Original source offset of the directive payload per input token.
    pub payload_starts: Vec<u32>,
    /// Original byte length of the directive payload per input token.
    pub payload_lens: Vec<u32>,
    /// Original source offset of the detail payload per input token.
    pub detail_starts: Vec<u32>,
    /// Original byte length of the detail payload per input token.
    pub detail_lens: Vec<u32>,
}

/// Decode pragma, include, and diagnostic side effects from one directive row.
///
/// # Errors
///
/// Returns a diagnostic when the row is not a directive row or uses a malformed
/// recognized side-effect spelling.
pub fn classify_c_preprocessor_side_effect(
    row: &[u8],
    directive_offset: usize,
) -> Result<Option<CPreprocessorSideEffect>, CPreprocessorError> {
    let spliced = c_translation_phase_line_splice(row);
    let directive = try_classify_preprocessor_directive(&spliced.bytes).map_err(|mut err| {
        err.offset = directive_offset + spliced.original_offset(err.offset);
        err
    })?;
    let payload = c_directive_payload(&spliced.bytes, directive).map_err(|mut err| {
        err.offset = directive_offset + spliced.original_offset(err.offset);
        err
    })?;
    let payload_start = directive_offset + spliced.original_offset(directive.payload_start);
    let payload_end = directive_offset + spliced.original_offset(directive.logical_end);
    let payload_len = payload_end.saturating_sub(payload_start);

    let (kind, detail_rel, detail_len) = match directive.kind {
        CPreprocessorDirectiveKind::Include => (
            CPreprocessorSideEffectKind::Include,
            first_payload_byte(payload),
            payload_trimmed_len(payload),
        ),
        CPreprocessorDirectiveKind::IncludeNext => (
            CPreprocessorSideEffectKind::IncludeNext,
            first_payload_byte(payload),
            payload_trimmed_len(payload),
        ),
        CPreprocessorDirectiveKind::Pragma => {
            classify_pragma_payload(payload).map_err(|mut err| {
                err.offset = directive_offset
                    + spliced.original_offset(directive.payload_start + err.offset);
                err
            })?
        }
        CPreprocessorDirectiveKind::Error => (
            CPreprocessorSideEffectKind::ErrorDiagnostic,
            first_payload_byte(payload),
            payload_trimmed_len(payload),
        ),
        CPreprocessorDirectiveKind::Warning => (
            CPreprocessorSideEffectKind::WarningDiagnostic,
            first_payload_byte(payload),
            payload_trimmed_len(payload),
        ),
        CPreprocessorDirectiveKind::Ident => (
            CPreprocessorSideEffectKind::Ident,
            first_payload_byte(payload),
            payload_trimmed_len(payload),
        ),
        CPreprocessorDirectiveKind::Sccs => (
            CPreprocessorSideEffectKind::Sccs,
            first_payload_byte(payload),
            payload_trimmed_len(payload),
        ),
        CPreprocessorDirectiveKind::Line => (
            CPreprocessorSideEffectKind::Line,
            first_payload_byte(payload),
            payload_trimmed_len(payload),
        ),
        _ => return Ok(None),
    };

    Ok(Some(CPreprocessorSideEffect {
        kind,
        payload_start,
        payload_len,
        detail_start: directive_offset
            + spliced.original_offset(directive.payload_start + detail_rel),
        detail_len,
    }))
}

/// Build side-effect metadata for compact C tokens.
///
/// # Errors
///
/// Returns a diagnostic when token streams are inconsistent or a directive span
/// is invalid.
#[deprecated(
    note = "CPU reference oracle only; production side-effect metadata must use GPU preprocessor classification"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_c_preprocessor_side_effect_metadata(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
    source: &[u8],
) -> Result<CPreprocessorSideEffectMetadata, CPreprocessorError> {
    if tok_types.len() != tok_starts.len() || tok_types.len() != tok_lens.len() {
        return Err(CPreprocessorError {
            offset: tok_types.len().min(tok_starts.len()).min(tok_lens.len()),
            message: "Fix: token type/start/length streams must have identical lengths",
        });
    }

    let mut metadata = CPreprocessorSideEffectMetadata {
        kinds: vec![0; tok_types.len()],
        payload_starts: vec![0; tok_types.len()],
        payload_lens: vec![0; tok_types.len()],
        detail_starts: vec![0; tok_types.len()],
        detail_lens: vec![0; tok_types.len()],
    };
    for (idx, ((tok_type, start), len)) in
        tok_types.iter().zip(tok_starts).zip(tok_lens).enumerate()
    {
        if *tok_type != TOK_PREPROC {
            continue;
        }
        let start = usize::try_from(*start).map_err(|_| CPreprocessorError {
            offset: idx,
            message: "Fix: token start does not fit host usize",
        })?;
        let len = usize::try_from(*len).map_err(|_| CPreprocessorError {
            offset: idx,
            message: "Fix: token length does not fit host usize",
        })?;
        let token_end = start.checked_add(len).ok_or(CPreprocessorError {
            offset: start,
            message: "Fix: token span overflows source address space",
        })?;
        let logical_len = c_logical_directive_len(source, start);
        if logical_len > len {
            return Err(CPreprocessorError {
                offset: start + len,
                message:
                    "Fix: TOK_PREPROC span must include the full phase-2 spliced directive row",
            });
        }
        if token_end > source.len() {
            return Err(CPreprocessorError {
                offset: start,
                message: "Fix: preprocessor token span must be inside the source buffer",
            });
        }
        let logical_end = start.checked_add(logical_len).ok_or(CPreprocessorError {
            offset: start,
            message: "Fix: directive logical span overflows source address space",
        })?;
        let row = source.get(start..logical_end).ok_or(CPreprocessorError {
            offset: start,
            message: "Fix: preprocessor token span must be inside the source buffer",
        })?;
        if let Some(effect) = classify_c_preprocessor_side_effect(row, start)? {
            metadata.kinds[idx] = effect.kind.token_id();
            metadata.payload_starts[idx] = checked_u32(
                effect.payload_start,
                "Fix: payload offset exceeds u32 metadata",
            )?;
            metadata.payload_lens[idx] = checked_u32(
                effect.payload_len,
                "Fix: payload length exceeds u32 metadata",
            )?;
            metadata.detail_starts[idx] = checked_u32(
                effect.detail_start,
                "Fix: detail offset exceeds u32 metadata",
            )?;
            metadata.detail_lens[idx] =
                checked_u32(effect.detail_len, "Fix: detail length exceeds u32 metadata")?;
        }
    }
    Ok(metadata)
}

fn classify_pragma_payload(
    payload: &[u8],
) -> Result<(CPreprocessorSideEffectKind, usize, usize), CPreprocessorError> {
    let Some((first, first_start, first_end)) = next_ident(payload, 0) else {
        return Err(CPreprocessorError {
            offset: 0,
            message: "Fix: #pragma needs a pragma namespace or command",
        });
    };
    if first == b"once" {
        return Ok((
            CPreprocessorSideEffectKind::PragmaOnce,
            first_start,
            first_end - first_start,
        ));
    }
    if first == b"GCC" || first == b"clang" {
        let Some((second, _, second_end)) = next_ident(payload, first_end) else {
            return Ok((
                CPreprocessorSideEffectKind::Pragma,
                first_start,
                payload_trimmed_len(payload),
            ));
        };
        if second == b"diagnostic" {
            let Some((action, action_start, action_end)) = next_ident(payload, second_end) else {
                return Err(CPreprocessorError {
                    offset: second_end,
                    message: "Fix: #pragma diagnostic needs push, pop, ignored, warning, or error",
                });
            };
            let kind = match action {
                b"push" => CPreprocessorSideEffectKind::PragmaDiagnosticPush,
                b"pop" => CPreprocessorSideEffectKind::PragmaDiagnosticPop,
                b"ignored" => CPreprocessorSideEffectKind::PragmaDiagnosticIgnored,
                b"warning" => CPreprocessorSideEffectKind::PragmaDiagnosticWarning,
                b"error" => CPreprocessorSideEffectKind::PragmaDiagnosticError,
                _ => {
                    return Err(CPreprocessorError {
                        offset: action_start,
                        message:
                            "Fix: #pragma diagnostic action must be push, pop, ignored, warning, or error",
                    });
                }
            };
            let detail_start = if matches!(
                kind,
                CPreprocessorSideEffectKind::PragmaDiagnosticIgnored
                    | CPreprocessorSideEffectKind::PragmaDiagnosticWarning
                    | CPreprocessorSideEffectKind::PragmaDiagnosticError
            ) {
                skip_horizontal_ws(payload, action_end)
            } else {
                action_start
            };
            return Ok((
                kind,
                detail_start,
                payload_trimmed_len(&payload[detail_start..]),
            ));
        }
    }
    Ok((
        CPreprocessorSideEffectKind::Pragma,
        first_start,
        payload_trimmed_len(payload),
    ))
}

fn next_ident(payload: &[u8], index: usize) -> Option<(&[u8], usize, usize)> {
    let mut start = skip_horizontal_ws(payload, index);
    if !matches!(payload.get(start), Some(b'_' | b'a'..=b'z' | b'A'..=b'Z')) {
        return None;
    }
    let ident_start = start;
    start += 1;
    while matches!(
        payload.get(start),
        Some(b'_' | b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9')
    ) {
        start += 1;
    }
    Some((&payload[ident_start..start], ident_start, start))
}

fn first_payload_byte(payload: &[u8]) -> usize {
    skip_horizontal_ws(payload, 0)
}

fn payload_trimmed_len(payload: &[u8]) -> usize {
    let start = skip_horizontal_ws(payload, 0);
    let mut end = payload.len();
    while end > start && matches!(payload[end - 1], b' ' | b'\t' | b'\x0b' | b'\x0c') {
        end -= 1;
    }
    end.saturating_sub(start)
}

fn skip_horizontal_ws(bytes: &[u8], mut index: usize) -> usize {
    while matches!(bytes.get(index), Some(b' ' | b'\t' | b'\x0b' | b'\x0c')) {
        index += 1;
    }
    index
}

fn checked_u32(value: usize, message: &'static str) -> Result<u32, CPreprocessorError> {
    u32::try_from(value).map_err(|_| CPreprocessorError {
        offset: value,
        message,
    })
}
