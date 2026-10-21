use crate::{dual_impls::common, workgroup::Memory};
use vyre_primitives::matching::CompiledDfa;
use vyre_primitives::PatternMatchDfa;

impl common::ReferenceEvaluator for PatternMatchDfa {
    fn evaluate(&self, inputs: &[Memory]) -> Result<Memory, common::EvalError> {
        let haystack = common::one_input(inputs, "scan_dfa")?;
        // Decode using the canonical V2 wire format produced by CompiledDfa::to_bytes.
        // The old hand-rolled V1 parser (magic + state_count + start + accept_count)
        // does not match the V2 envelope (magic + version + state_count + max_pattern_len
        // + length-prefixed sections). Using from_bytes here keeps the reference oracle
        // byte-identical with every other consumer of the DFA wire format.
        let compiled = CompiledDfa::from_bytes(&self.dfa).map_err(|e| {
            common::EvalError::new(format!(
                "primitive `scan_dfa` could not decode DFA wire blob: {e}. \
                 Fix: populate PatternMatchDfa.dfa via CompiledDfa::to_bytes()."
            ))
        })?;

        // State 0 is always the root/start state in the Aho-Corasick DFA produced
        // by dfa_compile. There is no separate start field in the V2 format.
        let mut state = 0usize;
        let mut offsets = Vec::new();
        for (offset, byte) in haystack.iter().copied().enumerate() {
            let next_state_idx = state * 256 + usize::from(byte);
            let next = compiled.transitions[next_state_idx] as usize;
            if next >= compiled.state_count as usize {
                return Err(common::EvalError::new(
                    "primitive `scan_dfa` transition targets an out-of-range state. \
                     Fix: validate every transition target in the DFA.",
                ));
            }
            state = next;
            // accept[state] is non-zero when the state matches at least one pattern.
            if compiled.accept[state] != 0 {
                offsets.push(u32::try_from(offset).map_err(|_| {
                    common::EvalError::new(
                        "primitive `scan_dfa` offset exceeds u32. Fix: split haystacks before 4 GiB.",
                    )
                })?);
            }
        }
        Ok(common::write_u32s(offsets))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dual_impls::common::ReferenceEvaluator;
    use vyre_primitives::matching::dfa_compile;
    use crate::workgroup::Memory;

    /// Verifies that the reference evaluator correctly decodes a V2 wire blob
    /// and finds the pattern at the expected offset. Before the fix the parser
    /// read a V1 layout (misaligned fields) and would return a length-mismatch
    /// error or wrong offsets for every real V2 DFA.
    #[test]
    fn test_dfa_reference_v2_roundtrip() {
        // Build a real V2 DFA for pattern "abc".
        let compiled = dfa_compile(&[b"abc"]);
        let wire_bytes = compiled
            .to_bytes()
            .expect("Fix: dfa_compile must produce a serializable DFA");

        let primitive = PatternMatchDfa { dfa: wire_bytes };
        // Haystack: "xxabcxx" — pattern starts at byte 2, ends (accepting) at byte 4.
        let haystack = Memory::from_bytes(b"xxabcxx".to_vec());
        let result = primitive
            .evaluate(&[haystack])
            .expect("Fix: V2 DFA roundtrip must succeed on valid haystack");

        // The evaluator records the offset of the accepting byte (offset 4, 0-indexed).
        let offsets: Vec<u32> = result
            .bytes()
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        // "abc" completes at index 4 (x=0,x=1,a=2,b=3,c=4).
        assert_eq!(
            offsets,
            vec![4u32],
            "Fix: V2 DFA reference evaluator must report offset 4 for 'abc' in 'xxabcxx'"
        );
    }
}
