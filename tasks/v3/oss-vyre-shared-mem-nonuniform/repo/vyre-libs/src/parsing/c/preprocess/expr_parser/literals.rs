use super::*;

impl PreprocessorExprParser<'_, '_, '_> {
    pub(super) fn consume_integer(&mut self) -> Option<u64> {
        self.skip_ws_and_splices();
        let start = self.index;
        let radix = if self.bytes.get(self.index..self.index + 2) == Some(b"0x")
            || self.bytes.get(self.index..self.index + 2) == Some(b"0X")
        {
            self.index += 2;
            16
        } else if self.bytes.get(self.index..self.index + 2) == Some(b"0b")
            || self.bytes.get(self.index..self.index + 2) == Some(b"0B")
        {
            self.index += 2;
            2
        } else if self.bytes.get(self.index).copied() == Some(b'0') {
            8
        } else {
            10
        };
        let digits_start = self.index;
        let mut value = 0u64;
        while let Some(byte) = self.bytes.get(self.index).copied() {
            if byte == b'\'' {
                let Some(next) = self.bytes.get(self.index + 1).copied() else {
                    break;
                };
                if digit_value_for_radix(next, radix).is_none() {
                    break;
                }
                self.index += 1;
                continue;
            }
            let digit = match byte {
                b'0'..=b'9' => u64::from(byte - b'0'),
                b'a'..=b'f' if radix == 16 => u64::from(byte - b'a' + 10),
                b'A'..=b'F' if radix == 16 => u64::from(byte - b'A' + 10),
                _ => break,
            };
            if digit >= radix {
                break;
            }
            value = value.saturating_mul(radix).saturating_add(digit);
            self.index += 1;
        }
        if self.index == digits_start {
            self.index = start;
            return None;
        }
        loop {
            if matches!(
                self.bytes.get(self.index),
                Some(b'u' | b'U' | b'l' | b'L' | b'z' | b'Z')
            ) {
                self.index += 1;
            } else if matches!(
                self.bytes.get(self.index..self.index + 2),
                Some(b"wb" | b"WB")
            ) {
                self.index += 2;
            } else {
                break;
            }
        }
        Some(value)
    }

    pub(super) fn consume_char_constant(&mut self) -> Result<Option<u64>, CPreprocessorError> {
        self.skip_ws_and_splices();
        let prefix_start = self.index;
        if self.bytes.get(self.index..self.index + 2) == Some(b"u8") {
            self.index += 2;
        } else if matches!(self.bytes.get(self.index), Some(b'L' | b'u' | b'U')) {
            self.index += 1;
        }
        if !self.consume_byte(b'\'') {
            self.index = prefix_start;
            return Ok(None);
        }

        let mut value = 0u64;
        let mut saw_character = false;
        loop {
            let Some(byte) = self.bytes.get(self.index).copied() else {
                return Err(self.error("Fix: terminate #if character constant"));
            };
            if byte == b'\'' {
                break;
            }
            if matches!(byte, b'\n' | b'\r') {
                return Err(self.error("Fix: close #if character constant before newline"));
            }
            let next_value = if self.consume_byte(b'\\') {
                self.consume_escape_value()?
            } else {
                self.index += 1;
                u64::from(byte)
            };
            value = value.wrapping_shl(8) | (next_value & 0xff);
            saw_character = true;
        }

        if !saw_character {
            return Err(
                self.error("Fix: #if character constant must contain at least one character")
            );
        }

        if !self.consume_byte(b'\'') {
            return Err(self.error("Fix: close #if character constant with single quote"));
        }
        Ok(Some(value))
    }

    pub(super) fn consume_escape_value(&mut self) -> Result<u64, CPreprocessorError> {
        let Some(byte) = self.bytes.get(self.index).copied() else {
            return Err(self.error("Fix: complete #if character escape"));
        };
        self.index += 1;
        let value = match byte {
            b'\'' => b'\'',
            b'"' => b'"',
            b'?' => b'?',
            b'\\' => b'\\',
            b'a' => 7,
            b'b' => 8,
            b'f' => 12,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'v' => 11,
            b'0'..=b'7' => {
                let mut value = u64::from(byte - b'0');
                let mut digits = 1u8;
                while digits < 3 {
                    let Some(next @ b'0'..=b'7') = self.bytes.get(self.index).copied() else {
                        break;
                    };
                    value = value * 8 + u64::from(next - b'0');
                    self.index += 1;
                    digits += 1;
                }
                return Ok(value);
            }
            b'x' => return self.consume_hex_escape(),
            b'u' => return self.consume_fixed_hex_escape(4),
            b'U' => return self.consume_fixed_hex_escape(8),
            other => other,
        };
        Ok(u64::from(value))
    }

    pub(super) fn consume_fixed_hex_escape(
        &mut self,
        digits: usize,
    ) -> Result<u64, CPreprocessorError> {
        let mut value = 0u64;
        for _ in 0..digits {
            let Some(byte) = self.bytes.get(self.index).copied() else {
                return Err(self.error("Fix: universal character escape is truncated"));
            };
            let digit = match byte {
                b'0'..=b'9' => u64::from(byte - b'0'),
                b'a'..=b'f' => u64::from(byte - b'a' + 10),
                b'A'..=b'F' => u64::from(byte - b'A' + 10),
                _ => return Err(self.error("Fix: universal character escape needs hex digits")),
            };
            value = value.saturating_mul(16).saturating_add(digit);
            self.index += 1;
        }
        Ok(value)
    }

    pub(super) fn consume_hex_escape(&mut self) -> Result<u64, CPreprocessorError> {
        let start = self.index;
        let mut value = 0u64;
        while let Some(byte) = self.bytes.get(self.index).copied() {
            let digit = match byte {
                b'0'..=b'9' => u64::from(byte - b'0'),
                b'a'..=b'f' => u64::from(byte - b'a' + 10),
                b'A'..=b'F' => u64::from(byte - b'A' + 10),
                _ => break,
            };
            value = value.saturating_mul(16).saturating_add(digit);
            self.index += 1;
        }
        if self.index == start {
            return Err(self.error("Fix: hex character escape needs at least one digit"));
        }
        Ok(value)
    }
}

fn digit_value_for_radix(byte: u8, radix: u64) -> Option<u64> {
    let digit = match byte {
        b'0'..=b'9' => u64::from(byte - b'0'),
        b'a'..=b'f' if radix == 16 => u64::from(byte - b'a' + 10),
        b'A'..=b'F' if radix == 16 => u64::from(byte - b'A' + 10),
        _ => return None,
    };
    (digit < radix).then_some(digit)
}
