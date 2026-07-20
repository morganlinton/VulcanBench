//! VAST header constants and decoding.

use super::error::VastError;
use super::node::NODE_STRIDE_U32;

/// Magic bytes at offset 0.
pub const VAST_MAGIC: [u8; 4] = *b"VAST";
/// Wire version carried in the header (`u16`, little-endian).
pub const VAST_VERSION: u16 = 0;
/// Fixed header size in bytes (see [`VastHeader::decode`]).
pub const HEADER_LEN: usize = 24;

/// Parsed fixed header (does not borrow the source buffer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VastHeader {
    /// Wire version (must be [`VAST_VERSION`] for this module).
    pub version: u16,
    /// Language discriminator (opaque to generic walks).
    pub source_lang: u16,
    /// Number of [`crate::vast::VastNode`] rows.
    pub node_count: u32,
    /// Number of file metadata rows (`12` bytes each) after the node table.
    pub file_count: u32,
    /// Byte length of the `string_blob` region.
    pub string_blob_len: u32,
    /// Byte length of the `attr_blob` region.
    pub attr_blob_len: u32,
}

impl VastHeader {
    /// Decode header from the first 24 bytes.
    /// Decode header from the first 24 bytes.
    ///
    /// # Errors
    ///
    /// Returns [`VastError`] when the buffer is too short or the header fields are invalid.
    pub fn decode(bytes: &[u8]) -> Result<Self, VastError> {
        if bytes.len() < HEADER_LEN {
            return Err(VastError::TooShort {
                need: HEADER_LEN,
                got: bytes.len(),
            });
        }
        if bytes[0..4] != VAST_MAGIC {
            let m: [u8; 4] = bytes[0..4].try_into().unwrap_or([0; 4]);
            return Err(VastError::BadMagic(m));
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != VAST_VERSION {
            return Err(VastError::UnsupportedVersion(version));
        }
        let source_lang = u16::from_le_bytes([bytes[6], bytes[7]]);
        let node_count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let file_count = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let string_blob_len = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let attr_blob_len = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        Ok(Self {
            version,
            source_lang,
            node_count,
            file_count,
            string_blob_len,
            attr_blob_len,
        })
    }

    /// Total byte length implied by this header + tables + blobs.
    #[must_use]
    pub fn total_byte_len(self) -> Option<usize> {
        let nodes = (self.node_count as usize).checked_mul(NODE_STRIDE_U32 * 4)?;
        let files = (self.file_count as usize).checked_mul(12)?;
        let strings = self.string_blob_len as usize;
        let attrs = self.attr_blob_len as usize;
        HEADER_LEN
            .checked_add(nodes)?
            .checked_add(files)?
            .checked_add(strings)?
            .checked_add(attrs)
    }
}
