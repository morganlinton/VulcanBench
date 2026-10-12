use super::*;

impl CObjectSecurityIndex {
    /// Iterate function definition/declaration symbols.
    pub fn function_symbols(&self) -> impl Iterator<Item = CObjectSymbolRef> + '_ {
        self.sema_scope
            .symbols()
            .filter(|symbol| matches!(symbol.decl_kind_name, "function" | "function_decl"))
    }

    /// Iterate variable declaration symbols.
    pub fn variable_symbols(&self) -> impl Iterator<Item = CObjectSymbolRef> + '_ {
        self.sema_scope
            .symbols()
            .filter(|symbol| symbol.decl_kind_name == "variable")
    }

    /// Iterate typedef declaration symbols.
    pub fn typedef_symbols(&self) -> impl Iterator<Item = CObjectSymbolRef> + '_ {
        self.sema_scope
            .symbols()
            .filter(|symbol| symbol.decl_kind_name == "typedef")
    }

    /// Iterate all symbol references joined to source tokens.
    pub fn symbols_with_tokens(&self) -> impl Iterator<Item = CObjectSymbolToken<'_>> + '_ {
        self.sema_scope.symbols().map(|symbol| CObjectSymbolToken {
            symbol,
            token: self.symbol_token(symbol),
        })
    }

    /// Iterate declaration symbols joined to source tokens.
    pub fn declarations_with_tokens(&self) -> impl Iterator<Item = CObjectSymbolToken<'_>> + '_ {
        self.symbols_with_tokens()
            .filter(|entry| entry.symbol.decl_kind_name != "none")
    }

    /// Return the source span token for a symbol reference.
    #[must_use]
    pub fn symbol_token(&self, symbol: CObjectSymbolRef) -> Option<&CObjectToken> {
        let idx = self
            .lex
            .tokens
            .binary_search_by_key(&symbol.token_start, |token| token.start)
            .ok()?;
        self.lex
            .tokens
            .get(idx)
            .filter(|token| token.len == symbol.token_len)
    }

    /// Return symbol source text when the original source bytes are available.
    pub fn symbol_text<'a>(
        &self,
        source: &'a [u8],
        symbol: CObjectSymbolRef,
    ) -> Result<Option<&'a str>, String> {
        let Some(token) = self.symbol_token(symbol) else {
            return Ok(None);
        };
        let Some(range) = token.byte_range() else {
            return Ok(None);
        };
        let Some(bytes) = source.get(range) else {
            return Ok(None);
        };
        std::str::from_utf8(bytes)
            .map(Some)
            .map_err(|error| format!("vyre-frontend-c symbol is not UTF-8: {error}"))
    }

    /// Return declaration symbols whose source text exactly matches `name`.
    pub fn declarations_named(
        &self,
        source: &[u8],
        name: &str,
    ) -> Result<Vec<CObjectSymbolRef>, String> {
        let mut matches = Vec::new();
        for entry in self.declarations_with_tokens() {
            if self.symbol_text(source, entry.symbol)? == Some(name) {
                matches.push(entry.symbol);
            }
        }
        Ok(matches)
    }
}
