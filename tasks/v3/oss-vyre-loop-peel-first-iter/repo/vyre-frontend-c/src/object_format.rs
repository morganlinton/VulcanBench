//! Multi-section container for GPU compiler artifacts (`VYRECOB2` payloads).
//!
//! Forward-compatible: older readers skip unknown `SectionTag` values via length fields.

/// File magic: `VYREC02\0`
pub const VYRECOB2_MAGIC: &[u8; 8] = b"VYREC02\0";
/// Bumped when new sections are added; still uses the same magic.
pub const VYRECOB2_VERSION: u32 = 7;

/// Discriminant of the per-section payload kind in a `VYRECOB2` container.
///
/// The integer value is the on-disk tag; readers MUST round-trip unknown tags
/// using their length prefix so older readers can skip newer payloads.
#[repr(u32)]
#[derive(Clone, Copy, Debug)]
pub enum SectionTag {
    /// Token type / start / length streams (the GPU lex output).
    Lex = 1,
    /// Paren-balanced span table.
    ParenPairs = 2,
    /// Brace-balanced span table.
    BracePairs = 3,
    /// Function shape table emitted from the AST.
    Functions = 4,
    /// Call-site table emitted from the AST.
    Calls = 5,
    /// Embedded Linux ET_REL `.o` payload.
    Elf = 6,
    /// `opt_conditional_mask` output (u32 per token).
    PreprocMask = 7,
    /// `opt_dynamic_macro_expansion` token stream (types buffer).
    MacroTypes = 8,
    /// `c11_compute_alignments` (`sizes` || `aligns`).
    AbiLayout = 9,
    /// Optional ABI type-kind rows parallel to `AbiLayout`.
    AbiTypes = 19,
    /// `ast_shunting_yard` flat AST pool + roots (concatenated blobs).
    Ast = 10,
    /// `c11_build_cfg_and_gotos` public outputs (`cfg` || `labels`);
    /// scratch hash tables are intentionally not serialized.
    Cfg = 11,
    /// `vyre_runtime::megakernel::protocol` fingerprint (fixed header).
    Megakernel = 12,
    /// Token-level VAST node table emitted by the C parser.
    Vast = 13,
    /// ProgramGraph node rows lowered from VAST.
    ProgramGraph = 14,
    /// `c_sema_scope` records: scope id, parent scope id, declaration kind,
    /// identifier id, token start, token length.
    SemaScope = 15,
    /// `c11_build_expression_shape_nodes` rows derived from raw + typed VAST.
    ExpressionShape = 16,
    /// Semantic ProgramGraph node rows: base PG fields plus category, role, and attributes.
    SemanticProgramGraphNodes = 17,
    /// Semantic ProgramGraph edge rows, including resolved expression/statement control edges.
    SemanticProgramGraphEdges = 18,
}

/// Append a single tagged section (`u32` tag, `u32` payload length, payload bytes) to `out`.
pub fn push_section(out: &mut Vec<u8>, tag: SectionTag, payload: &[u8]) -> Result<(), String> {
    out.extend_from_slice(&(tag as u32).to_le_bytes());
    let section_len = u32::try_from(payload.len()).map_err(|_| {
        format!(
            "section `{tag:?}` length {} exceeds u32::MAX. Fix: split this vyre-frontend-c object section.",
            payload.len()
        )
    })?;
    out.extend_from_slice(&section_len.to_le_bytes());
    out.extend_from_slice(payload);
    Ok(())
}

