use super::*;

impl PreprocessorExprParser<'_, '_, '_> {
    pub(super) fn parse_has_x_operator(&mut self) -> Result<u64, CPreprocessorError> {
        self.skip_ws_and_splices();
        if !self.consume_byte(b'(') {
            return Err(self.error("Fix: __has_X operator requires a parenthesized argument"));
        }
        // Argument may be a single ident, or `namespace::attr` / `vendor::attr`
        // for __has_c_attribute / __has_cpp_attribute. Consume any sequence of
        // ident chars, "::", and surrounding whitespace until the matching `)`.
        self.skip_ws_and_splices();
        if !self.consume_string_literal()? {
            if self.consume_identifier_span().is_none() {
                return Err(
                    self.error("Fix: __has_X operator argument must be an identifier or string")
                );
            }
            loop {
                self.skip_ws_and_splices();
                if self.consume_pair(b':', b':') {
                    self.skip_ws_and_splices();
                    if self.consume_identifier_span().is_none() {
                        return Err(self.error(
                            "Fix: __has_X operator scoped name needs an identifier after ::",
                        ));
                    }
                } else {
                    break;
                }
            }
        }
        self.skip_ws_and_splices();
        if !self.consume_byte(b')') {
            return Err(self.error("Fix: close __has_X operator with ')'"));
        }
        Ok(0)
    }

    fn consume_string_literal(&mut self) -> Result<bool, CPreprocessorError> {
        self.skip_ws_and_splices();
        let start = self.index;
        if self.bytes.get(self.index..self.index + 2) == Some(b"u8") {
            self.index += 2;
        } else if matches!(self.bytes.get(self.index), Some(b'L' | b'u' | b'U')) {
            self.index += 1;
        }
        if !self.consume_byte(b'"') {
            self.index = start;
            return Ok(false);
        }
        loop {
            let Some(byte) = self.bytes.get(self.index).copied() else {
                return Err(self.error("Fix: terminate __has_X string argument"));
            };
            match byte {
                b'"' => {
                    self.index += 1;
                    return Ok(true);
                }
                b'\n' | b'\r' => {
                    return Err(self.error("Fix: close __has_X string argument before newline"));
                }
                b'\\' => {
                    self.index += 1;
                    if self.bytes.get(self.index).is_none() {
                        return Err(self.error("Fix: complete __has_X string escape"));
                    }
                    self.index += 1;
                }
                _ => self.index += 1,
            }
        }
    }

    pub(super) fn parse_has_builtin_operator(&mut self) -> Result<u64, CPreprocessorError> {
        self.skip_ws_and_splices();
        if !self.consume_byte(b'(') {
            return Err(
                self.error("Fix: __has_builtin operator requires a parenthesized builtin name")
            );
        }
        self.skip_ws_and_splices();
        let Some((start, end)) = self.consume_identifier_span() else {
            return Err(self.error("Fix: __has_builtin argument must be a builtin identifier"));
        };
        let present = crate::parsing::c::parse::gnu_builtins::try_classify_gnu_builtin_name(
            &self.bytes[start..end],
        )
        .ok()
        .flatten()
        .is_some();
        self.skip_ws_and_splices();
        if !self.consume_byte(b')') {
            return Err(self.error("Fix: close __has_builtin operator with ')'"));
        }
        Ok(u64::from(present))
    }

    /// Parse a clang `__has_include(<header>)` / `__has_include("header")` /
    /// `__has_include_next(...)` operator. Conservatively returns 0; the host
    /// source's `#else` branch executes.

    pub(super) fn parse_has_include_operator(&mut self) -> Result<u64, CPreprocessorError> {
        self.skip_ws_and_splices();
        if !self.consume_byte(b'(') {
            return Err(self.error("Fix: __has_include operator requires a parenthesized header"));
        }
        self.skip_ws_and_splices();
        let opener = self.bytes.get(self.index).copied();
        let closer = match opener {
            Some(b'<') => b'>',
            Some(b'"') => b'"',
            _ => return Err(self.error("Fix: __has_include header must be <name> or \"name\"")),
        };
        self.index += 1;
        let mut closed = false;
        while let Some(byte) = self.bytes.get(self.index).copied() {
            if byte == closer {
                self.index += 1;
                closed = true;
                break;
            }
            if matches!(byte, b'\n' | b'\r') {
                return Err(self.error("Fix: close __has_include header before newline"));
            }
            self.index += 1;
        }
        if !closed {
            return Err(self.error("Fix: close __has_include header before end of input"));
        }
        self.skip_ws_and_splices();
        if !self.consume_byte(b')') {
            return Err(self.error("Fix: close __has_include operator with ')'"));
        }
        Ok(0)
    }

    /// Parse a C23/GCC `__has_embed(<resource> parameters...)` operator.
    ///
    /// We conservatively return `0` (`__STDC_EMBED_NOT_FOUND__` semantics) so
    /// feature guards choose their compatibility branch unless a later stage
    /// wires real resource lookup. The important parity point here is syntax:
    /// the first operand may be include-like `<...>` / `"..."` or a macro
    /// identifier such as `__FILE__`, followed by optional `#embed` parameters
    /// with nested parentheses.
    pub(super) fn parse_has_embed_operator(&mut self) -> Result<u64, CPreprocessorError> {
        self.skip_ws_and_splices();
        if !self.consume_byte(b'(') {
            return Err(self.error("Fix: __has_embed operator requires parenthesized operands"));
        }
        self.skip_ws_and_splices();
        match self.bytes.get(self.index).copied() {
            Some(b'<') => {
                self.consume_angle_resource("Fix: close __has_embed resource with '>'")?
            }
            Some(b'"' | b'L' | b'u' | b'U') => {
                if !self.consume_string_literal()? {
                    return Err(self.error(
                        "Fix: __has_embed resource must be <name>, string literal, or identifier",
                    ));
                }
            }
            Some(b'_') | Some(b'a'..=b'z') | Some(b'A'..=b'Z') => {
                if self.consume_identifier_span().is_none() {
                    return Err(self.error("Fix: __has_embed resource identifier is malformed"));
                }
            }
            _ => {
                return Err(self.error(
                    "Fix: __has_embed resource must be <name>, string literal, or identifier",
                ));
            }
        }
        self.consume_balanced_tail_until_close_paren("Fix: close __has_embed operator with ')'")
    }

    fn consume_angle_resource(
        &mut self,
        eof_message: &'static str,
    ) -> Result<(), CPreprocessorError> {
        if !self.consume_byte(b'<') {
            return Err(self.error("Fix: expected '<' resource opener"));
        }
        while let Some(byte) = self.bytes.get(self.index).copied() {
            if byte == b'>' {
                self.index += 1;
                return Ok(());
            }
            if matches!(byte, b'\n' | b'\r') {
                return Err(self.error(eof_message));
            }
            self.index += 1;
        }
        Err(self.error(eof_message))
    }

    fn consume_balanced_tail_until_close_paren(
        &mut self,
        eof_message: &'static str,
    ) -> Result<u64, CPreprocessorError> {
        let mut depth = 0u32;
        loop {
            let Some(byte) = self.bytes.get(self.index).copied() else {
                return Err(self.error(eof_message));
            };
            match byte {
                b')' if depth == 0 => {
                    self.index += 1;
                    return Ok(0);
                }
                b')' => {
                    depth -= 1;
                    self.index += 1;
                }
                b'(' => {
                    depth = depth.saturating_add(1);
                    self.index += 1;
                }
                b'"' | b'\'' => self.consume_quoted_pp_token(byte, eof_message)?,
                b'\n' | b'\r' => return Err(self.error(eof_message)),
                _ => self.index += 1,
            }
        }
    }

    fn consume_quoted_pp_token(
        &mut self,
        quote: u8,
        eof_message: &'static str,
    ) -> Result<(), CPreprocessorError> {
        self.index += 1;
        loop {
            let Some(byte) = self.bytes.get(self.index).copied() else {
                return Err(self.error(eof_message));
            };
            match byte {
                b if b == quote => {
                    self.index += 1;
                    return Ok(());
                }
                b'\\' => {
                    self.index += 1;
                    if self.bytes.get(self.index).is_none() {
                        return Err(self.error(eof_message));
                    }
                    self.index += 1;
                }
                b'\n' | b'\r' => return Err(self.error(eof_message)),
                _ => self.index += 1,
            }
        }
    }

    /// Parse clang's `__is_identifier(name)` feature-test operator.
    ///
    /// Returns 0 for C keywords and common GNU/Clang extension keywords, 1
    /// for ordinary identifier spellings. That matches the compatibility-guard
    /// use case in real headers without pretending unsupported keywords are
    /// available as identifiers.
    pub(super) fn parse_is_identifier_operator(&mut self) -> Result<u64, CPreprocessorError> {
        self.skip_ws_and_splices();
        if !self.consume_byte(b'(') {
            return Err(
                self.error("Fix: __is_identifier operator requires a parenthesized identifier")
            );
        }
        self.skip_ws_and_splices();
        let Some((start, end)) = self.consume_identifier_span() else {
            return Err(self.error("Fix: __is_identifier argument must be an identifier"));
        };
        let ident = &self.bytes[start..end];
        self.skip_ws_and_splices();
        if !self.consume_byte(b')') {
            return Err(self.error("Fix: close __is_identifier operator with ')'"));
        }
        Ok(u64::from(!is_reserved_preprocessor_identifier(ident)))
    }

    pub(super) fn parse_defined_operator(&mut self) -> Result<u64, CPreprocessorError> {
        self.skip_ws_and_splices();
        let parenthesized = self.consume_byte(b'(');
        self.skip_ws_and_splices();
        let Some((start, end)) = self.consume_identifier_span() else {
            return Err(self.error("Fix: defined operator requires a macro identifier"));
        };
        self.skip_ws_and_splices();
        if parenthesized && !self.consume_byte(b')') {
            return Err(self.error("Fix: close defined(identifier) with ')'"));
        }
        Ok(u64::from(macro_is_defined(
            self.defined_macros,
            &self.bytes[start..end],
        )))
    }
}

