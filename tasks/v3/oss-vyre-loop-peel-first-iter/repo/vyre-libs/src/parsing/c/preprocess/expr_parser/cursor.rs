use super::*;

impl PreprocessorExprParser<'_, '_, '_> {
    pub(super) fn consume_identifier_span(&mut self) -> Option<(usize, usize)> {
        self.skip_ws_and_splices();
        let start = self.index;
        let first = self.bytes.get(self.index).copied()?;
        if !is_c_ident_start(first) {
            return None;
        }
        self.index += 1;
        while self
            .bytes
            .get(self.index)
            .copied()
            .is_some_and(is_directive_ident_continue)
        {
            self.index += 1;
        }
        Some((start, self.index))
    }

    pub(super) fn consume_ident(&mut self, ident: &[u8]) -> bool {
        self.skip_ws_and_splices();
        let end = self.index.saturating_add(ident.len());
        if self.bytes.get(self.index..end) != Some(ident) {
            return false;
        }
        if self
            .bytes
            .get(end)
            .copied()
            .is_some_and(is_directive_ident_continue)
        {
            return false;
        }
        self.index = end;
        true
    }

    pub(super) fn consume_pair(&mut self, first: u8, second: u8) -> bool {
        if self.bytes.get(self.index..self.index + 2) == Some(&[first, second]) {
            self.index += 2;
            true
        } else {
            false
        }
    }

    pub(super) fn consume_byte(&mut self, byte: u8) -> bool {
        if self.bytes.get(self.index).copied() == Some(byte) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    pub(super) fn skip_ws_and_splices(&mut self) {
        loop {
            match self.bytes.get(self.index).copied() {
                Some(b' ' | b'\t' | b'\x0b' | b'\x0c' | b'\n' | b'\r') => self.index += 1,
                Some(b'\\') if self.bytes.get(self.index + 1).copied() == Some(b'\n') => {
                    self.index += 2;
                }
                Some(b'\\') if self.bytes.get(self.index + 1).copied() == Some(b'\r') => {
                    self.index += 2;
                    if self.bytes.get(self.index).copied() == Some(b'\n') {
                        self.index += 1;
                    }
                }
                Some(b'/') if self.bytes.get(self.index + 1).copied() == Some(b'/') => {
                    self.index += 2;
                    while !matches!(self.bytes.get(self.index), None | Some(b'\n' | b'\r')) {
                        self.index += 1;
                    }
                }
                Some(b'/') if self.bytes.get(self.index + 1).copied() == Some(b'*') => {
                    self.index += 2;
                    while self.index + 1 < self.bytes.len()
                        && self.bytes.get(self.index..self.index + 2) != Some(b"*/")
                    {
                        self.index += 1;
                    }
                    if self.index + 1 < self.bytes.len() {
                        self.index += 2;
                    }
                }
                _ => return,
            }
        }
    }

    pub(super) fn error(&self, message: &'static str) -> CPreprocessorError {
        CPreprocessorError {
            offset: self.base_offset + self.index,
            message,
        }
    }
}
