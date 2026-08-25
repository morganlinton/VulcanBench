//! Versioned scan database header framing.
//!
//! This is not a `VIR0` Program payload. It is the canonical cache/evidence
//! header for serialized scan databases that reference compiled pattern sets,
//! table sections, and unsupported construct diagnostics.

use super::WireEncodeErr;
use crate::serial::wire::framing::{put_len_u32, put_string, put_u32, put_u8};
use crate::serial::wire::Reader;
use serde::{Deserialize, Serialize};

/// Four-byte magic identifying a versioned scan database header (`VSDH`).
pub const SCAN_DATABASE_HEADER_MAGIC: &[u8; 4] = b"VSDH";
/// Current scan database header wire version.
pub const SCAN_DATABASE_HEADER_VERSION: u32 = 1;
/// Upper bound on table sections accepted from an untrusted header.
pub const MAX_SCAN_DATABASE_SECTIONS: usize = 4_096;
/// Upper bound on unsupported-feature diagnostics accepted from a header.
pub const MAX_SCAN_DATABASE_UNSUPPORTED_FEATURES: usize = 4_096;

/// Scan execution mode the database was compiled for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScanDatabaseMode {
    /// Whole-buffer block scanning.
    Block,
    /// Incremental streaming scan with carried state.
    Streaming,
    /// Vectored scan over multiple discontiguous buffers.
    Vectored,
}

impl ScanDatabaseMode {
    const fn tag(self) -> u8 {
        match self {
            Self::Block => 1,
            Self::Streaming => 2,
            Self::Vectored => 3,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, String> {
        match tag {
            1 => Ok(Self::Block),
            2 => Ok(Self::Streaming),
            3 => Ok(Self::Vectored),
            _ => Err(format!(
                "scan database mode tag {tag} is unsupported. Fix: recompile the scan database with a compatible Vyre scan compiler."
            )),
        }
    }
}

/// Kind of payload a table section carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScanDatabaseSectionKind {
    /// Compiled literal-set table.
    LiteralTable,
    /// Compiled automata (DFA/NFA) transition table.
    AutomataTable,
    /// Verifier-only regex fragments for constructs the engine cannot match.
    VerifierFragments,
    /// Output/result layout descriptor.
    OutputLayout,
    /// Carried streaming-scan state.
    StreamingState,
    /// Relation seed table for cross-pattern relations.
    RelationSeeds,
}

impl ScanDatabaseSectionKind {
    const fn tag(self) -> u8 {
        match self {
            Self::LiteralTable => 1,
            Self::AutomataTable => 2,
            Self::VerifierFragments => 3,
            Self::OutputLayout => 4,
            Self::StreamingState => 5,
            Self::RelationSeeds => 6,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, String> {
        match tag {
            1 => Ok(Self::LiteralTable),
            2 => Ok(Self::AutomataTable),
            3 => Ok(Self::VerifierFragments),
            4 => Ok(Self::OutputLayout),
            5 => Ok(Self::StreamingState),
            6 => Ok(Self::RelationSeeds),
            _ => Err(format!(
                "scan database section tag {tag} is unsupported. Fix: recompile the scan database with a compatible Vyre scan compiler."
            )),
        }
    }
}

/// Locator and integrity digest for one table section in the database body.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScanDatabaseSectionHeader {
    /// Kind of payload this section carries.
    pub kind: ScanDatabaseSectionKind,
    /// Byte offset of the section payload from the start of the database body.
    pub offset: u64,
    /// Byte length of the section payload.
    pub byte_len: u64,
    /// Integrity digest over the section payload bytes.
    pub section_digest: u64,
}

/// A construct the compiler could not lower, recorded for verifier handoff.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnsupportedScanFeature {
    /// Index of the pattern that used the unsupported construct.
    pub pattern_index: u32,
    /// Human-readable description of the unsupported feature.
    pub feature: String,
}

/// How a reader may consume a database given its unsupported constructs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScanDatabaseReaderCompatibility {
    /// Fully consumable by the engine alone.
    Compatible,
    /// Consumable only with a verifier for the unsupported fragments.
    RequiresVerifier,
    /// Not consumable by this reader at all.
    Incompatible,
}

