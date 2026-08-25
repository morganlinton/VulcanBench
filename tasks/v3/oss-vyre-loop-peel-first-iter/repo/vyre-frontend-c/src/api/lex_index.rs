use std::path::Path;

use crate::api::object_io::read_object_bytes_bounded;
use crate::object_format::{parse_embedded_vyrecob2, SectionTag, Vyrecob2};

/// Decoded token stream from the embedded `VYRECOB1` lex section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CObjectLexIndex {
    /// VYRECOB2 container version.
    pub vyrecob2_version: u32,
    /// Source path embedded in the lex section.
    pub source_path: String,
    /// Logical C tokens.
    pub tokens: Vec<CObjectToken>,
}

/// One token row: kind plus source byte span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CObjectToken {
    /// C token kind.
    pub kind: u32,
    /// Source byte offset.
    pub start: u32,
    /// Source byte length.
    pub len: u32,
}

impl CObjectToken {
    /// Exclusive source byte end.
    #[must_use]
    pub fn end(self) -> Option<u32> {
        self.start.checked_add(self.len)
    }

    /// Source byte range as host indices.
    #[must_use]
    pub fn byte_range(self) -> Option<std::ops::Range<usize>> {
        let start = usize::try_from(self.start).ok()?;
        let end = usize::try_from(self.end()?).ok()?;
        Some(start..end)
    }
}

impl CObjectLexIndex {
    /// Return a token row by token index.
    #[must_use]
    pub fn token(&self, token_index: u32) -> Option<&CObjectToken> {
        self.tokens.get(usize::try_from(token_index).ok()?)
    }

    /// Return the source bytes covered by a token row.
    #[must_use]
    pub fn token_bytes<'a>(&self, source: &'a [u8], token_index: u32) -> Option<&'a [u8]> {
        let range = self.token(token_index)?.byte_range()?;
        source.get(range)
    }

    /// Return the UTF-8 source text covered by a token row.
    pub fn token_text<'a>(
        &self,
        source: &'a [u8],
        token_index: u32,
    ) -> Result<Option<&'a str>, String> {
        let Some(bytes) = self.token_bytes(source, token_index) else {
            return Ok(None);
        };
        std::str::from_utf8(bytes)
            .map(Some)
            .map_err(|error| format!("vyre-frontend-c token {token_index} is not UTF-8: {error}"))
    }
}

/// Decode the token stream from object bytes.
pub fn decode_object_lex_index(object_bytes: &[u8]) -> Result<CObjectLexIndex, String> {
    let container = parse_embedded_vyrecob2(object_bytes)?;
    decode_object_lex_index_from_container(&container)
}

pub(crate) fn decode_object_lex_index_from_container(
    container: &Vyrecob2<'_>,
) -> Result<CObjectLexIndex, String> {
    let lex_section = container.section(SectionTag::Lex).ok_or_else(|| {
        "vyre-frontend-c object is missing Lex. Fix: compile with lexer object emission enabled."
            .to_string()
    })?;
    let (source_path, tokens) = decode_vyrecob1_lex_section(lex_section)?;
    Ok(CObjectLexIndex {
        vyrecob2_version: container.version,
        source_path,
        tokens,
    })
}

/// Read and decode the token stream from an object path.
pub fn decode_object_lex_index_file(path: &Path) -> Result<CObjectLexIndex, String> {
    let bytes = read_object_bytes_bounded(path)?;
    decode_object_lex_index(&bytes)
}

