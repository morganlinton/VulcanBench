//! Packed binary wire format for grammar table blobs.
//!
//! `vyre-libs::parsing::{lexer,lr_table}` loads these blobs as ReadOnly
//! storage buffers on the GPU. Layout:
//!
//! ```text
//! bytes 0..4   : magic b"SGGC"
//! bytes 4..6   : version = 1 (LE u16)
//! bytes 6..8   : kind (LE u16, 0 = lexer DFA, 1 = LR)
//! bytes 8..12  : num_states (LE u32)
//! bytes 12..16 : num_classes (for lexer) or num_tokens (for LR)
//! bytes 16..20 : extra (nonterminal count for LR, token-id count for lexer)
//! bytes 20..24 : payload_len
//! bytes 24..N  : payload (packed u32 transitions + aux arrays)
//! bytes N..N+16: BLAKE3-128(payload) integrity tag
//! ```

use crate::dfa::DfaTable;
use crate::lr::{validate_lr_table, LrTable, Production};

/// Magic bytes at the head of every blob: `SGGC` = "Surgec Grammar-Gen C".
pub const MAGIC: [u8; 4] = *b"SGGC";
/// Wire format version.
pub const VERSION: u16 = 1;

/// Which kind of grammar table the blob carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum BlobKind {
    /// Lexer DFA.
    LexerDfa = 0,
    /// LR(1) action + goto tables.
    LrTables = 1,
}

/// Failure to decode an `SGGC` blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// Buffer shorter than the 24-byte header.
    TooShort {
        /// Minimum bytes required.
        need: usize,
        /// Bytes available.
        got: usize,
    },
    /// Magic bytes do not spell `SGGC`.
    BadMagic([u8; 4]),
    /// Wire [`VERSION`] mismatch.
    UnsupportedVersion(u16),
    /// Unknown [`BlobKind`] discriminant.
    UnsupportedKind(u16),
    /// Declared payload does not fit in `bytes`.
    PayloadTruncated {
        /// Total bytes implied by header + payload.
        expected: usize,
        /// Actual byte length.
        got: usize,
    },
    /// Payload checksum does not match the BLAKE3 tag at the tail.
    ChecksumMismatch {
        /// BLAKE3-128 tag computed from the payload.
        expected: [u8; 16],
        /// BLAKE3-128 tag read from the blob.
        got: [u8; 16],
    },
    /// Lexer payload word count does not match header dimensions.
    LexerPayloadWordCount {
        /// `(num_states * num_classes) + token_id_words` expected.
        expected: usize,
        /// Words in payload.
        got: usize,
    },
    /// LR payload cannot be split into action, goto, and productions.
    LrPayloadSize,
}

/// A packed grammar blob.
#[derive(Debug, Clone)]
pub struct PackedBlob {
    /// Which kind of table this blob carries.
    pub kind: BlobKind,
    /// Raw bytes ready for upload.
    pub bytes: Vec<u8>,
}

impl PackedBlob {
    /// Pack a lexer DFA into a blob.
    #[must_use]
    pub fn from_dfa(dfa: &DfaTable) -> Self {
        let mut payload = Vec::new();
        for &word in &dfa.transitions {
            payload.extend_from_slice(&word.to_le_bytes());
        }
        for &word in &dfa.token_ids {
            payload.extend_from_slice(&word.to_le_bytes());
        }

        let bytes = write_header(
            BlobKind::LexerDfa,
            dfa.num_states,
            dfa.num_classes,
            u32::try_from(dfa.token_ids.len()).unwrap_or(u32::MAX),
            &payload,
        );

        Self {
            kind: BlobKind::LexerDfa,
            bytes,
        }
    }

    /// Pack an LR table into a blob.
    ///
    /// # Errors
    ///
    /// Returns the validation error when the table's `action` or `goto` vector
    /// lengths do not match the declared dimension fields (`num_states`,
    /// `num_tokens`, `num_nonterminals`). Packing a malformed table would
    /// produce a structurally valid `SGGC` blob (correct magic + checksum) that
    /// contains wrong data; the GPU parser would index out-of-bounds or dispatch
    /// to the wrong state.
    pub fn from_lr(lr: &LrTable) -> Result<Self, String> {
        validate_lr_table(lr)?;
        let mut payload = Vec::new();
        for &word in &lr.action {
            payload.extend_from_slice(&word.to_le_bytes());
        }
        for &word in &lr.goto {
            payload.extend_from_slice(&word.to_le_bytes());
        }
        for prod in &lr.productions {
            payload.extend_from_slice(&prod.lhs.to_le_bytes());
            payload.extend_from_slice(&prod.rhs_len.to_le_bytes());
        }

        let bytes = write_header(
            BlobKind::LrTables,
            lr.num_states,
            lr.num_tokens,
            lr.num_nonterminals,
            &payload,
        );

        Ok(Self {
            kind: BlobKind::LrTables,
            bytes,
        })
    }