impl ScanDatabaseReaderCompatibility {
    const fn tag(self) -> u8 {
        match self {
            Self::Compatible => 1,
            Self::RequiresVerifier => 2,
            Self::Incompatible => 3,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, String> {
        match tag {
            1 => Ok(Self::Compatible),
            2 => Ok(Self::RequiresVerifier),
            3 => Ok(Self::Incompatible),
            _ => Err(format!(
                "scan database reader compatibility tag {tag} is unsupported. Fix: rebuild the scan database cache with a compatible Vyre scan compiler."
            )),
        }
    }
}

/// Construct-tier and dialect compatibility fingerprint for a database.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScanDatabaseCompatibilityRecord {
    /// Digest of the construct-tier matrix the database was compiled against.
    pub construct_tier_digest: u64,
    /// Digest of the regex dialect lattice the database was compiled against.
    pub dialect_digest: u64,
    /// Reader compatibility class for this database.
    pub reader_compatibility: ScanDatabaseReaderCompatibility,
}

/// Versioned, self-describing header for a serialized scan database.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScanDatabaseHeader {
    /// 32-byte digest of the compiled pattern set.
    pub pattern_set_digest: [u8; 32],
    /// Version string of the compiler that produced the database.
    pub compiler_version: String,
    /// Scan mode the database was compiled for.
    pub mode: ScanDatabaseMode,
    /// Locators for the table sections in the database body.
    pub table_sections: Vec<ScanDatabaseSectionHeader>,
    /// Constructs that require verifier handoff.
    pub unsupported_features: Vec<UnsupportedScanFeature>,
    /// Construct-tier and dialect compatibility fingerprint.
    pub compatibility: ScanDatabaseCompatibilityRecord,
}

impl ScanDatabaseHeader {
    /// Number of table sections in the database body.
    #[must_use]
    pub fn section_count(&self) -> usize {
        self.table_sections.len()
    }

    /// Number of unsupported-feature diagnostics recorded in the header.
    #[must_use]
    pub fn unsupported_feature_count(&self) -> usize {
        self.unsupported_features.len()
    }

    /// Validate this header against the consumer's required compiler version
    /// and scan mode before any table payload is trusted.
    ///
    /// # Errors
    ///
    /// Returns an actionable `Fix:` diagnostic when compiler version or mode
    /// differs from the expected cache key.
    pub fn validate_compatible(
        &self,
        expected_compiler_version: &str,
        expected_mode: ScanDatabaseMode,
    ) -> Result<(), String> {
        if self.compiler_version != expected_compiler_version {
            return Err(format!(
                "scan database compiler version `{}` is incompatible with expected `{expected_compiler_version}`. Fix: rebuild the scan database cache with the current compiler.",
                self.compiler_version
            ));
        }
        if self.mode != expected_mode {
            return Err(format!(
                "scan database mode {:?} is incompatible with expected {:?}. Fix: rebuild the scan database cache for the requested scan mode.",
                self.mode, expected_mode
            ));
        }
        Ok(())
    }

    /// Validate construct-tier and dialect compatibility before trusting table
    /// payload bytes.
    ///
    /// # Errors
    ///
    /// Returns a `Fix:` diagnostic when the construct-tier digest, dialect
    /// digest, or reader compatibility class is not accepted by the caller.
    pub fn validate_database_compatibility(
        &self,
        expected_construct_tier_digest: u64,
        expected_dialect_digest: u64,
        accepted_reader_compatibility: &[ScanDatabaseReaderCompatibility],
    ) -> Result<(), String> {
        if self.compatibility.construct_tier_digest != expected_construct_tier_digest {
            return Err(format!(
                "scan database construct tier digest {:#x} is incompatible with expected {expected_construct_tier_digest:#x}. Fix: rebuild the scan database cache from the current construct tier matrix.",
                self.compatibility.construct_tier_digest
            ));
        }
        if self.compatibility.dialect_digest != expected_dialect_digest {
            return Err(format!(
                "scan database dialect digest {:#x} is incompatible with expected {expected_dialect_digest:#x}. Fix: rebuild the scan database cache from the current regex dialect lattice.",
                self.compatibility.dialect_digest
            ));
        }
        if !accepted_reader_compatibility
            .iter()
            .any(|accepted| *accepted == self.compatibility.reader_compatibility)
        {
            return Err(format!(
                "scan database reader compatibility {:?} is not accepted by this reader. Fix: choose a verifier-capable reader or rebuild the scan database.",
                self.compatibility.reader_compatibility
            ));
        }
        Ok(())
    }
}

