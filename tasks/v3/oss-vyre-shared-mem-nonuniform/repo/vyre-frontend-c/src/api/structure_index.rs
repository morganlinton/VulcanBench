use std::path::Path;

use super::object_io::{decode_embedded_object, read_object_file};
use super::word_decode::decode_u32_words_for_section;
use crate::object_format::{SectionTag, Vyrecob2};

/// Decoded function and call-site records from a `vyre-frontend-c` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CObjectStructureIndex {
    /// VYRECOB2 container version.
    pub vyrecob2_version: u32,
    /// Function records emitted by `c11_extract_functions`.
    pub functions: Vec<CObjectFunctionRecord>,
    /// Call-site records emitted by `c11_extract_calls`.
    pub calls: Vec<CObjectCallRecord>,
}

/// One C function record: function-name token and body brace span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CObjectFunctionRecord {
    /// Token index of the function identifier.
    pub name_token: u32,
    /// Token index of the opening body brace.
    pub body_start_token: u32,
    /// Token index of the closing body brace.
    pub body_end_token: u32,
}

/// One call-site record: caller id plus callee/argument token span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CObjectCallRecord {
    /// Function-record index of the enclosing caller, or `u32::MAX` when unknown.
    pub caller_id: u32,
    /// Token index of the callee expression/name.
    pub callee_token: u32,
    /// Token index of the opening argument parenthesis.
    pub args_start_token: u32,
    /// Token index of the closing argument parenthesis.
    pub args_end_token: u32,
}

/// Decode function and call-site records from object bytes.
pub fn decode_object_structure_index(object_bytes: &[u8]) -> Result<CObjectStructureIndex, String> {
    decode_embedded_object(object_bytes, decode_object_structure_index_from_container)
}

pub(crate) fn decode_object_structure_index_from_container(
    container: &Vyrecob2<'_>,
) -> Result<CObjectStructureIndex, String> {
    let functions_section = container.section(SectionTag::Functions).ok_or_else(|| {
        "vyre-frontend-c object is missing Functions. Fix: compile with structure extraction enabled."
            .to_string()
    })?;
    let calls_section = container.section(SectionTag::Calls).ok_or_else(|| {
        "vyre-frontend-c object is missing Calls. Fix: compile with call extraction enabled."
            .to_string()
    })?;
    let functions = decode_function_records(functions_section)?;
    let calls = decode_call_records(calls_section)?;
    validate_structure_index(&functions, &calls)?;
    Ok(CObjectStructureIndex {
        vyrecob2_version: container.version,
        functions,
        calls,
    })
}

/// Read and decode function/call records from an object path.
pub fn decode_object_structure_index_file(path: &Path) -> Result<CObjectStructureIndex, String> {
    read_object_file(path, decode_object_structure_index)
}

fn decode_function_records(section: &[u8]) -> Result<Vec<CObjectFunctionRecord>, String> {
    let words = decode_u32_words_for_section(section, "Functions")?;
    if words.len() % 3 != 0 {
        return Err(format!(
            "vyre-frontend-c Functions section has {} u32 words, not whole 3-word records. Fix: regenerate the object.",
            words.len()
        ));
    }
    Ok(words
        .chunks_exact(3)
        .enumerate()
        .map(|(idx, row)| {
            if row[1] > row[2] {
                return Err(format!(
                    "vyre-frontend-c Functions record {idx} has body_start_token {} after body_end_token {}. Fix: regenerate the object with ordered function spans.",
                    row[1], row[2]
                ));
            }
            Ok(CObjectFunctionRecord {
                name_token: row[0],
                body_start_token: row[1],
                body_end_token: row[2],
            })
        })
        .collect::<Result<Vec<_>, _>>()?)
}