    /// Decode a lexer DFA from this blob’s bytes.
    pub fn try_as_dfa(&self) -> Result<DfaTable, WireError> {
        decode_dfa_from_bytes(&self.bytes)
    }

    /// Decode LR tables from this blob’s bytes.
    pub fn try_as_lr(&self) -> Result<LrTable, WireError> {
        decode_lr_from_bytes(&self.bytes)
    }
}

/// Decode a lexer DFA from raw `SGGC` bytes (host round-trip / tests).
pub fn decode_dfa_from_bytes(bytes: &[u8]) -> Result<DfaTable, WireError> {
    let header = parse_header(bytes)?;
    if header.kind != BlobKind::LexerDfa as u16 {
        return Err(WireError::UnsupportedKind(header.kind));
    }
    let num_states = header.num_states;
    let num_classes = header.num_classes;
    let token_words = header.extra as usize;
    let trans_words = (num_states as usize).saturating_mul(num_classes as usize);
    let expected_words = trans_words.saturating_add(token_words);
    let got_words = header.payload.len() / 4;
    if got_words != expected_words || header.payload.len() % 4 != 0 {
        return Err(WireError::LexerPayloadWordCount {
            expected: expected_words,
            got: got_words,
        });
    }
    let mut words = read_u32_words(header.payload);
    let transitions = words.drain(..trans_words).collect();
    let token_ids = words;
    Ok(DfaTable {
        num_states,
        num_classes,
        transitions,
        token_ids,
    })
}

/// Decode LR tables from raw `SGGC` bytes.
pub fn decode_lr_from_bytes(bytes: &[u8]) -> Result<LrTable, WireError> {
    let header = parse_header(bytes)?;
    if header.kind != BlobKind::LrTables as u16 {
        return Err(WireError::UnsupportedKind(header.kind));
    }
    let num_states = header.num_states;
    let num_tokens = header.num_classes;
    let num_nonterminals = header.extra;
    let action_words = (num_states as usize).saturating_mul(num_tokens as usize);
    let goto_words = (num_states as usize).saturating_mul(num_nonterminals as usize);
    let words: Vec<u32> = read_u32_words(header.payload);
    let min = action_words.saturating_add(goto_words);
    if words.len() < min || (words.len() - min) % 2 != 0 {
        return Err(WireError::LrPayloadSize);
    }
    let mut w = words.into_iter();
    let action: Vec<u32> = w.by_ref().take(action_words).collect();
    let goto: Vec<u32> = w.by_ref().take(goto_words).collect();
    let mut productions = Vec::new();
    while let (Some(lhs), Some(rhs_len)) = (w.next(), w.next()) {
        productions.push(Production { lhs, rhs_len });
    }
    Ok(LrTable {
        num_states,
        num_tokens,
        num_nonterminals,
        action,
        goto,
        productions,
    })
}

struct HeaderParts<'a> {
    kind: u16,
    num_states: u32,
    num_classes: u32,
    extra: u32,
    payload: &'a [u8],
}

fn parse_header(bytes: &[u8]) -> Result<HeaderParts<'_>, WireError> {
    if bytes.len() < 24 {
        return Err(WireError::TooShort {
            need: 24,
            got: bytes.len(),
        });
    }
    if bytes[0..4] != MAGIC {
        return Err(WireError::BadMagic([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]));
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != VERSION {
        return Err(WireError::UnsupportedVersion(version));
    }
    let kind = u16::from_le_bytes([bytes[6], bytes[7]]);
    let num_states = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let num_classes = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let extra = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let payload_len = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as usize;
    let total = 24usize.saturating_add(payload_len);
    let checksum_end = total.saturating_add(16);
    if bytes.len() < checksum_end {
        return Err(WireError::PayloadTruncated {
            expected: checksum_end,
            got: bytes.len(),
        });
    }
    let expected = blake3_128(&bytes[24..total]);
    let mut got = [0u8; 16];
    got.copy_from_slice(&bytes[total..checksum_end]);
    if got != expected {
        return Err(WireError::ChecksumMismatch { expected, got });
    }
    Ok(HeaderParts {
        kind,
        num_states,
        num_classes,
        extra,
        payload: &bytes[24..total],
    })
}