/// Encode a versioned scan database header.
///
/// # Errors
///
/// Returns [`WireEncodeErr`] when string or vector lengths exceed the bounded
/// wire representation.
pub fn encode_scan_database_header(
    header: &ScanDatabaseHeader,
) -> Result<Vec<u8>, WireEncodeErr> {
    let mut out = Vec::with_capacity(96 + header.table_sections.len() * 25);
    put_scan_database_header(&mut out, header)?;
    Ok(out)
}

/// Append a versioned scan database header to an existing byte buffer.
///
/// # Errors
///
/// Returns [`WireEncodeErr`] when the header cannot be represented in the
/// fixed-width wire format.
pub fn put_scan_database_header(
    out: &mut Vec<u8>,
    header: &ScanDatabaseHeader,
) -> Result<(), WireEncodeErr> {
    out.extend_from_slice(SCAN_DATABASE_HEADER_MAGIC);
    put_u32(out, SCAN_DATABASE_HEADER_VERSION);
    out.extend_from_slice(&header.pattern_set_digest);
    put_string(out, &header.compiler_version)?;
    put_u8(out, header.mode.tag());
    put_len_u32(
        out,
        header.table_sections.len(),
        "scan database section count ",
    )?;
    for section in &header.table_sections {
        put_u8(out, section.kind.tag());
        put_u64(out, section.offset);
        put_u64(out, section.byte_len);
        put_u64(out, section.section_digest);
    }
    put_len_u32(
        out,
        header.unsupported_features.len(),
        "scan database unsupported feature count ",
    )?;
    for unsupported in &header.unsupported_features {
        put_u32(out, unsupported.pattern_index);
        put_string(out, &unsupported.feature)?;
    }
    put_u64(out, header.compatibility.construct_tier_digest);
    put_u64(out, header.compatibility.dialect_digest);
    put_u8(out, header.compatibility.reader_compatibility.tag());
    Ok(())
}

/// Decode a versioned scan database header without checking compiler/mode
/// compatibility.
///
/// # Errors
///
/// Returns a `Fix:` diagnostic when the header is truncated, has the wrong
/// magic/version, has unknown enum tags, or contains trailing bytes.
pub fn decode_scan_database_header(bytes: &[u8]) -> Result<ScanDatabaseHeader, String> {
    let mut reader = Reader {
        bytes,
        pos: 0,
        depth: 0,
    };
    let magic = reader.take(SCAN_DATABASE_HEADER_MAGIC.len())?;
    if magic != SCAN_DATABASE_HEADER_MAGIC {
        return Err(
            "invalid scan database header magic. Fix: load a VSDH scan database header, not a VIR0 Program blob."
                .to_string(),
        );
    }
    let version = reader.u32()?;
    if version != SCAN_DATABASE_HEADER_VERSION {
        return Err(format!(
            "scan database header version {version} is unsupported; expected {SCAN_DATABASE_HEADER_VERSION}. Fix: rebuild the scan database cache."
        ));
    }
    let digest_bytes = reader.take(32)?;
    let mut pattern_set_digest = [0u8; 32];
    pattern_set_digest.copy_from_slice(digest_bytes);
    let compiler_version = reader.string()?;
    let mode = ScanDatabaseMode::from_tag(reader.u8()?)?;

    let section_count = reader.bounded_len(
        MAX_SCAN_DATABASE_SECTIONS,
        "scan database section count",
    )?;
    let mut table_sections = Vec::with_capacity(section_count);
    for _ in 0..section_count {
        table_sections.push(ScanDatabaseSectionHeader {
            kind: ScanDatabaseSectionKind::from_tag(reader.u8()?)?,
            offset: reader.u64()?,
            byte_len: reader.u64()?,
            section_digest: reader.u64()?,
        });
    }

    let unsupported_feature_count = reader.bounded_len(
        MAX_SCAN_DATABASE_UNSUPPORTED_FEATURES,
        "scan database unsupported feature count",
    )?;
    let mut unsupported_features = Vec::with_capacity(unsupported_feature_count);
    for _ in 0..unsupported_feature_count {
        unsupported_features.push(UnsupportedScanFeature {
            pattern_index: reader.u32()?,
            feature: reader.string()?,
        });
    }

    let compatibility = if reader.pos == bytes.len() {
        legacy_compatibility_record(&unsupported_features)
    } else {
        ScanDatabaseCompatibilityRecord {
            construct_tier_digest: reader.u64()?,
            dialect_digest: reader.u64()?,
            reader_compatibility: ScanDatabaseReaderCompatibility::from_tag(reader.u8()?)?,
        }
    };

    if reader.pos != bytes.len() {
        return Err(
            "scan database header has trailing bytes. Fix: split the header from table payload sections before decoding."
                .to_string(),
        );
    }

    Ok(ScanDatabaseHeader {
        pattern_set_digest,
        compiler_version,
        mode,
        table_sections,
        unsupported_features,
        compatibility,
    })
}

