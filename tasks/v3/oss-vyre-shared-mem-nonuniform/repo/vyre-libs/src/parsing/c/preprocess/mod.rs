//! C11 preprocessor passes.

#[cfg(any(test, feature = "cpu-parity"))]
use crate::parsing::c::lex::tokens::TOK_PREPROC;
use crate::parsing::c::lex::tokens::{
    TOK_PP_DEFINE, TOK_PP_ELIF, TOK_PP_ELIFDEF, TOK_PP_ELIFNDEF, TOK_PP_ELSE, TOK_PP_EMBED,
    TOK_PP_ENDIF, TOK_PP_ERROR, TOK_PP_IDENT, TOK_PP_IF, TOK_PP_IFDEF, TOK_PP_IFNDEF,
    TOK_PP_IMPORT, TOK_PP_INCLUDE, TOK_PP_INCLUDE_NEXT, TOK_PP_LINE, TOK_PP_NULL, TOK_PP_PRAGMA,
    TOK_PP_SCCS, TOK_PP_UNDEF, TOK_PP_WARNING,
};

/// Preprocessor side-effect metadata.
pub mod effects;
/// Macro-expansion kernel.
pub mod expansion;
/// GPU char-constant scanner. Phase 17b.3a: prefix tolerance + simple
/// escape table. 17b.3b adds octal / hex / UCN numeric escapes in the
/// same kernel.
pub mod gpu_char_constant_scan;
#[cfg(test)]
mod gpu_char_constant_scan_tests;
/// GPU comment-strip mask. Phase 17b.5: per-byte mask `1=comment,
/// 0=code` covering `//` line comments and `/*…*/` block comments.
/// Composes with `gpu_line_splice_classify` via mask-AND for the
/// pre-lex byte filter.
pub mod gpu_comment_strip_mask;
#[cfg(test)]
mod gpu_comment_strip_mask_tests;
#[cfg(test)]
mod gpu_conditional_value_tests;
/// GPU `#define` row parser. Phase 17b.6: per `TOK_PREPROC` token of
/// kind `TOK_PP_DEFINE`, extracts macro name + optional arg-list +
/// replacement body byte spans. Per-thread, fully parallel.
pub mod gpu_define_parse;
#[cfg(test)]
mod gpu_define_parse_tests;
/// GPU directive-metadata kernel  -  replaces the CPU
/// `reference_c_preprocessor_directive_metadata` for production paths.
/// Phase 17a: directive kind classification. Phase 17b will add the
/// shunting-yard conditional evaluator in the same module.
pub mod gpu_directive_metadata;
mod gpu_directive_parse_shared;
/// GPU `#if` / `#elif` expression evaluator. Phase 17b.4: per-thread
/// iterative shunting-yard parser using fixed-depth value/operator
/// stacks. Composes the literal scan, char-constant scan, and
/// defined-name lookup logic. Last piece of 17b.
pub mod gpu_if_expression;
/// ABI helpers for the GPU `#if` / `#elif` expression evaluator.
pub mod gpu_if_expression_abi;
/// GPU `#ifdef` / `#ifndef` evaluator. Phase 17b.1 of the directive
/// metadata pipeline. Composes with `gpu_directive_metadata` (which
/// runs first to populate `directive_kinds`) and runs second to fill
/// the `ifdef`/`ifndef` rows of `directive_values`.
pub mod gpu_ifdef_value;
/// GPU `#include` row parser. Phase 17b.7: per `TOK_PREPROC` token of
/// kind `TOK_PP_INCLUDE` / `TOK_PP_INCLUDE_NEXT`, extracts the path
/// byte span and `<…>` vs `"…"` flag. Per-thread, fully parallel.
pub mod gpu_include_parse;
/// GPU integer-literal scanner. Phase 17b.2: standalone scanner kernel
/// for testing the literal-parse logic in isolation; phase 17b.4 will
/// inline the same logic into the `#if` expression evaluator.
pub mod gpu_int_literal_scan;
/// GPU-resident preprocessor pipeline orchestration. Phase 18 of the
/// v0.4 plan: composes every kernel above into the host-side flow that
/// `vyre-frontend-c::tu_host` calls. Lives here (not in
/// vyre-frontend-c) so the unit/roundtrip tests don't have to drag in
/// the wgpu/vyre-debug dev-dep stack.
pub mod gpu_pipeline;
mod gpu_source_bytes;
/// GPU `#undef` row parser. Per `TOK_PREPROC` token of kind
/// `TOK_PP_UNDEF`, extracts the macro-name byte span. Per-thread,
/// fully parallel. Replaces the previous workaround of routing
/// `#undef` rows through `gpu_define_parse` (which has a 6-byte
/// keyword-length offset baked in for `#define`).
pub mod gpu_undef_parse;
/// Macro-expansion source-byte materialization helpers.
pub mod materialization;
/// Include source-manager ABI.
pub mod source;
/// Token synthesis helpers for macro stringification and token paste.
pub mod synthesis;

