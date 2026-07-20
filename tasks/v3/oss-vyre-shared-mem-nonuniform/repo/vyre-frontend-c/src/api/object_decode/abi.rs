use std::path::Path;

use super::{common::decode_u32_words_for_section, decode_embedded_object, read_object_file};
use crate::object_format::{SectionTag, Vyrecob2};

/// Decoded ABI layout table from a `vyre-frontend-c` object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CObjectAbiLayout {
    /// VYRECOB2 container version.
    pub vyrecob2_version: u32,
    /// Dense ABI type-kind table consumed by `c11_compute_alignments`.
    pub type_kinds: Vec<u32>,
    /// One size/alignment row per GPU ABI type slot.
    pub entries: Vec<CObjectAbiLayoutEntry>,
}

/// One ABI layout row emitted by `c11_compute_alignments`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CObjectAbiLayoutEntry {
    /// Dense ABI type-table index.
    pub type_index: u32,
    /// ABI type kind from the required `AbiTypes` section.
    pub type_kind: u32,
    /// Size in bytes.
    pub size: u32,
    /// Alignment in bytes.
    pub align: u32,
}

/// Decode ABI layout rows from a compiled `vyre-frontend-c` object.
pub fn decode_object_abi_layout(object_bytes: &[u8]) -> Result<CObjectAbiLayout, String> {
    decode_embedded_object(object_bytes, decode_object_abi_layout_from_container)
}

pub(crate) fn decode_object_abi_layout_from_container(
    container: &Vyrecob2<'_>,
) -> Result<CObjectAbiLayout, String> {
    let abi_section = container.section(SectionTag::AbiLayout).ok_or_else(|| {
        "vyre-frontend-c object is missing AbiLayout. Fix: compile with ABI layout emission enabled."
            .to_string()
    })?;
    let type_section = container.section(SectionTag::AbiTypes).ok_or_else(|| {
        "vyre-frontend-c object is missing AbiTypes. Fix: regenerate the object; do not decode ABI layout without exact type-kind metadata."
            .to_string()
    })?;
    let type_kinds = decode_u32_words_for_section(type_section, "AbiTypes")?;
    let entries = decode_abi_layout_entries(abi_section, &type_kinds)?;
    Ok(CObjectAbiLayout {
        vyrecob2_version: container.version,
        type_kinds,
        entries,
    })
}

/// Read and decode ABI layout rows from a compiled object path.
pub fn decode_object_abi_layout_file(path: &Path) -> Result<CObjectAbiLayout, String> {
    read_object_file(path, decode_object_abi_layout)
}

fn decode_abi_layout_entries(
    section: &[u8],
    type_kinds: &[u32],
) -> Result<Vec<CObjectAbiLayoutEntry>, String> {
    if section.len() % 8 != 0 {
        return Err(format!(
            "vyre-frontend-c AbiLayout section length {} is not size/align paired u32 rows. Fix: regenerate the object.",
            section.len()
        ));
    }
    let count = section.len() / 8;
    if type_kinds.len() != count {
        return Err(format!(
            "vyre-frontend-c AbiTypes row count {} does not match AbiLayout row count {count}. Fix: regenerate the object; do not decode truncated ABI type metadata as kind 0.",
            type_kinds.len()
        ));
    }
    let split = count.checked_mul(4).ok_or_else(|| {
        "vyre-frontend-c AbiLayout split offset overflows usize. Fix: split the type table."
            .to_string()
    })?;
    let (sizes, aligns) = section.split_at(split);
    let mut entries = Vec::with_capacity(count);
    for idx in 0..count {
        let off = idx.checked_mul(4).ok_or_else(|| {
            "vyre-frontend-c AbiLayout row byte offset overflows usize. Fix: split the type table."
                .to_string()
        })?;
        let size = read_u32_word(sizes, off, "ABI size")?;
        let align = read_u32_word(aligns, off, "ABI alignment")?;
        let type_index = u32::try_from(idx).map_err(|_| {
            "vyre-frontend-c AbiLayout row index exceeds u32. Fix: split the type table."
                .to_string()
        })?;
        entries.push(CObjectAbiLayoutEntry {
            type_index,
            type_kind: type_kinds[idx],
            size,
            align,
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::decode_abi_layout_entries;

    fn words(bytes: &[u32]) -> Vec<u8> {
        bytes.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    #[test]
    fn abi_layout_rejects_truncated_present_type_kinds() {
        let mut section = words(&[4, 8]);
        section.extend(words(&[4, 8]));
        let err = decode_abi_layout_entries(&section, &[17])
            .expect_err("present AbiTypes must match AbiLayout row count");
        assert!(
            err.contains("does not match AbiLayout row count"),
            "error must explain the row-count mismatch"
        );
    }

    #[test]
    fn abi_layout_missing_type_kinds_are_rejected() {
        let mut section = words(&[4]);
        section.extend(words(&[4]));
        let err = decode_abi_layout_entries(&section, &[])
            .expect_err("AbiLayout without AbiTypes must not decode");
        assert!(
            err.contains("does not match AbiLayout row count"),
            "missing AbiTypes must be rejected instead of defaulting kind 0"
        );
    }

    #[test]
    fn abi_layout_present_type_kinds_are_attached_exactly() {
        let mut section = words(&[4, 8]);
        section.extend(words(&[4, 8]));
        let entries = decode_abi_layout_entries(&section, &[17, 23])
            .expect("Fix: matching AbiTypes and AbiLayout rows must decode");
        assert_eq!(entries[0].type_kind, 17);
        assert_eq!(entries[1].type_kind, 23);
    }
}

fn read_u32_word(bytes: &[u8], offset: usize, label: &str) -> Result<u32, String> {
    // Canonical LEGO: byte-offset LE u32 read lives once in vyre-primitives::wire.
    vyre_primitives::wire::read_u32_le_at(bytes, offset, label)
}