fn decode_vyrecob1_lex_section(section: &[u8]) -> Result<(String, Vec<CObjectToken>), String> {
    const MAGIC: &[u8; 8] = b"VYRECOB1";
    if section.len() < 16 || &section[..8] != MAGIC {
        return Err(
            "vyre-frontend-c Lex section is not a VYRECOB1 payload. Fix: regenerate the object."
                .to_string(),
        );
    }
    let version = read_u32(section, 8, "VYRECOB1 version")?;
    if version != 1 {
        return Err(format!(
            "vyre-frontend-c Lex section version {version} is unsupported. Fix: use a compatible decoder."
        ));
    }
    let path_len_u32 = read_u32(section, 12, "VYRECOB1 path length")?;
    let path_len = usize::try_from(path_len_u32).map_err(|_| {
        format!(
            "vyre-frontend-c Lex path length {path_len_u32} exceeds host usize. Fix: regenerate the object."
        )
    })?;
    if path_len == 0 {
        return Err(
            "vyre-frontend-c Lex source path is empty. Fix: regenerate the object with the originating source path."
                .to_string(),
        );
    }
    let path_start = 16usize;
    let path_end = path_start.checked_add(path_len).ok_or_else(|| {
        "vyre-frontend-c Lex path length overflowed usize. Fix: regenerate the object.".to_string()
    })?;
    let path_bytes = section.get(path_start..path_end).ok_or_else(|| {
        format!(
            "vyre-frontend-c Lex path is truncated: need byte {path_end}, section has {}. Fix: regenerate the object.",
            section.len()
        )
    })?;
    let source_path = std::str::from_utf8(path_bytes)
        .map_err(|error| format!("vyre-frontend-c Lex path is not UTF-8: {error}"))?
        .to_string();
    let token_count_offset = path_end
        .checked_add(7)
        .map(|offset| offset & !7)
        .ok_or_else(|| {
            "vyre-frontend-c Lex token-count offset overflowed usize. Fix: regenerate the object."
                .to_string()
        })?;
    let token_count_u32 = read_u32(section, token_count_offset, "VYRECOB1 token count")?;
    let token_count = usize::try_from(token_count_u32).map_err(|_| {
        format!(
            "vyre-frontend-c Lex token count {token_count_u32} exceeds host usize. Fix: shard the object."
        )
    })?;
    let rows_start = token_count_offset.checked_add(4).ok_or_else(|| {
        "vyre-frontend-c Lex token rows offset overflowed usize. Fix: shard the object.".to_string()
    })?;
    let row_bytes = token_count.checked_mul(12).ok_or_else(|| {
        "vyre-frontend-c Lex token row byte count overflowed usize. Fix: shard the object."
            .to_string()
    })?;
    let rows_end = rows_start.checked_add(row_bytes).ok_or_else(|| {
        "vyre-frontend-c Lex token rows overflowed usize. Fix: shard the object.".to_string()
    })?;
    if rows_end > section.len() {
        return Err(format!(
            "vyre-frontend-c Lex rows are truncated: need byte {rows_end}, section has {}. Fix: regenerate the object.",
            section.len()
        ));
    }
    if rows_end != section.len() {
        return Err(format!(
            "vyre-frontend-c Lex section has {} trailing bytes after token rows. Fix: regenerate the object; Lex payloads must be exact.",
            section.len() - rows_end
        ));
    }
    let mut tokens = Vec::with_capacity(token_count);
    let mut offset = rows_start;
    let mut previous_end = 0u32;
    for row_index in 0..token_count {
        let token = CObjectToken {
            kind: read_u32(section, offset, "VYRECOB1 token kind")?,
            start: read_u32(section, offset + 4, "VYRECOB1 token start")?,
            len: read_u32(section, offset + 8, "VYRECOB1 token length")?,
        };
        let token_end = token.end().ok_or_else(|| {
            format!(
                "vyre-frontend-c Lex token row {row_index} span overflows u32: start={} len={}. Fix: regenerate the object with bounded source spans.",
                token.start, token.len
            )
        })?;
        if row_index != 0 && token.start < previous_end {
            return Err(format!(
                "vyre-frontend-c Lex token row {row_index} starts at {} before previous token end {previous_end}. Fix: regenerate the object with monotonic non-overlapping token spans.",
                token.start
            ));
        }
        previous_end = token_end;
        tokens.push(token);
        offset = offset.checked_add(12).ok_or_else(|| {
            "vyre-frontend-c Lex token row offset overflowed usize. Fix: shard the object."
                .to_string()
        })?;
    }
    Ok((source_path, tokens))
}

