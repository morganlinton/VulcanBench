use super::*;

impl CObjectSecurityIndex {
    /// Return a token row by token index.
    #[must_use]
    pub fn token(&self, token_index: u32) -> Option<&CObjectToken> {
        self.lex.token(token_index)
    }

    /// Return the source bytes covered by a token index.
    #[must_use]
    pub fn token_bytes<'a>(&self, source: &'a [u8], token_index: u32) -> Option<&'a [u8]> {
        self.lex.token_bytes(source, token_index)
    }

    /// Return the UTF-8 source text covered by a token index.
    pub fn token_text<'a>(
        &self,
        source: &'a [u8],
        token_index: u32,
    ) -> Result<Option<&'a str>, String> {
        self.lex.token_text(source, token_index)
    }
}
