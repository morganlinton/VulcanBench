//! CPU reference lexer for the Rust nano-subset.

use crate::parsing::rust::lex::keyword::promote;
use crate::parsing::rust::lex::tokens::*;

/// A token with a source span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// Token kind id.
    pub kind: u16,
    /// Start byte offset in source.
    pub start: u32,
    /// Length in bytes.
    pub len: u16,
}

impl Token {
    /// Return the token's text with checked bounds and UTF-8 validation.
    pub fn try_text<'a>(&self, source: &'a [u8]) -> Result<&'a str, usize> {
        let start = usize::try_from(self.start).map_err(|_| source.len())?;
        let end = start.checked_add(usize::from(self.len)).ok_or(start)?;
        let span = source.get(start..end).ok_or(start)?;
        std::str::from_utf8(span).map_err(|error| start.saturating_add(error.valid_up_to()))
    }

    /// Return the token's text as a `&str` from the source buffer.
    #[cfg(test)]
    pub fn text<'a>(&self, source: &'a [u8]) -> &'a str {
        self.try_text(source)
            .expect("test token text should be addressable and valid UTF-8")
    }
}

/// Convert a scanned token byte length to the `u16` span field, failing closed
/// (returning the start offset as the error) if it exceeds `u16::MAX`. Without
/// this, a token of 65536+ bytes would wrap to a small `len` and make
/// `Token::text()` read a truncated, wrong span — a silent miscompile reachable
/// from a single oversized identifier or integer literal.
fn token_len(start: usize, end: usize) -> Result<u16, usize> {
    u16::try_from(end - start).map_err(|_| start)
}

/// Lex a byte slice into a vector of tokens.
pub fn lex(source: &[u8]) -> Result<Vec<Token>, usize> {
    let _ = std::str::from_utf8(source).map_err(|e| e.valid_up_to())?;
    // Span offsets are stored as `u32`; a source larger than `u32::MAX` would
    // silently truncate `start` and make every later span point at the wrong
    // bytes. Fail closed rather than miscompile an oversized input.
    if source.len() > u32::MAX as usize {
        return Err(u32::MAX as usize);
    }

    let mut tokens = Vec::new();
    let mut i = 0usize;

    while i < source.len() {
        let b = source[i];

        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        if b == b'/' && i + 1 < source.len() && source[i + 1] == b'/' {
            while i < source.len() && source[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if b == b'/' && i + 1 < source.len() && source[i + 1] == b'*' {
            i += 2;
            while i + 1 < source.len() {
                if source[i] == b'*' && source[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }

        let start = i;

        if b == b'_' || b.is_ascii_alphabetic() {
            while i < source.len() && (source[i].is_ascii_alphanumeric() || source[i] == b'_') {
                i += 1;
            }
            let text = match std::str::from_utf8(&source[start..i]) {
                Ok(text) => text,
                Err(_) => "",
            };
            let kind = promote(text).unwrap_or(IDENT);
            tokens.push(Token {
                kind,
                start: start as u32,
                len: token_len(start, i)?,
            });
            continue;
        }

        if b.is_ascii_digit() {
            while i < source.len() && source[i].is_ascii_digit() {
                i += 1;
            }
            tokens.push(Token {
                kind: LITERAL_INT,
                start: start as u32,
                len: token_len(start, i)?,
            });
            continue;
        }

        if i + 1 < source.len() {
            let pair = [b, source[i + 1]];
            let (kind, advance) = match &pair {
                b"==" => (EQ, 2),
                b"+=" => (PLUS_EQ, 2),
                b"-=" => (MINUS_EQ, 2),
                b"<=" => (LE, 2),
                b">=" => (GE, 2),
                b"!=" => (NE, 2),
                b"&&" => (ANDAND, 2),
                b"||" => (OROR, 2),
                b"->" => (ARROW, 2),
                b".." => (DOTDOT, 2),
                b"&m" if i + 4 <= source.len() && &source[i + 1..i + 4] == b"mut" => (AMP_MUT, 4),
                _ => (0, 0),
            };
            if advance > 0 {
                i += advance;
                tokens.push(Token {
                    kind,
                    start: start as u32,
                    len: advance as u16,
                });
                continue;
            }
        }

        let kind = match b {
            b'+' => PLUS,
            b'-' => MINUS,
            b'*' => STAR,
            b'/' => SLASH,
            b'%' => PERCENT,
            b'=' => ASSIGN,
            b'<' => LT,
            b'>' => GT,
            b';' => SEMI,
            b':' => COLON,
            b',' => COMMA,
            b'&' => AMP,
            b'!' => BANG,
            b'(' => LPAREN,
            b')' => RPAREN,
            b'{' => LBRACE,
            b'}' => RBRACE,
            _ => return Err(start),
        };
        i += 1;
        tokens.push(Token {
            kind,
            start: start as u32,
            len: 1,
        });
    }

    tokens.push(Token {
        kind: EOF,
        start: source.len() as u32,
        len: 0,
    });
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_identifier_fails_closed() {
        // A token of 65536+ bytes would wrap `len: as u16` to a tiny value and
        // make `Token::text()` read a truncated, wrong span (silent miscompile).
        // The lexer must instead reject it with the token's start offset.
        let src = "a".repeat(70_000);
        assert_eq!(
            lex(src.as_bytes()),
            Err(0),
            "70k-byte identifier must fail closed, not truncate"
        );
    }

    #[test]
    fn oversized_integer_literal_fails_closed() {
        let src = "1".repeat(70_000);
        assert_eq!(
            lex(src.as_bytes()),
            Err(0),
            "70k-byte int literal must fail closed"
        );
    }

    #[test]
    fn max_length_identifier_still_lexes_with_exact_len() {
        // u16::MAX bytes is the largest representable span and must succeed with
        // the exact length (boundary: the fix rejects only > u16::MAX).
        let n = u16::MAX as usize;
        let src = "a".repeat(n);
        let tokens = lex(src.as_bytes()).expect("u16::MAX-byte identifier must lex");
        assert_eq!(
            tokens[0].len as usize, n,
            "length must be exact, not truncated"
        );
        assert_eq!(
            tokens[0].text(src.as_bytes()).len(),
            n,
            "text() must read the full span"
        );
    }
}