fn read_u32(bytes: &[u8], offset: usize, label: &str) -> Result<u32, String> {
    // Canonical LEGO: byte-offset LE u32 read lives once in vyre-primitives::wire.
    vyre_primitives::wire::read_u32_le_at(bytes, offset, label)
}

#[cfg(test)]
mod tests {
    use super::decode_object_lex_index;
    use crate::object_format::{build_vyrecob1_lex_section, serialize_vyrecob2, SectionTag};
    use std::path::Path;

    #[test]
    fn decodes_vyrecob1_token_rows() {
        let types = pack_words(&[107u32, 1, 16]);
        let starts = pack_words(&[0u32, 4, 5]);
        let lens = pack_words(&[3u32, 1, 1]);
        let lex = build_vyrecob1_lex_section(Path::new("x.c"), &types, &starts, &lens, 3)
            .expect("Fix: lex fixture must serialize");
        let object = serialize_vyrecob2(&[(SectionTag::Lex, lex.as_slice())])
            .expect("Fix: object fixture must serialize");

        let decoded = decode_object_lex_index(&object).expect("Fix: lex fixture must decode");
        assert_eq!(decoded.source_path, "x.c");
        assert_eq!(decoded.tokens.len(), 3);
        assert_eq!(decoded.tokens[0].kind, 107);
        assert_eq!(decoded.tokens[1].start, 4);
        assert_eq!(decoded.tokens[2].len, 1);
    }

    #[test]
    fn rejects_empty_lex_source_path() {
        let mut lex = Vec::new();
        lex.extend_from_slice(b"VYRECOB1");
        lex.extend_from_slice(&1u32.to_le_bytes());
        lex.extend_from_slice(&0u32.to_le_bytes());
        lex.extend_from_slice(&0u32.to_le_bytes());
        let object = serialize_vyrecob2(&[(SectionTag::Lex, lex.as_slice())])
            .expect("Fix: object fixture must serialize");
        let err = decode_object_lex_index(&object).expect_err("empty path must not decode");
        assert!(
            err.contains("source path is empty"),
            "empty Lex source path must fail loudly"
        );
    }

    #[test]
    fn rejects_lex_token_span_overflow() {
        let types = pack_words(&[107u32]);
        let starts = pack_words(&[u32::MAX]);
        let lens = pack_words(&[1u32]);
        let lex = build_vyrecob1_lex_section(Path::new("x.c"), &types, &starts, &lens, 1)
            .expect("Fix: lex fixture must serialize");
        let object = serialize_vyrecob2(&[(SectionTag::Lex, lex.as_slice())])
            .expect("Fix: object fixture must serialize");
        let err = decode_object_lex_index(&object).expect_err("overflowing token must not decode");
        assert!(
            err.contains("span overflows u32"),
            "overflowing token span must fail loudly"
        );
    }

    #[test]
    fn rejects_overlapping_lex_token_spans() {
        let types = pack_words(&[107u32, 108]);
        let starts = pack_words(&[10u32, 9]);
        let lens = pack_words(&[2u32, 1]);
        let lex = build_vyrecob1_lex_section(Path::new("x.c"), &types, &starts, &lens, 2)
            .expect("Fix: lex fixture must serialize");
        let object = serialize_vyrecob2(&[(SectionTag::Lex, lex.as_slice())])
            .expect("Fix: object fixture must serialize");
        let err = decode_object_lex_index(&object).expect_err("overlap must not decode");
        assert!(
            err.contains("monotonic non-overlapping"),
            "overlapping Lex token spans must fail loudly"
        );
    }

    use vyre_primitives::wire::pack_u32_slice as pack_words;
}