/// Build a self-contained `VYRECOB1` lex blob for `source_path` from the type/start/length
/// streams emitted by the GPU C lexer.
///
/// Returns an error if `types`, `starts`, or `lens` is shorter than `n_tokens` u32 words.
pub fn build_vyrecob1_lex_section(
    source_path: &std::path::Path,
    types: &[u8],
    starts: &[u8],
    lens: &[u8],
    n_tokens: u32,
) -> Result<Vec<u8>, String> {
    let n = n_tokens as usize;
    let stream_bytes = n.checked_mul(4).ok_or_else(|| {
        format!(
            "VYRECOB1 token stream byte length overflows usize for {n_tokens} tokens. Fix: shard this translation unit."
        )
    })?;
    require_prefix(types, stream_bytes, "token type stream")?;
    require_prefix(starts, stream_bytes, "token start stream")?;
    require_prefix(lens, stream_bytes, "token length stream")?;

    let path = source_path.to_str().ok_or_else(|| {
        format!(
            "source path `{}` is not valid UTF-8. Fix: compile from a UTF-8 path so object metadata can round-trip exactly.",
            source_path.display()
        )
    })?;
    let p = path.as_bytes();
    let header_len = 8usize
        .checked_add(4)
        .and_then(|len| len.checked_add(4))
        .and_then(|len| len.checked_add(p.len()))
        .ok_or_else(|| {
            "VYRECOB1 header length overflows usize. Fix: compile from a shorter path.".to_string()
        })?;
    let aligned_header_len = header_len
        .checked_add(7)
        .map(|len| len & !7)
        .ok_or_else(|| {
            "VYRECOB1 aligned header length overflows usize. Fix: compile from a shorter path."
                .to_string()
        })?;
    let record_bytes = n.checked_mul(12).ok_or_else(|| {
        format!(
            "VYRECOB1 token record byte length overflows usize for {n_tokens} tokens. Fix: shard this translation unit."
        )
    })?;
    let capacity = aligned_header_len
        .checked_add(4)
        .and_then(|len| len.checked_add(record_bytes))
        .ok_or_else(|| {
            "VYRECOB1 section length overflows usize. Fix: shard this translation unit.".to_string()
        })?;
    let mut file = Vec::with_capacity(capacity);
    file.extend_from_slice(b"VYRECOB1");
    file.extend_from_slice(&1u32.to_le_bytes());
    let path_len = u32::try_from(p.len()).map_err(|_| {
        format!(
            "source path length {} exceeds u32::MAX. Fix: compile from a shorter path or canonicalize through a shorter build root.",
            p.len()
        )
    })?;
    file.extend_from_slice(&path_len.to_le_bytes());
    file.extend_from_slice(p);
    while file.len() % 8 != 0 {
        file.push(0);
    }
    file.extend_from_slice(&n_tokens.to_le_bytes());
    for i in 0..n {
        let o = i.checked_mul(4).ok_or_else(|| {
            format!("VYRECOB1 token row {i} byte offset overflows usize. Fix: shard this translation unit.")
        })?;
        file.extend_from_slice(&types[o..o + 4]);
        file.extend_from_slice(&starts[o..o + 4]);
        file.extend_from_slice(&lens[o..o + 4]);
    }
    Ok(file)
}

fn require_prefix(buf: &[u8], bytes: usize, label: &str) -> Result<(), String> {
    if bytes > buf.len() {
        return Err(format!(
            "{label}: buffer too short: need {bytes} bytes, have {}",
            buf.len()
        ));
    }
    Ok(())
}

/// Serialize a `VYRECOB2` container into memory (same layout as on-disk).
pub fn serialize_vyrecob2(sections: &[(SectionTag, &[u8])]) -> Result<Vec<u8>, String> {
    let section_count = u32::try_from(sections.len()).map_err(|_| {
        format!(
            "VYRECOB2 section count {} exceeds u32::MAX. Fix: split this object container.",
            sections.len()
        )
    })?;
    let payload_bytes = sections
        .iter()
        .try_fold(0usize, |acc, (_, payload)| {
            acc.checked_add(8)?.checked_add(payload.len())
        })
        .ok_or_else(|| {
            "VYRECOB2 total payload length overflows usize. Fix: split this object container."
                .to_string()
        })?;
    let capacity = 16usize.checked_add(payload_bytes).ok_or_else(|| {
        "VYRECOB2 total container length overflows usize. Fix: split this object container."
            .to_string()
    })?;
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(VYRECOB2_MAGIC);
    out.extend_from_slice(&VYRECOB2_VERSION.to_le_bytes());
    out.extend_from_slice(&section_count.to_le_bytes());
    for (tag, payload) in sections {
        push_section(&mut out, *tag, payload)?;
    }
    Ok(out)
}