/// Source bytes after C translation phase 2 line splicing.
///
/// `bytes` contains the source with every backslash-newline pair deleted.
/// `original_offsets` maps each output byte boundary back to the input byte
/// boundary at the same logical position. Its length is always
/// `bytes.len() + 1`, with the final entry pointing at `source.len()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CLineSplicedSource {
    /// Phase-2 source bytes with line-splice pairs removed.
    pub bytes: Vec<u8>,
    /// Output byte-boundary to original byte-boundary map.
    pub original_offsets: Vec<usize>,
}

impl CLineSplicedSource {
    /// Map a logical byte boundary in `bytes` back to an original source offset.
    #[must_use]
    pub fn original_offset(&self, logical_offset: usize) -> usize {
        self.original_offsets
            .get(logical_offset)
            .copied()
            .or_else(|| self.original_offsets.last().copied())
            .unwrap_or(0)
    }
}

/// Delete C translation phase 2 backslash-newline pairs.
///
/// This is intentionally global and independent of directive parsing: every C
/// tokenization path must see the same phase-2 byte stream before directives,
/// macro names, and ordinary tokens are interpreted.
#[must_use]
pub fn c_translation_phase_line_splice(source: &[u8]) -> CLineSplicedSource {
    let mut bytes = Vec::with_capacity(source.len());
    let mut original_offsets = Vec::with_capacity(source.len() + 1);
    let mut index = 0usize;

    while index < source.len() {
        if source[index] == b'\\' {
            match source.get(index + 1).copied() {
                Some(b'\n') => {
                    index += 2;
                    continue;
                }
                Some(b'\r') => {
                    index += 2;
                    if source.get(index).copied() == Some(b'\n') {
                        index += 1;
                    }
                    continue;
                }
                _ => {}
            }
        }

        original_offsets.push(index);
        bytes.push(source[index]);
        index += 1;
    }

    original_offsets.push(source.len());
    CLineSplicedSource {
        bytes,
        original_offsets,
    }
}

/// Stable directive kind identifiers carried by host-side preprocessor analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CPreprocessorDirectiveKind {
    /// Empty `#` directive.
    Null,
    /// `#define`.
    Define,
    /// `#undef`.
    Undef,
    /// `#include`.
    Include,
    /// GNU `#include_next`.
    IncludeNext,
    /// `#if`.
    If,
    /// `#ifdef`.
    Ifdef,
    /// `#ifndef`.
    Ifndef,
    /// `#elif`.
    Elif,
    /// `#else`.
    Else,
    /// `#endif`.
    Endif,
    /// `#pragma`.
    Pragma,
    /// `#line`.
    Line,
    /// `#error`.
    Error,
    /// GNU `#warning`.
    Warning,
    /// System `#ident`.
    Ident,
    /// System `#sccs`.
    Sccs,
    /// `#embed` (C23): bring file contents into the TU as initializers.
    Embed,
    /// `#elifdef` (C23): shorthand for `#elif defined(...)`.
    Elifdef,
    /// `#elifndef` (C23): shorthand for `#elif !defined(...)`.
    Elifndef,
    /// `#import` (clang/Objective-C): include-once form.
    Import,
}

