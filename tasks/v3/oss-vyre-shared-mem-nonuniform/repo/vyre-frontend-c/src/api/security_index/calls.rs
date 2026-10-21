use super::*;

impl CObjectSecurityIndex {
    /// Return the token row for a function record's name.
    #[must_use]
    pub fn function_name_token(&self, function: CObjectFunctionRecord) -> Option<&CObjectToken> {
        self.token(function.name_token)
    }

    /// Return the token row for a call record's callee expression/name.
    #[must_use]
    pub fn call_callee_token(&self, call: CObjectCallRecord) -> Option<&CObjectToken> {
        self.token(call.callee_token)
    }

    /// Return the function name text when the source bytes are available.
    pub fn function_name_text<'a>(
        &self,
        source: &'a [u8],
        function: CObjectFunctionRecord,
    ) -> Result<Option<&'a str>, String> {
        self.token_text(source, function.name_token)
    }

    /// Return the call callee text when the source bytes are available.
    pub fn call_callee_text<'a>(
        &self,
        source: &'a [u8],
        call: CObjectCallRecord,
    ) -> Result<Option<&'a str>, String> {
        self.token_text(source, call.callee_token)
    }

    /// Return function records whose source name exactly matches `name`.
    pub fn functions_named(
        &self,
        source: &[u8],
        name: &str,
    ) -> Result<Vec<CObjectFunctionRecord>, String> {
        let mut matches = Vec::new();
        for function in &self.structure.functions {
            if self.function_name_text(source, *function)? == Some(name) {
                matches.push(*function);
            }
        }
        Ok(matches)
    }

    /// Return call records whose callee source text exactly matches `callee`.
    pub fn calls_named(
        &self,
        source: &[u8],
        callee: &str,
    ) -> Result<Vec<CObjectCallRecord>, String> {
        let mut matches = Vec::new();
        for call in &self.structure.calls {
            if self.call_callee_text(source, *call)? == Some(callee) {
                matches.push(*call);
            }
        }
        Ok(matches)
    }

    /// Iterate function records paired with their name-token source spans.
    pub fn functions_with_tokens(
        &self,
    ) -> impl Iterator<Item = (CObjectFunctionRecord, Option<&CObjectToken>)> + '_ {
        self.structure
            .functions
            .iter()
            .copied()
            .map(|function| (function, self.function_name_token(function)))
    }

    /// Iterate call records paired with their callee-token source spans.
    pub fn calls_with_tokens(
        &self,
    ) -> impl Iterator<Item = (CObjectCallRecord, Option<&CObjectToken>)> + '_ {
        self.structure
            .calls
            .iter()
            .copied()
            .map(|call| (call, self.call_callee_token(call)))
    }

    /// Iterate call records whose caller id matches a function-record index.
    pub fn calls_in_function(
        &self,
        function_index: u32,
    ) -> impl Iterator<Item = CObjectCallRecord> + '_ {
        self.structure
            .calls
            .iter()
            .copied()
            .filter(move |call| call.caller_id == function_index)
    }

    /// Iterate call records joined to valid caller function records.
    pub fn call_edges(&self) -> impl Iterator<Item = CObjectFunctionCall<'_>> + '_ {
        self.structure.calls.iter().filter_map(move |call| {
            let caller_index = usize::try_from(call.caller_id).ok()?;
            let caller = self.structure.functions.get(caller_index)?;
            Some(CObjectFunctionCall {
                caller_index: call.caller_id,
                caller,
                call,
                callee: self.call_callee_token(*call),
            })
        })
    }

    /// Iterate resolved calls for one function-record index.
    pub fn call_edges_for_function(
        &self,
        function_index: u32,
    ) -> impl Iterator<Item = CObjectFunctionCall<'_>> + '_ {
        self.call_edges()
            .filter(move |edge| edge.caller_index == function_index)
    }

    /// Iterate call records whose caller id does not resolve to a function.
    pub fn unresolved_caller_calls(&self) -> impl Iterator<Item = CObjectCallRecord> + '_ {
        self.structure.calls.iter().copied().filter(|call| {
            call.caller_id == u32::MAX
                || usize::try_from(call.caller_id)
                    .ok()
                    .and_then(|index| self.structure.functions.get(index))
                    .is_none()
        })
    }

    /// Return the function record whose body token span contains `token_index`.
    #[must_use]
    pub fn function_containing_token(
        &self,
        token_index: u32,
    ) -> Option<(u32, &CObjectFunctionRecord)> {
        self.structure
            .functions
            .iter()
            .enumerate()
            .find(|(_, function)| {
                function.body_start_token <= token_index && token_index <= function.body_end_token
            })
            .and_then(|(index, function)| u32::try_from(index).ok().map(|index| (index, function)))
    }

    /// then token containment when the caller id is absent.
    #[must_use]
    pub fn enclosing_function_for_call(
        &self,
        call: CObjectCallRecord,
    ) -> Option<(u32, &CObjectFunctionRecord)> {
        self.structure
            .functions
            .get(usize::try_from(call.caller_id).ok()?)
            .map(|function| (call.caller_id, function))
            .or_else(|| self.function_containing_token(call.callee_token))
    }
}
