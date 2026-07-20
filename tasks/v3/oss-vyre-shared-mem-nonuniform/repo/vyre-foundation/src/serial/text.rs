// Canonical text format for vyre IR programs (VYRE_RELEASE_PLAN Phase 2.3-2.4).
//
// The text format is a *stable* human-diffable encoding of the IR
// that round-trips byte-for-byte through the binary wire format.
//
// ```text
// vyre_ir v0.1
// wire_bytes 42
// 56495230 00050001 ... (hex-encoded wire body)
// ```
//
// # Format
//
// ```ebnf
// program       = header "\n" body "\n"
// header        = "vyre_ir v0.1\n" wire_byte_line
// wire_byte_line = "wire_bytes " uint32 "\n"
// body          = hex_line { hex_line }
// hex_line      = hex_byte{1..64} "\n"
// hex_byte      = hex_digit hex_digit
// hex_digit     = "0".."9" | "a".."f"
// uint32        = ("0".."9")+
// ```
//
// The header's `wire_bytes` line carries the body length in bytes so
// the parser can reject truncation before allocating the decode
// buffer. The body is the exact output of [`Program::to_wire`]
// rendered as lowercase hex, chunked at 32 bytes per line (64 hex
// characters) for diffability. The last line may be shorter.
//
// # Why route through the binary wire format?
//
// The binary wire format is already a stable canonical encoding of
// every `Program` variant, already has bounds checks, already has
// cross-crate parity tests, and is already the thing the runtime
// uses. Building a second parser that reads a handwritten
// hierarchical syntax (S-expressions, JSON, TOML, etc.) would
// duplicate every escape/bounds/unicode check while providing no
// additional safety. The text format layered on top of the binary
// format is:
//
// - **Deterministic**  -  same program always produces the same bytes
//   because `to_wire` is deterministic and hex encoding is
//   deterministic.
// - **Human-diffable**  -  `git diff` on two `.vyre` files shows
//   exactly which bytes changed, which in the wire format usually
//   corresponds to specific node/buffer changes.
// - **Round-trippable**  -  the round-trip property
//   `from_text(to_text(p)) == p` holds by construction because the
//   inner `to_wire`/`from_wire` already round-trips. This file only
//   adds the hex envelope.
// - **Small**  -  ~150 LOC of parser and serializer total, fits in
//   one file, one set of tests.
//
// A richer S-expression form can be layered on top later if a
// reader wants op-by-op pretty printing; the stable format for
// persistence and CI diff is this one.

use crate::ir_inner::model::program::Program;

/// Magic header that every text-format program starts with.
///
/// Bumping the version requires a migration. The parser rejects any
/// program with a different header.
pub const TEXT_FORMAT_HEADER: &str = "vyre_ir v0.1";

/// Maximum body length in bytes the parser will accept before
/// failing with a bounded-allocation error. Mirrors the I10 bound
/// on `Program::from_wire`: 64 MiB is larger than any legitimate
/// program but small enough that a hostile input cannot trigger an
/// OOM.
pub const MAX_TEXT_WIRE_BYTES: usize = 64 * 1024 * 1024;

/// How many wire bytes pack into each hex line. 32 bytes = 64
/// hex characters, which keeps line width under the standard
/// 80-column budget including the trailing newline.
pub const WIRE_BYTES_PER_LINE: usize = 32;

/// Error returned when a text-format program fails to parse.
///
/// Every variant carries an actionable `Fix:` message rendered via
/// [`TextParseError::fix_hint`]. Parsing never panics.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextParseError {
    /// The program did not start with the `vyre_ir v0.1` header.
    MissingHeader {
        /// Snippet of the first line, truncated to 64 characters.
        observed: String,
    },
    /// The header was present but the second line was not the
    /// expected `wire_bytes N` declaration.
    MissingWireBytesLine {
        /// Snippet of the second line, truncated to 64 characters.
        observed: String,
    },
    /// `wire_bytes` parsed but exceeded `MAX_TEXT_WIRE_BYTES`.
    WireBytesTooLarge {
        /// The declared length.
        declared: usize,
    },
    /// A hex line contained a non-hex character.
    InvalidHexCharacter {
        /// Offending line number (1-indexed).
        line: usize,
        /// The character that broke the parse.
        character: char,
    },
    /// A hex line's character count was odd, which cannot round-trip
    /// to whole bytes.
    OddHexLineLength {
        /// Offending line number (1-indexed).
        line: usize,
        /// The observed character count.
        observed: usize,
    },
    /// Total decoded bytes did not match the declared `wire_bytes`.
    DeclaredLengthMismatch {
        /// Declared byte count from the header.
        declared: usize,
        /// Actual decoded byte count.
        actual: usize,
    },
    /// The inner binary wire decoder rejected the byte payload.
    ///
    /// The carried error is whatever [`Program::from_wire`] emitted  -
    /// a typed [`crate::error::Error`] whose `Display` impl already
    /// carries the `Fix:`-prefixed diagnostic prose.
    WireDecodeFailed {
        /// The inner decoder error.
        inner: crate::error::Error,
    },
    /// The inner binary wire encoder rejected the program when
    /// we tried to serialize it. Only emitted by `to_text`.
    WireEncodeFailed {
        /// The inner encoder error.
        inner: crate::error::Error,
    },
}