/// Decode and immediately validate compiler-version and mode compatibility.
///
/// # Errors
///
/// Returns the first decode or compatibility diagnostic.
pub fn decode_compatible_scan_database_header(
    bytes: &[u8],
    expected_compiler_version: &str,
    expected_mode: ScanDatabaseMode,
) -> Result<ScanDatabaseHeader, String> {
    let header = decode_scan_database_header(bytes)?;
    header.validate_compatible(expected_compiler_version, expected_mode)?;
    Ok(header)
}

/// Decode and validate compiler, mode, construct-tier, and dialect
/// compatibility.
///
/// # Errors
///
/// Returns the first decode or compatibility diagnostic.
pub fn decode_scan_database_header_with_compatibility(
    bytes: &[u8],
    expected_compiler_version: &str,
    expected_mode: ScanDatabaseMode,
    expected_construct_tier_digest: u64,
    expected_dialect_digest: u64,
    accepted_reader_compatibility: &[ScanDatabaseReaderCompatibility],
) -> Result<ScanDatabaseHeader, String> {
    let header =
        decode_compatible_scan_database_header(bytes, expected_compiler_version, expected_mode)?;
    header.validate_database_compatibility(
        expected_construct_tier_digest,
        expected_dialect_digest,
        accepted_reader_compatibility,
    )?;
    Ok(header)
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn legacy_compatibility_record(
    unsupported_features: &[UnsupportedScanFeature],
) -> ScanDatabaseCompatibilityRecord {
    ScanDatabaseCompatibilityRecord {
        construct_tier_digest: 0,
        dialect_digest: 0,
        reader_compatibility: if unsupported_features.is_empty() {
            ScanDatabaseReaderCompatibility::Compatible
        } else {
            ScanDatabaseReaderCompatibility::RequiresVerifier
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONSTRUCT_TIER_DIGEST: u64 = 0x5ca1_c075_7e12;
    const DIALECT_DIGEST: u64 = 0xd1a1_ec7;

    fn header() -> ScanDatabaseHeader {
        ScanDatabaseHeader {
            pattern_set_digest: [7u8; 32],
            compiler_version: "vyre-scan-compiler-test-v1".to_string(),
            mode: ScanDatabaseMode::Streaming,
            table_sections: vec![
                ScanDatabaseSectionHeader {
                    kind: ScanDatabaseSectionKind::LiteralTable,
                    offset: 128,
                    byte_len: 64,
                    section_digest: 0x11,
                },
                ScanDatabaseSectionHeader {
                    kind: ScanDatabaseSectionKind::AutomataTable,
                    offset: 192,
                    byte_len: 256,
                    section_digest: 0x12,
                },
            ],
            unsupported_features: vec![UnsupportedScanFeature {
                pattern_index: 3,
                feature: "Fix: unsupported backreference must stay verifier-only".to_string(),
            }],
            compatibility: ScanDatabaseCompatibilityRecord {
                construct_tier_digest: CONSTRUCT_TIER_DIGEST,
                dialect_digest: DIALECT_DIGEST,
                reader_compatibility: ScanDatabaseReaderCompatibility::RequiresVerifier,
            },
        }
    }

    #[test]
    fn scan_database_header_round_trips_all_fields() {
        let original = header();
        let bytes = encode_scan_database_header(&original).unwrap();
        let decoded = decode_compatible_scan_database_header(
            &bytes,
            "vyre-scan-compiler-test-v1",
            ScanDatabaseMode::Streaming,
        )
        .unwrap();

        assert_eq!(decoded, original);
        assert_eq!(decoded.section_count(), 2);
        assert_eq!(decoded.unsupported_feature_count(), 1);
    }

    #[test]
    fn scan_database_header_validates_construct_and_dialect_compatibility() {
        let bytes = encode_scan_database_header(&header()).unwrap();
        let decoded = decode_scan_database_header_with_compatibility(
            &bytes,
            "vyre-scan-compiler-test-v1",
            ScanDatabaseMode::Streaming,
            CONSTRUCT_TIER_DIGEST,
            DIALECT_DIGEST,
            &[ScanDatabaseReaderCompatibility::RequiresVerifier],
        )
        .unwrap();
        assert_eq!(
            decoded.compatibility.reader_compatibility,
            ScanDatabaseReaderCompatibility::RequiresVerifier
        );

        let construct_error = decode_scan_database_header_with_compatibility(
            &bytes,
            "vyre-scan-compiler-test-v1",
            ScanDatabaseMode::Streaming,
            CONSTRUCT_TIER_DIGEST + 1,
            DIALECT_DIGEST,
            &[ScanDatabaseReaderCompatibility::RequiresVerifier],
        )
        .unwrap_err();
        assert!(construct_error.contains("construct tier digest"));

        let dialect_error = decode_scan_database_header_with_compatibility(
            &bytes,
            "vyre-scan-compiler-test-v1",
            ScanDatabaseMode::Streaming,
            CONSTRUCT_TIER_DIGEST,
            DIALECT_DIGEST + 1,
            &[ScanDatabaseReaderCompatibility::RequiresVerifier],
        )
        .unwrap_err();
        assert!(dialect_error.contains("dialect digest"));
    }

    #[test]
    fn scan_database_header_rejects_unaccepted_reader_compatibility() {
        let bytes = encode_scan_database_header(&header()).unwrap();
        let error = decode_scan_database_header_with_compatibility(
            &bytes,
            "vyre-scan-compiler-test-v1",
            ScanDatabaseMode::Streaming,
            CONSTRUCT_TIER_DIGEST,
            DIALECT_DIGEST,
            &[ScanDatabaseReaderCompatibility::Compatible],
        )
        .unwrap_err();

        assert!(error.contains("reader compatibility"));
    }

    #[test]
    fn scan_database_header_decodes_legacy_headers_with_conservative_compatibility() {
        let mut legacy_bytes = encode_scan_database_header(&header()).unwrap();
        legacy_bytes.truncate(legacy_bytes.len() - 17);
        let decoded = decode_compatible_scan_database_header(
            &legacy_bytes,
            "vyre-scan-compiler-test-v1",
            ScanDatabaseMode::Streaming,
        )
        .unwrap();

        assert_eq!(decoded.compatibility.construct_tier_digest, 0);
        assert_eq!(decoded.compatibility.dialect_digest, 0);
        assert_eq!(
            decoded.compatibility.reader_compatibility,
            ScanDatabaseReaderCompatibility::RequiresVerifier
        );
        assert_eq!(decoded.unsupported_feature_count(), 1);
    }

    #[test]
    fn scan_database_header_rejects_incompatible_compiler_or_mode() {
        let bytes = encode_scan_database_header(&header()).unwrap();

        let compiler_error = decode_compatible_scan_database_header(
            &bytes,
            "vyre-scan-compiler-test-v2",
            ScanDatabaseMode::Streaming,
        )
        .unwrap_err();
        assert!(compiler_error.contains("compiler version"));

        let mode_error = decode_compatible_scan_database_header(
            &bytes,
            "vyre-scan-compiler-test-v1",
            ScanDatabaseMode::Block,
        )
        .unwrap_err();
        assert!(mode_error.contains("mode"));
    }

    #[test]
    fn scan_database_header_rejects_wrong_blob_family() {
        let error = decode_scan_database_header(b"VIR0").unwrap_err();
        assert!(error.contains("VSDH"));
    }
}