/// Borrowed VYRECOB2 container view.
#[derive(Debug, Clone)]
pub struct Vyrecob2<'a> {
    /// Container format version.
    pub version: u32,
    sections: Vec<(u32, &'a [u8])>,
}

impl<'a> Vyrecob2<'a> {
    /// Return the first section payload with the requested tag.
    #[must_use]
    pub fn section(&self, tag: SectionTag) -> Option<&'a [u8]> {
        let tag = tag as u32;
        self.sections
            .iter()
            .find_map(|(section_tag, payload)| (*section_tag == tag).then_some(*payload))
    }
}

/// Parse a standalone VYRECOB2 blob, or find the embedded VYRECOB2 payload
/// inside a larger object file.
pub fn parse_embedded_vyrecob2(bytes: &[u8]) -> Result<Vyrecob2<'_>, String> {
    let start = bytes
        .windows(VYRECOB2_MAGIC.len())
        .position(|window| window == VYRECOB2_MAGIC)
        .ok_or_else(|| {
            "vyre-frontend-c object does not contain VYRECOB2 magic. Fix: pass a vyre-frontend-c object emitted by vyrec -c.".to_string()
        })?;
    let mut offset = start + VYRECOB2_MAGIC.len();
    let version = read_container_u32(bytes, &mut offset, "VYRECOB2 version")?;
    let section_count = read_container_u32(bytes, &mut offset, "VYRECOB2 section count")?;
    let section_capacity = usize::try_from(section_count).map_err(|_| {
        format!(
            "VYRECOB2 section count {section_count} exceeds host usize. Fix: split this object container."
        )
    })?;
    let mut sections = Vec::with_capacity(section_capacity);
    for section_idx in 0..section_count {
        let tag = read_container_u32(bytes, &mut offset, "VYRECOB2 section tag")?;
        let len_u32 = read_container_u32(bytes, &mut offset, "VYRECOB2 section length")?;
        let len = usize::try_from(len_u32).map_err(|_| {
            format!(
                "VYRECOB2 section {section_idx} tag {tag} length {len_u32} exceeds host usize. Fix: regenerate the object."
            )
        })?;
        let end = offset.checked_add(len).ok_or_else(|| {
            format!("VYRECOB2 section {section_idx} length overflows usize. Fix: regenerate the object.")
        })?;
        let payload = bytes.get(offset..end).ok_or_else(|| {
            format!(
                "VYRECOB2 section {section_idx} tag {tag} is truncated: need byte {end}, object has {}. Fix: regenerate the object.",
                bytes.len()
            )
        })?;
        sections.push((tag, payload));
        offset = end;
    }
    Ok(Vyrecob2 { version, sections })
}

fn read_container_u32(bytes: &[u8], offset: &mut usize, label: &str) -> Result<u32, String> {
    let end = offset.checked_add(4).ok_or_else(|| {
        format!(
            "{label} offset overflows usize at byte {}. Fix: regenerate the object.",
            *offset
        )
    })?;
    let word = bytes.get(*offset..end).ok_or_else(|| {
        format!(
            "{label} is truncated at byte {}. Fix: regenerate the object.",
            *offset
        )
    })?;
    *offset = end;
    Ok(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
}

/// Serialize `sections` into a `VYRECOB2` blob and write it to `path`, replacing any existing file.
pub fn write_vyrecob2(
    path: &std::path::Path,
    sections: &[(SectionTag, &[u8])],
) -> Result<(), String> {
    let out = serialize_vyrecob2(sections)?;
    std::fs::write(path, out).map_err(|e| format!("write {}: {e}", path.display()))
}