impl TextParseError {
    /// Human-readable one-line rendering.
    #[must_use]
    #[inline]
    pub fn message(&self) -> String {
        match self {
            Self::MissingHeader { observed } => format!(
                "text format must start with `{TEXT_FORMAT_HEADER}` but saw `{observed}`. {}",
                self.fix_hint()
            ),
            Self::MissingWireBytesLine { observed } => format!(
                "text format header must be followed by `wire_bytes <N>` but saw `{observed}`. {}",
                self.fix_hint()
            ),
            Self::WireBytesTooLarge { declared } => format!(
                "declared wire_bytes = {declared} exceeds MAX_TEXT_WIRE_BYTES = {MAX_TEXT_WIRE_BYTES}. {}",
                self.fix_hint()
            ),
            Self::InvalidHexCharacter { line, character } => format!(
                "invalid hex character `{character}` on body line {line}. {}",
                self.fix_hint()
            ),
            Self::OddHexLineLength { line, observed } => format!(
                "hex body line {line} has {observed} characters, must be even. {}",
                self.fix_hint()
            ),
            Self::DeclaredLengthMismatch { declared, actual } => format!(
                "declared wire_bytes = {declared} but decoded {actual}. {}",
                self.fix_hint()
            ),
            Self::WireDecodeFailed { inner } => {
                format!("inner binary wire decoder rejected the body: {inner}")
            }
            Self::WireEncodeFailed { inner } => {
                format!("inner binary wire encoder rejected the program: {inner}")
            }
        }
    }

    /// Actionable `Fix:`-prefixed hint for the caller.
    #[must_use]
    #[inline]
    pub fn fix_hint(&self) -> &'static str {
        match self {
            Self::MissingHeader { .. } => {
                "Fix: re-emit the program with Program::to_text, or manually prepend `vyre_ir v0.1\\n`."
            }
            Self::MissingWireBytesLine { .. } => {
                "Fix: re-emit the program with Program::to_text; the second line must read `wire_bytes N`."
            }
            Self::WireBytesTooLarge { .. } => {
                "Fix: the program is too large to round-trip through the text format; use Program::to_wire directly or split the program."
            }
            Self::InvalidHexCharacter { .. } | Self::OddHexLineLength { .. } => {
                "Fix: the text body must be lowercase hex with 64 characters per line (32 bytes). Re-emit with Program::to_text."
            }
            Self::DeclaredLengthMismatch { .. } => {
                "Fix: the wire_bytes header does not match the body length. Recompute wire_bytes or re-emit with Program::to_text."
            }
            Self::WireDecodeFailed { .. } | Self::WireEncodeFailed { .. } => {
                "Fix: see the wrapped error message for the underlying wire-format problem."
            }
        }
    }
}

impl std::fmt::Display for TextParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for TextParseError {}

impl Program {
    /// Serialize to the canonical vyre IR text format.
    ///
    /// # Errors
    ///
    /// Returns `TextParseError::WireEncodeFailed` when the inner
    /// binary wire encoder fails. This cannot happen for a program
    /// produced by a successful `Program::new` because every field
    /// of `Program` is a valid wire input by construction; the
    /// error path exists only for programs synthesized through
    /// unsafe means or a wire-format breaking change.
    #[inline]
    #[must_use]
    pub fn to_text(&self) -> Result<String, TextParseError> {
        let bytes = self
            .to_wire()
            .map_err(|error| TextParseError::WireEncodeFailed { inner: error })?;
        Ok(encode_text_body(&bytes))
    }