impl CPreprocessorDirectiveKind {
    /// Return the stable directive metadata token ID.
    #[must_use]
    pub const fn token_id(self) -> u32 {
        match self {
            Self::Null => TOK_PP_NULL,
            Self::Define => TOK_PP_DEFINE,
            Self::Undef => TOK_PP_UNDEF,
            Self::Include => TOK_PP_INCLUDE,
            Self::IncludeNext => TOK_PP_INCLUDE_NEXT,
            Self::If => TOK_PP_IF,
            Self::Ifdef => TOK_PP_IFDEF,
            Self::Ifndef => TOK_PP_IFNDEF,
            Self::Elif => TOK_PP_ELIF,
            Self::Else => TOK_PP_ELSE,
            Self::Endif => TOK_PP_ENDIF,
            Self::Pragma => TOK_PP_PRAGMA,
            Self::Line => TOK_PP_LINE,
            Self::Error => TOK_PP_ERROR,
            Self::Warning => TOK_PP_WARNING,
            Self::Ident => TOK_PP_IDENT,
            Self::Sccs => TOK_PP_SCCS,
            Self::Embed => TOK_PP_EMBED,
            Self::Elifdef => TOK_PP_ELIFDEF,
            Self::Elifndef => TOK_PP_ELIFNDEF,
            Self::Import => TOK_PP_IMPORT,
        }
    }
}

/// Parsed metadata for one compact `TOK_PREPROC` source row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CPreprocessorDirective {
    /// Recognized directive kind.
    pub kind: CPreprocessorDirectiveKind,
    /// Byte offset of the directive keyword within the phase-2 logical row.
    pub keyword_start: usize,
    /// Byte length of the directive keyword. Null directives use zero.
    pub keyword_len: usize,
    /// Byte offset where directive payload starts after horizontal whitespace.
    pub payload_start: usize,
    /// Byte offset where the phase-2 logical directive row ends.
    pub logical_end: usize,
}

/// Fail-loud preprocessor row classification error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CPreprocessorError {
    /// Byte offset where classification failed.
    pub offset: usize,
    /// Actionable diagnostic.
    pub message: &'static str,
}

impl core::fmt::Display for CPreprocessorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for CPreprocessorError {}

pub(crate) fn c_directive_payload<'a>(
    row: &'a [u8],
    directive: CPreprocessorDirective,
) -> Result<&'a [u8], CPreprocessorError> {
    row.get(directive.payload_start..directive.logical_end)
        .ok_or(CPreprocessorError {
            offset: directive.payload_start.min(row.len()),
            message: "preprocessor directive payload span is outside the logical row. Fix: pass phase-2 directive spans from the same row bytes.",
        })
}

/// Return the physical byte length of one logical preprocessing directive row.
///
/// C translation phase 2 deletes backslash-newline pairs before directive
/// parsing. The returned span therefore continues across `\\\n` and `\\\r\n`
/// pairs and stops before the first non-spliced line terminator.
#[must_use]
pub fn c_logical_directive_len(source: &[u8], offset: usize) -> usize {
    if offset >= source.len() {
        return 0;
    }

    let mut index = offset;
    while index < source.len() {
        match source[index] {
            b'\n' => {
                if index > offset && source[index - 1] == b'\\' {
                    index += 1;
                    continue;
                }
                break;
            }
            b'\r' => {
                let has_lf = source.get(index + 1).copied() == Some(b'\n');
                if index > offset && source[index - 1] == b'\\' {
                    index += usize::from(has_lf) + 1;
                    continue;
                }
                break;
            }
            _ => index += 1,
        }
    }

    index - offset
}

/// Classify a compact preprocessor row without expanding macros.
///
/// This function validates the directive name, treats horizontal whitespace
/// after `#` the same way C does, and leaves payload bytes untouched so macro
/// definitions, includes, pragmas, and `#error` diagnostics share one phase-2
/// view with downstream directive and macro handling.
///
/// # Errors
///
/// Returns a diagnostic when the row is not a directive row or uses an
/// unsupported directive spelling.
pub fn try_classify_preprocessor_directive(
    row: &[u8],
) -> Result<CPreprocessorDirective, CPreprocessorError> {
    let logical_end = c_logical_directive_len(row, 0);
    let physical_line = row.get(..logical_end).unwrap_or(row);
    let spliced = c_translation_phase_line_splice(physical_line);
    classify_phase2_preprocessor_directive(&spliced.bytes).map_err(|mut err| {
        err.offset = spliced.original_offset(err.offset);
        err
    })
}

