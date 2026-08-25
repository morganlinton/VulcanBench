use super::*;

impl PreprocessorExprParser<'_, '_, '_> {
    pub(super) fn parse_conditional(&mut self) -> Result<u64, CPreprocessorError> {
        // Depth-guard the ternary recursion (and, transitively via parens in
        // `parse_unary`, all parenthesized nesting). Fail closed before the
        // native stack overflows on hostile input like `#if 1?1:1?1:...`.
        self.depth += 1;
        let r = if self.depth > MAX_PP_EXPR_DEPTH {
            Err(self.error("Fix: #if expression nesting too deep"))
        } else {
            self.parse_conditional_inner()
        };
        self.depth -= 1;
        r
    }

    fn parse_conditional_inner(&mut self) -> Result<u64, CPreprocessorError> {
        let condition = self.parse_logical_or()?;
        self.skip_ws_and_splices();
        if !self.consume_byte(b'?') {
            return Ok(condition);
        }

        let then_value = self.parse_conditional()?;
        self.skip_ws_and_splices();
        if !self.consume_byte(b':') {
            return Err(self.error("Fix: close #if conditional operator with ':'"));
        }
        let else_value = self.parse_conditional()?;
        Ok(if condition != 0 {
            then_value
        } else {
            else_value
        })
    }

    pub(super) fn parse_logical_or(&mut self) -> Result<u64, CPreprocessorError> {
        let mut value = self.parse_logical_and()?;
        loop {
            self.skip_ws_and_splices();
            if !self.consume_pair(b'|', b'|') {
                return Ok(value);
            }
            let rhs = self.parse_logical_and()?;
            value = u64::from(value != 0 || rhs != 0);
        }
    }

    pub(super) fn parse_logical_and(&mut self) -> Result<u64, CPreprocessorError> {
        let mut value = self.parse_bitwise_or()?;
        loop {
            self.skip_ws_and_splices();
            if !self.consume_pair(b'&', b'&') {
                return Ok(value);
            }
            let rhs = self.parse_bitwise_or()?;
            value = u64::from(value != 0 && rhs != 0);
        }
    }

    pub(super) fn parse_bitwise_or(&mut self) -> Result<u64, CPreprocessorError> {
        let mut value = self.parse_bitwise_xor()?;
        loop {
            self.skip_ws_and_splices();
            if self.consume_pair(b'|', b'|') {
                self.index = self.index.saturating_sub(2);
                return Ok(value);
            }
            if !self.consume_byte(b'|') {
                return Ok(value);
            }
            value |= self.parse_bitwise_xor()?;
        }
    }

    pub(super) fn parse_bitwise_xor(&mut self) -> Result<u64, CPreprocessorError> {
        let mut value = self.parse_bitwise_and()?;
        loop {
            self.skip_ws_and_splices();
            if !self.consume_byte(b'^') {
                return Ok(value);
            }
            value ^= self.parse_bitwise_and()?;
        }
    }

    pub(super) fn parse_bitwise_and(&mut self) -> Result<u64, CPreprocessorError> {
        let mut value = self.parse_equality()?;
        loop {
            self.skip_ws_and_splices();
            if self.consume_pair(b'&', b'&') {
                self.index = self.index.saturating_sub(2);
                return Ok(value);
            }
            if !self.consume_byte(b'&') {
                return Ok(value);
            }
            value &= self.parse_equality()?;
        }
    }

    pub(super) fn parse_equality(&mut self) -> Result<u64, CPreprocessorError> {
        let mut value = self.parse_relational()?;
        loop {
            self.skip_ws_and_splices();
            if self.consume_pair(b'=', b'=') {
                value = u64::from(value == self.parse_relational()?);
            } else if self.consume_pair(b'!', b'=') {
                value = u64::from(value != self.parse_relational()?);
            } else {
                return Ok(value);
            }
        }
    }

    pub(super) fn parse_relational(&mut self) -> Result<u64, CPreprocessorError> {
        let mut value = self.parse_shift()?;
        loop {
            self.skip_ws_and_splices();
            if self.consume_pair(b'<', b'=') {
                value = u64::from(value <= self.parse_shift()?);
            } else if self.consume_pair(b'>', b'=') {
                value = u64::from(value >= self.parse_shift()?);
            } else if self.consume_byte(b'<') {
                value = u64::from(value < self.parse_shift()?);
            } else if self.consume_byte(b'>') {
                value = u64::from(value > self.parse_shift()?);
            } else {
                return Ok(value);
            }
        }
    }

    pub(super) fn parse_shift(&mut self) -> Result<u64, CPreprocessorError> {
        let mut value = self.parse_additive()?;
        loop {
            self.skip_ws_and_splices();
            if self.consume_pair(b'<', b'<') {
                let rhs = self.parse_additive()?;
                value = value.checked_shl(rhs.min(127) as u32).unwrap_or(0);
            } else if self.consume_pair(b'>', b'>') {
                let rhs = self.parse_additive()?;
                value = value.checked_shr(rhs.min(127) as u32).unwrap_or(0);
            } else {
                return Ok(value);
            }
        }
    }

    pub(super) fn parse_additive(&mut self) -> Result<u64, CPreprocessorError> {
        let mut value = self.parse_multiplicative()?;
        loop {
            self.skip_ws_and_splices();
            if self.consume_byte(b'+') {
                value = value.wrapping_add(self.parse_multiplicative()?);
            } else if self.consume_byte(b'-') {
                value = value.wrapping_sub(self.parse_multiplicative()?);
            } else {
                return Ok(value);
            }
        }
    }

    pub(super) fn parse_multiplicative(&mut self) -> Result<u64, CPreprocessorError> {
        let mut value = self.parse_unary()?;
        loop {
            self.skip_ws_and_splices();
            if self.consume_byte(b'*') {
                value = value.wrapping_mul(self.parse_unary()?);
            } else if self.consume_byte(b'/') {
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(self.error("Fix: #if expression divides by zero"));
                }
                value /= rhs;
            } else if self.consume_byte(b'%') {
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err(self.error("Fix: #if expression takes modulo by zero"));
                }
                value %= rhs;
            } else {
                return Ok(value);
            }
        }
    }

    pub(super) fn parse_unary(&mut self) -> Result<u64, CPreprocessorError> {
        // `! ~ + -` chains right-recurse here and parens route back to
        // `parse_conditional`; guard on the shared depth counter so neither
        // path can overflow the native stack.
        self.depth += 1;
        let r = if self.depth > MAX_PP_EXPR_DEPTH {
            Err(self.error("Fix: #if expression nesting too deep"))
        } else {
            self.parse_unary_inner()
        };
        self.depth -= 1;
        r
    }

    fn parse_unary_inner(&mut self) -> Result<u64, CPreprocessorError> {
        self.skip_ws_and_splices();
        if self.consume_byte(b'!') {
            return Ok(u64::from(self.parse_unary()? == 0));
        }
        if self.consume_byte(b'~') {
            return Ok(!self.parse_unary()?);
        }
        if self.consume_byte(b'+') {
            return self.parse_unary();
        }
        if self.consume_byte(b'-') {
            return Ok(self.parse_unary()?.wrapping_neg());
        }
        if self.consume_byte(b'(') {
            let value = self.parse_conditional()?;
            self.skip_ws_and_splices();
            if !self.consume_byte(b')') {
                return Err(self.error("Fix: close parenthesized #if expression with ')'"));
            }
            return Ok(value);
        }
        if self.consume_ident(b"defined") {
            return self.parse_defined_operator();
        }
        // clang feature-test operators. Real C code (glibc, kernel, OpenSSL,
        // most defensive headers) gates declarations on `__has_attribute(...)`,
        // `__has_builtin(...)`, etc. We must at minimum recognize the syntax
        // so the directive parses; conservatively we return 0 ("not
        // supported") so the host source falls through to its compatibility
        // path. `__has_include` is special-cased because its argument is a
        // `<header>` / `"header"` literal, not an identifier.
        if self.consume_ident(b"__has_include") || self.consume_ident(b"__has_include_next") {
            return self.parse_has_include_operator();
        }
        if self.consume_ident(b"__has_embed") {
            return self.parse_has_embed_operator();
        }
        if self.consume_ident(b"__has_builtin") || self.consume_ident(b"__has_constexpr_builtin") {
            return self.parse_has_builtin_operator();
        }
        if self.consume_ident(b"__is_identifier") {
            return self.parse_is_identifier_operator();
        }
        if self.consume_ident(b"__has_attribute")
            || self.consume_ident(b"__has_feature")
            || self.consume_ident(b"__has_extension")
            || self.consume_ident(b"__has_warning")
            || self.consume_ident(b"__has_c_attribute")
            || self.consume_ident(b"__has_cpp_attribute")
            || self.consume_ident(b"__has_declspec_attribute")
        {
            return self.parse_has_x_operator();
        }
        if let Some(value) = self.consume_char_constant()? {
            return Ok(value);
        }
        if let Some(value) = self.consume_integer() {
            return Ok(value);
        }
        if let Some((start, end)) = self.consume_identifier_span() {
            return Ok(u64::from(macro_is_defined(
                self.defined_macros,
                &self.bytes[start..end],
            )));
        }
        Err(self.error("Fix: expected #if operand, integer literal, identifier, or defined()"))
    }
}