    /// Parse the canonical vyre IR text format.
    ///
    /// # Errors
    ///
    /// Returns a [`TextParseError`] describing the first parse
    /// failure. Parsing is total  -  no panic path.
    #[inline]
    #[must_use]
    pub fn from_text(input: &str) -> Result<Self, TextParseError> {
        let mut lines = input.lines();
        let header = lines.next().unwrap_or("");
        if header != TEXT_FORMAT_HEADER {
            return Err(TextParseError::MissingHeader {
                observed: truncate(header, 64),
            });
        }
        let wire_line = lines.next().unwrap_or("");
        let declared_bytes = parse_wire_bytes_line(wire_line)?;
        if declared_bytes > MAX_TEXT_WIRE_BYTES {
            return Err(TextParseError::WireBytesTooLarge {
                declared: declared_bytes,
            });
        }
        let mut body = Vec::with_capacity(declared_bytes);
        for (offset, line) in lines.enumerate() {
            let trimmed = line.trim_end_matches('\r');
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.len() % 2 != 0 {
                return Err(TextParseError::OddHexLineLength {
                    line: offset + 3,
                    observed: trimmed.len(),
                });
            }
            let mut bytes = trimmed.as_bytes().chunks_exact(2);
            for pair in &mut bytes {
                let high =
                    hex_nibble(pair[0]).ok_or_else(|| TextParseError::InvalidHexCharacter {
                        line: offset + 3,
                        character: pair[0] as char,
                    })?;
                let low =
                    hex_nibble(pair[1]).ok_or_else(|| TextParseError::InvalidHexCharacter {
                        line: offset + 3,
                        character: pair[1] as char,
                    })?;
                body.push((high << 4) | low);
            }
        }
        if body.len() != declared_bytes {
            return Err(TextParseError::DeclaredLengthMismatch {
                declared: declared_bytes,
                actual: body.len(),
            });
        }
        Program::from_wire(&body).map_err(|inner| TextParseError::WireDecodeFailed { inner })
    }
}

/// Hex-encode wire bytes into the canonical vyre IR text format.
///
/// Prepends the `vyre_ir v0.1` header and `wire_bytes N` line, then
/// writes the body as lowercase hex chunked at [`WIRE_BYTES_PER_LINE`].
#[inline]
#[must_use]
pub(crate) fn encode_text_body(bytes: &[u8]) -> String {
    let hex_chars = bytes.len() * 2;
    let line_count = bytes.len().div_ceil(WIRE_BYTES_PER_LINE);
    // header + wire_bytes line + body lines + trailing newline
    let capacity = TEXT_FORMAT_HEADER.len() + 32 + hex_chars + line_count + 1;
    let mut out = String::with_capacity(capacity);
    out.push_str(TEXT_FORMAT_HEADER);
    out.push('\n');
    out.push_str("wire_bytes ");
    push_usize(&mut out, bytes.len());
    out.push('\n');
    for chunk in bytes.chunks(WIRE_BYTES_PER_LINE) {
        for byte in chunk {
            push_hex_byte(&mut out, *byte);
        }
        out.push('\n');
    }
    out
}

/// Append a decimal `usize` to a `String` without allocating.
#[inline]
pub(crate) fn push_usize(out: &mut String, value: usize) {
    if value == 0 {
        out.push('0');
        return;
    }
    let mut digits = [0u8; 20];
    let mut idx = 0;
    let mut v = value;
    while v > 0 {
        let digit = u8::try_from(v % 10).map_or(0, |digit| digit);
        digits[idx] = b'0' + digit;
        v /= 10;
        idx += 1;
    }
    while idx > 0 {
        idx -= 1;
        out.push(digits[idx] as char);
    }
}

/// Append a byte as two lowercase hex characters.
#[inline]
pub(crate) fn push_hex_byte(out: &mut String, byte: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push(HEX[(byte >> 4) as usize] as char);
    out.push(HEX[(byte & 0x0f) as usize] as char);
}

/// Parse the `wire_bytes N` header line from the text format.
#[inline]
#[must_use]
pub(crate) fn parse_wire_bytes_line(line: &str) -> Result<usize, TextParseError> {
    let trimmed = line.trim_end_matches('\r');
    let Some(rest) = trimmed.strip_prefix("wire_bytes ") else {
        return Err(TextParseError::MissingWireBytesLine {
            observed: truncate(trimmed, 64),
        });
    };
    rest.parse::<usize>()
        .map_err(|_| TextParseError::MissingWireBytesLine {
            observed: truncate(trimmed, 64),
        })
}

/// Convert an ASCII hex digit to its numeric value.
#[inline]
#[must_use]
pub(crate) fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(10 + (byte - b'a')),
        b'A'..=b'F' => Some(10 + (byte - b'A')),
        _ => None,
    }
}

/// Truncate a string to `max` characters, appending an ellipsis if truncated.
#[inline]
#[must_use]
pub(crate) fn truncate(input: &str, max: usize) -> String {
    if input.chars().count() <= max {
        input.to_string()
    } else {
        let mut out = input.chars().take(max - 1).collect::<String>();
        out.push('…');
        out
    }
}
#[cfg(test)]
mod tests;
