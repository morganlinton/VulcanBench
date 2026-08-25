use super::*;

impl PreprocessorExprParser<'_, '_, '_> {
    pub(crate) fn parse(&mut self) -> Result<bool, CPreprocessorError> {
        let value = self.parse_conditional()?;
        self.skip_ws_and_splices();
        if self.index != self.bytes.len() {
            return Err(self.error("Fix: unsupported tokens remain in #if expression"));
        }
        Ok(value != 0)
    }
}