/// Return true when `ident` is a C/GNU/Clang spelling that
/// `__is_identifier` must report as unavailable for ordinary identifier use.
pub fn is_reserved_preprocessor_identifier(ident: &[u8]) -> bool {
    matches!(
        ident,
        b"auto"
            | b"break"
            | b"case"
            | b"char"
            | b"const"
            | b"continue"
            | b"default"
            | b"do"
            | b"double"
            | b"else"
            | b"enum"
            | b"extern"
            | b"float"
            | b"for"
            | b"goto"
            | b"if"
            | b"inline"
            | b"int"
            | b"long"
            | b"register"
            | b"restrict"
            | b"return"
            | b"short"
            | b"signed"
            | b"sizeof"
            | b"static"
            | b"struct"
            | b"switch"
            | b"typedef"
            | b"union"
            | b"unsigned"
            | b"void"
            | b"volatile"
            | b"while"
            | b"_Alignas"
            | b"_Alignof"
            | b"_Atomic"
            | b"_Bool"
            | b"_Complex"
            | b"_Generic"
            | b"_Imaginary"
            | b"_Noreturn"
            | b"_Static_assert"
            | b"_Thread_local"
            | b"asm"
            | b"typeof"
            | b"typeof_unqual"
            | b"__asm"
            | b"__asm__"
            | b"__attribute"
            | b"__attribute__"
            | b"__auto_type"
            | b"__complex"
            | b"__complex__"
            | b"__const"
            | b"__const__"
            | b"__extension__"
            | b"__inline"
            | b"__inline__"
            | b"__int128"
            | b"__label__"
            | b"__restrict"
            | b"__restrict__"
            | b"__signed"
            | b"__signed__"
            | b"__thread"
            | b"__typeof"
            | b"__typeof__"
            | b"__volatile"
            | b"__volatile__"
    )
}