fn classify_phase2_preprocessor_directive(
    line: &[u8],
) -> Result<CPreprocessorDirective, CPreprocessorError> {
    let mut index = skip_horizontal_ws(line, 0);
    if line.get(index).copied() != Some(b'#') {
        return Err(CPreprocessorError {
            offset: index,
            message: "Fix: preprocessor row must begin with # after horizontal whitespace",
        });
    }

    index += 1;
    index = skip_horizontal_ws(line, index);
    if index >= line.len() {
        return Ok(CPreprocessorDirective {
            kind: CPreprocessorDirectiveKind::Null,
            keyword_start: index,
            keyword_len: 0,
            payload_start: index,
            logical_end: line.len(),
        });
    }

    let keyword_start = index;
    while index < line.len() && is_directive_ident_continue(line[index]) {
        index += 1;
    }
    let keyword = &line[keyword_start..index];
    let kind = match keyword {
        b"define" => CPreprocessorDirectiveKind::Define,
        b"undef" => CPreprocessorDirectiveKind::Undef,
        b"include" => CPreprocessorDirectiveKind::Include,
        b"include_next" => CPreprocessorDirectiveKind::IncludeNext,
        b"if" => CPreprocessorDirectiveKind::If,
        b"ifdef" => CPreprocessorDirectiveKind::Ifdef,
        b"ifndef" => CPreprocessorDirectiveKind::Ifndef,
        b"elif" => CPreprocessorDirectiveKind::Elif,
        b"else" => CPreprocessorDirectiveKind::Else,
        b"endif" => CPreprocessorDirectiveKind::Endif,
        b"pragma" => CPreprocessorDirectiveKind::Pragma,
        b"line" => CPreprocessorDirectiveKind::Line,
        b"error" => CPreprocessorDirectiveKind::Error,
        b"warning" => CPreprocessorDirectiveKind::Warning,
        b"ident" => CPreprocessorDirectiveKind::Ident,
        b"sccs" => CPreprocessorDirectiveKind::Sccs,
        b"embed" => CPreprocessorDirectiveKind::Embed,
        b"elifdef" => CPreprocessorDirectiveKind::Elifdef,
        b"elifndef" => CPreprocessorDirectiveKind::Elifndef,
        b"import" => CPreprocessorDirectiveKind::Import,
        _ => {
            return Err(CPreprocessorError {
                offset: keyword_start,
                message: "Fix: implement or reject this C preprocessor directive explicitly",
            });
        }
    };

    Ok(CPreprocessorDirective {
        kind,
        keyword_start,
        keyword_len: keyword.len(),
        payload_start: skip_horizontal_ws(line, index),
        logical_end: line.len(),
    })
}

/// Build directive-kind and conditional-value metadata for compact C tokens.
///
/// `TOK_PREPROC` rows are classified from original source spans. Conditional
/// rows get an evaluated truth value; all other rows get `0`.
///
/// # Errors
///
/// Returns a diagnostic when token streams are inconsistent, a directive span
/// is outside `source`, or the current payload evaluator cannot parse a
/// conditional expression.
#[deprecated(
    note = "CPU reference oracle only; production C preprocessing must use the GPU directive metadata pipeline"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_c_preprocessor_directive_metadata(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
    source: &[u8],
    defined_macros: &[&[u8]],
) -> Result<(Vec<u32>, Vec<u32>), CPreprocessorError> {
    if tok_types.len() != tok_starts.len() || tok_types.len() != tok_lens.len() {
        return Err(CPreprocessorError {
            offset: tok_types.len().min(tok_starts.len()).min(tok_lens.len()),
            message: "Fix: token type/start/length streams must have identical lengths",
        });
    }

    let mut directive_kinds = vec![0; tok_types.len()];
    let mut directive_values = vec![0; tok_types.len()];
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
        let physical_logical_len = c_logical_directive_len(source, start);
        if physical_logical_len > len {
            return Err(CPreprocessorError {
                offset: start + len,
                message:
                    "Fix: TOK_PREPROC span must include the full phase-2 spliced directive row",
            });
        }
        let logical_end = start
            .checked_add(physical_logical_len)
            .ok_or(CPreprocessorError {
                offset: start,
                message: "Fix: directive logical span overflows source address space",
            })?;
        if token_end > source.len() {
            return Err(CPreprocessorError {
                offset: start,
                message: "Fix: preprocessor token span must be inside the source buffer",
            });
        }
        let row = source.get(start..logical_end).ok_or(CPreprocessorError {
            offset: start,
            message: "Fix: preprocessor token span must be inside the source buffer",
        })?;
        let spliced = c_translation_phase_line_splice(row);
        let directive =
            classify_phase2_preprocessor_directive(&spliced.bytes).map_err(|mut err| {
                err.offset = start + spliced.original_offset(err.offset);
                err
            })?;
        directive_kinds[idx] = directive.kind.token_id();
        directive_values[idx] =
            conditional_directive_value(&spliced.bytes, directive, defined_macros)
                .map_err(|mut err| {
                    err.offset = start + spliced.original_offset(err.offset);
                    err
                })?
                .unwrap_or(0);
    }
    Ok((directive_kinds, directive_values))
}

