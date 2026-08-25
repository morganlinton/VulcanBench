//! Source-manager ABI for C `#include` and GNU `#include_next`.

#[cfg(any(test, feature = "cpu-parity"))]
use crate::parsing::c::lex::tokens::TOK_PREPROC;
#[cfg(any(test, feature = "cpu-parity"))]
use crate::parsing::c::preprocess::c_logical_directive_len;
use crate::parsing::c::preprocess::{
    c_directive_payload, c_translation_phase_line_splice, try_classify_preprocessor_directive,
    CPreprocessorDirectiveKind, CPreprocessorError,
};

/// Header spelling class from a C include directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CIncludeStyle {
    /// `"header.h"` lookup.
    Quote,
    /// `<header.h>` lookup.
    Angle,
}

/// Fully parsed include request passed to the embedding source manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CIncludeRequest {
    /// `#include` or GNU `#include_next`.
    pub directive: CPreprocessorDirectiveKind,
    /// Header-name delimiter style.
    pub style: CIncludeStyle,
    /// Header spelling without delimiters.
    pub spelling: Vec<u8>,
    /// Original source offset of the directive row.
    pub directive_offset: usize,
    /// Original source offset of the header payload.
    pub payload_offset: usize,
}

impl CIncludeRequest {
    /// Return true when this request came from GNU `#include_next`.
    #[must_use]
    pub const fn is_include_next(&self) -> bool {
        matches!(self.directive, CPreprocessorDirectiveKind::IncludeNext)
    }
}

/// Source bytes returned by a source manager include load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CSourceFile {
    /// Stable source ID assigned by the embedding source manager.
    pub source_id: u32,
    /// Human-readable resolved name or path for diagnostics.
    pub display_name: String,
    /// Loaded source bytes.
    pub bytes: Vec<u8>,
}

/// Host source manager contract for include loading.
///
/// The preprocessor frontend owns directive parsing and include spelling
/// validation. The embedder owns search paths, `#include_next` continuation,
/// virtual filesystems, and filesystem policy.
pub trait CPreprocessorSourceManager {
    /// Resolve and load one parsed include request.
    ///
    /// # Errors
    ///
    /// Returns an actionable preprocessor diagnostic when the include cannot
    /// be resolved or loaded.
    fn load_include(&self, request: &CIncludeRequest) -> Result<CSourceFile, CPreprocessorError>;
}

/// Include source loaded for one directive token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CResolvedInclude {
    /// Index of the `TOK_PREPROC` token that requested this include.
    pub token_index: usize,
    /// Parsed include request.
    pub request: CIncludeRequest,
    /// Loaded source returned by the source manager.
    pub source: CSourceFile,
}

/// Parse an include request from one physical directive row.
///
/// `row` may contain phase-2 line splices; the returned offsets are mapped back
/// to the original row byte space and then shifted by `directive_offset`.
///
/// # Errors
///
/// Returns a diagnostic when the row is an include directive but its header
/// payload is malformed.
pub fn parse_c_include_request(
    row: &[u8],
    directive_offset: usize,
) -> Result<Option<CIncludeRequest>, CPreprocessorError> {
    let spliced = c_translation_phase_line_splice(row);
    let directive = try_classify_preprocessor_directive(&spliced.bytes).map_err(|mut err| {
        err.offset = directive_offset + spliced.original_offset(err.offset);
        err
    })?;
    if !matches!(
        directive.kind,
        CPreprocessorDirectiveKind::Include | CPreprocessorDirectiveKind::IncludeNext
    ) {
        return Ok(None);
    }

    let payload = c_directive_payload(&spliced.bytes, directive).map_err(|mut err| {
        err.offset = directive_offset + spliced.original_offset(err.offset);
        err
    })?;
    let (style, spelling, payload_rel) =
        parse_header_name_payload(payload).map_err(|mut err| {
            err.offset =
                directive_offset + spliced.original_offset(directive.payload_start + err.offset);
            err
        })?;
    Ok(Some(CIncludeRequest {
        directive: directive.kind,
        style,
        spelling,
        directive_offset,
        payload_offset: directive_offset
            + spliced.original_offset(directive.payload_start + payload_rel),
    }))
}

/// Load all include directives from a compact token stream through `manager`.
///
/// # Errors
///
/// Returns a diagnostic when token streams are inconsistent, a directive span
/// is invalid, an include payload is malformed, or the source manager rejects
/// a load.
#[deprecated(
    note = "CPU reference oracle only; production include loading must use the GPU preprocessor pipeline source-manager path"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_c_preprocessor_load_includes<M: CPreprocessorSourceManager>(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
    source: &[u8],
    manager: &M,
) -> Result<Vec<CResolvedInclude>, CPreprocessorError> {
    if tok_types.len() != tok_starts.len() || tok_types.len() != tok_lens.len() {
        return Err(CPreprocessorError {
            offset: tok_types.len().min(tok_starts.len()).min(tok_lens.len()),
            message: "Fix: token type/start/length streams must have identical lengths",
        });
    }

    let mut resolved = Vec::new();
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
        if let Some(request) = parse_c_include_request(row, start)? {
            let source = manager.load_include(&request)?;
            resolved.push(CResolvedInclude {
                token_index: idx,
                request,
                source,
            });
        }
    }
    Ok(resolved)
}

fn parse_header_name_payload(
    payload: &[u8],
) -> Result<(CIncludeStyle, Vec<u8>, usize), CPreprocessorError> {
    let start = skip_horizontal_ws(payload, 0);
    let Some(open) = payload.get(start).copied() else {
        return Err(CPreprocessorError {
            offset: start,
            message: "Fix: #include needs a header name payload",
        });
    };
    match open {
        b'"' => parse_delimited_header(payload, start, b'"', CIncludeStyle::Quote),
        b'<' => parse_delimited_header(payload, start, b'>', CIncludeStyle::Angle),
        _ => Err(CPreprocessorError {
            offset: start,
            message:
                "Fix: #include payload must be a quoted or angle-bracket header name after macro expansion",
        }),
    }
}

fn parse_delimited_header(
    payload: &[u8],
    start: usize,
    close: u8,
    style: CIncludeStyle,
) -> Result<(CIncludeStyle, Vec<u8>, usize), CPreprocessorError> {
    let mut index = start + 1;
    while let Some(byte) = payload.get(index).copied() {
        if matches!(byte, b'\n' | b'\r') {
            return Err(CPreprocessorError {
                offset: index,
                message: "Fix: #include header name must close before newline",
            });
        }
        if byte == close {
            let trailing = skip_horizontal_ws(payload, index + 1);
            if trailing != payload.len() {
                return Err(CPreprocessorError {
                    offset: trailing,
                    message: "Fix: unexpected tokens after #include header name",
                });
            }
            return Ok((style, payload[start + 1..index].to_vec(), start + 1));
        }
        index += 1;
    }
    Err(CPreprocessorError {
        offset: start,
        message: "Fix: terminate #include header name",
    })
}

fn skip_horizontal_ws(bytes: &[u8], mut index: usize) -> usize {
    while matches!(bytes.get(index), Some(b' ' | b'\t' | b'\x0b' | b'\x0c')) {
        index += 1;
    }
    index
}