fn decode_call_records(section: &[u8]) -> Result<Vec<CObjectCallRecord>, String> {
    let words = decode_u32_words_for_section(section, "Calls")?;
    if words.len() % 4 != 0 {
        return Err(format!(
            "vyre-frontend-c Calls section has {} u32 words, not whole 4-word records. Fix: regenerate the object.",
            words.len()
        ));
    }
    Ok(words
        .chunks_exact(4)
        .enumerate()
        .map(|(idx, row)| {
            if row[2] > row[3] {
                return Err(format!(
                    "vyre-frontend-c Calls record {idx} has args_start_token {} after args_end_token {}. Fix: regenerate the object with ordered call spans.",
                    row[2], row[3]
                ));
            }
            Ok(CObjectCallRecord {
                caller_id: row[0],
                callee_token: row[1],
                args_start_token: row[2],
                args_end_token: row[3],
            })
        })
        .collect::<Result<Vec<_>, _>>()?)
}

fn validate_structure_index(
    functions: &[CObjectFunctionRecord],
    calls: &[CObjectCallRecord],
) -> Result<(), String> {
    for (idx, call) in calls.iter().enumerate() {
        if call.caller_id == u32::MAX {
            continue;
        }
        let caller = usize::try_from(call.caller_id).map_err(|_| {
            format!(
                "vyre-frontend-c Calls record {idx} caller id {} exceeds usize. Fix: regenerate the object with bounded caller ids.",
                call.caller_id
            )
        })?;
        if caller >= functions.len() {
            return Err(format!(
                "vyre-frontend-c Calls record {idx} references caller id {caller}, but only {} function records decoded. Fix: regenerate the object with valid caller ids.",
                functions.len()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::decode_object_structure_index;
    use crate::object_format::{serialize_vyrecob2, SectionTag};

    #[test]
    fn decodes_function_and_call_records() {
        let function_words = [3u32, 10, 30];
        let call_words = [0u32, 14, 15, 18];
        let functions = pack_words(&function_words);
        let calls = pack_words(&call_words);
        let object = serialize_vyrecob2(&[
            (SectionTag::Functions, functions.as_slice()),
            (SectionTag::Calls, calls.as_slice()),
        ])
        .expect("Fix: fixture object must serialize");

        let decoded = decode_object_structure_index(&object).expect("Fix: fixture must decode");
        assert_eq!(decoded.functions.len(), 1);
        assert_eq!(decoded.functions[0].name_token, 3);
        assert_eq!(decoded.functions[0].body_start_token, 10);
        assert_eq!(decoded.functions[0].body_end_token, 30);
        assert_eq!(decoded.calls.len(), 1);
        assert_eq!(decoded.calls[0].caller_id, 0);
        assert_eq!(decoded.calls[0].callee_token, 14);
        assert_eq!(decoded.calls[0].args_start_token, 15);
        assert_eq!(decoded.calls[0].args_end_token, 18);
    }

    #[test]
    fn rejects_partial_rows() {
        let bad_functions = pack_words(&[1u32, 2]);
        let calls = pack_words(&[0u32, 1, 2, 3]);
        let object = serialize_vyrecob2(&[
            (SectionTag::Functions, bad_functions.as_slice()),
            (SectionTag::Calls, calls.as_slice()),
        ])
        .expect("Fix: fixture object must serialize");

        let err = decode_object_structure_index(&object)
            .expect_err("partial function row must be rejected");
        assert!(err.contains("3-word records"));
    }

    #[test]
    fn rejects_unordered_function_span() {
        let functions = pack_words(&[3u32, 30, 10]);
        let calls = Vec::new();
        let object = serialize_vyrecob2(&[
            (SectionTag::Functions, functions.as_slice()),
            (SectionTag::Calls, calls.as_slice()),
        ])
        .expect("Fix: fixture object must serialize");
        let err = decode_object_structure_index(&object)
            .expect_err("unordered function span must be rejected");
        assert!(err.contains("body_start_token"));
    }

    #[test]
    fn rejects_call_with_missing_caller() {
        let functions = pack_words(&[3u32, 10, 30]);
        let calls = pack_words(&[7u32, 14, 15, 18]);
        let object = serialize_vyrecob2(&[
            (SectionTag::Functions, functions.as_slice()),
            (SectionTag::Calls, calls.as_slice()),
        ])
        .expect("Fix: fixture object must serialize");
        let err =
            decode_object_structure_index(&object).expect_err("missing caller id must be rejected");
        assert!(err.contains("only 1 function records decoded"));
    }

    use vyre_primitives::wire::pack_u32_slice as pack_words;
}