fn conditional_directive_value(
    row: &[u8],
    directive: CPreprocessorDirective,
    defined_macros: &[&[u8]],
) -> Result<Option<u32>, CPreprocessorError> {
    let payload = c_directive_payload(row, directive)?;
    match directive.kind {
        CPreprocessorDirectiveKind::If | CPreprocessorDirectiveKind::Elif => Ok(Some(u32::from(
            PreprocessorExprParser {
                bytes: payload,
                index: 0,
                base_offset: directive.payload_start,
                defined_macros,
                depth: 0,
            }
            .parse()?,
        ))),
        CPreprocessorDirectiveKind::Ifdef => Ok(Some(u32::from(
            first_payload_ident(payload).is_some_and(|name| macro_is_defined(defined_macros, name)),
        ))),
        CPreprocessorDirectiveKind::Ifndef => Ok(Some(u32::from(
            first_payload_ident(payload)
                .is_some_and(|name| !macro_is_defined(defined_macros, name)),
        ))),
        _ => Ok(None),
    }
}

mod expr_parser;
pub use expr_parser::is_reserved_preprocessor_identifier;
use expr_parser::PreprocessorExprParser;

pub(super) fn first_payload_ident(payload: &[u8]) -> Option<&[u8]> {
    let mut index = skip_horizontal_ws(payload, 0);
    let start = index;
    if !payload.get(index).copied().is_some_and(is_c_ident_start) {
        return None;
    }
    index += 1;
    while payload
        .get(index)
        .copied()
        .is_some_and(is_directive_ident_continue)
    {
        index += 1;
    }
    payload.get(start..index)
}

#[inline]
pub(super) fn macro_is_defined(defined_macros: &[&[u8]], name: &[u8]) -> bool {
    defined_macros.iter().any(|candidate| *candidate == name)
}

#[inline]
pub(super) fn skip_horizontal_ws(bytes: &[u8], mut index: usize) -> usize {
    loop {
        match bytes.get(index).copied() {
            Some(b' ' | b'\t' | b'\x0b' | b'\x0c') => index += 1,
            Some(b'/') if bytes.get(index + 1).copied() == Some(b'/') => {
                return bytes.len();
            }
            Some(b'/') if bytes.get(index + 1).copied() == Some(b'*') => {
                index += 2;
                while index + 1 < bytes.len() && bytes.get(index..index + 2) != Some(b"*/") {
                    index += 1;
                }
                if index + 1 >= bytes.len() {
                    return bytes.len();
                }
                index += 2;
            }
            _ => return index,
        }
    }
}

#[inline]
pub(super) fn is_directive_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[inline]
pub(super) fn is_c_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::{c_directive_payload, CPreprocessorDirective, CPreprocessorDirectiveKind};

    #[test]
    fn directive_payload_rejects_corrupt_span_instead_of_defaulting_empty() {
        let directive = CPreprocessorDirective {
            kind: CPreprocessorDirectiveKind::If,
            keyword_start: 1,
            keyword_len: 2,
            payload_start: 8,
            logical_end: 4,
        };
        let err = c_directive_payload(b"#if 1", directive)
            .expect_err("corrupt directive spans must fail loudly");
        assert_eq!(err.offset, 5);
        assert!(
            err.message.contains("payload span is outside"),
            "error must explain the corrupted payload span"
        );
    }
}