fn read_u32_words(payload: &[u8]) -> Vec<u32> {
    payload
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn write_header(kind: BlobKind, states: u32, classes: u32, extra: u32, payload: &[u8]) -> Vec<u8> {
    let payload_len = u32::try_from(payload.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(24 + payload.len() + 16);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(kind as u16).to_le_bytes());
    out.extend_from_slice(&states.to_le_bytes());
    out.extend_from_slice(&classes.to_le_bytes());
    out.extend_from_slice(&extra.to_le_bytes());
    out.extend_from_slice(&payload_len.to_le_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&blake3_128(payload));
    out
}

fn blake3_128(payload: &[u8]) -> [u8; 16] {
    let digest = blake3::hash(payload);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dfa::DfaBuilder;
    use crate::lr::{Action, LrBuilder};

    fn test_lr_table() -> LrTable {
        let mut b = LrBuilder::new(4, 3, 1);
        let prod_unit = b.add_production(0, 2);
        b.set_action(0, 0, Action::Shift(1));
        b.set_action(0, 2, Action::Accept);
        b.set_action(1, 1, Action::Shift(2));
        b.set_action(2, 0, Action::Reduce(prod_unit));
        b.set_action(2, 2, Action::Reduce(prod_unit));
        b.build()
    }

    #[test]
    fn lexer_blob_starts_with_magic_and_kind() {
        let dfa = DfaBuilder::new(4, 8)
            .build()
            .expect("empty pattern set must succeed");
        let blob = PackedBlob::from_dfa(&dfa);
        assert_eq!(&blob.bytes[0..4], &MAGIC);
        let version = u16::from_le_bytes([blob.bytes[4], blob.bytes[5]]);
        assert_eq!(version, VERSION);
        let kind = u16::from_le_bytes([blob.bytes[6], blob.bytes[7]]);
        assert_eq!(kind, BlobKind::LexerDfa as u16);
    }

    #[test]
    fn lr_blob_starts_with_magic_and_kind() {
        let lr = test_lr_table();
        let blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack without error");
        assert_eq!(&blob.bytes[0..4], &MAGIC);
        let kind = u16::from_le_bytes([blob.bytes[6], blob.bytes[7]]);
        assert_eq!(kind, BlobKind::LrTables as u16);
    }

    #[test]
    fn lexer_blob_payload_length_matches_header() {
        let dfa = DfaBuilder::new(4, 8)
            .build()
            .expect("empty pattern set must succeed");
        let blob = PackedBlob::from_dfa(&dfa);
        let payload_len = u32::from_le_bytes([
            blob.bytes[20],
            blob.bytes[21],
            blob.bytes[22],
            blob.bytes[23],
        ]) as usize;
        assert_eq!(blob.bytes.len(), 24 + payload_len + 16);
    }

    #[test]
    fn lexer_blob_checksum_rejects_corruption() {
        let dfa = DfaBuilder::new(4, 8)
            .build()
            .expect("empty pattern set must succeed");
        let mut blob = PackedBlob::from_dfa(&dfa);
        let last_payload_byte = blob.bytes.len() - 17;
        blob.bytes[last_payload_byte] ^= 0x80;
        assert!(matches!(
            blob.try_as_dfa(),
            Err(WireError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn lexer_dfa_roundtrips_through_wire() {
        let dfa = DfaBuilder::new(4, 8)
            .build()
            .expect("empty pattern set must succeed");
        let blob = PackedBlob::from_dfa(&dfa);
        let got = blob.try_as_dfa().expect("decode lexer blob");
        assert_eq!(got, dfa);
    }

    #[test]
    fn lr_table_roundtrips_through_wire() {
        let lr = test_lr_table();
        let blob = PackedBlob::from_lr(&lr).expect("valid LR table must pack without error");
        let got = blob.try_as_lr().expect("decode LR blob");
        assert_eq!(got, lr);
    }

    #[test]
    fn from_lr_rejects_mismatched_action_table_not_packs_malformed_blob() {
        // A table with num_states=4, num_tokens=3 declares 12 action words,
        // but if we pop one the dimensions are inconsistent. from_lr must
        // return Err, not pack a structurally valid blob with wrong data
        // (wire-lr-validation-discarded).
        let mut lr = test_lr_table();
        assert_eq!(lr.num_states, 4);
        assert_eq!(lr.num_tokens, 3);
        // Remove one action word to break the num_states * num_tokens == action.len() contract.
        lr.action.pop();
        let err = PackedBlob::from_lr(&lr).expect_err(
            "Fix: from_lr must return Err when action table length does not match \
             num_states * num_tokens; packing the blob would produce wrong GPU parse tables",
        );
        assert!(
            err.contains("Fix:"),
            "Fix: validation error must include a 'Fix:' hint, got: {err}"
        );
        assert!(
            err.contains("action") || err.contains("dimension"),
            "Fix: error must identify the mismatched action table, got: {err}"
        );
    }
}
