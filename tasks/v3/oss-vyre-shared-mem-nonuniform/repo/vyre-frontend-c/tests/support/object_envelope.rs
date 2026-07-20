// Integration test module for the containing Vyre package.

use super::object::{read_u32, u32_words_from_bytes, MAGIC};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjectFlavor {
    RawVyrecob2,
    Elf,
}

/// Portable wrapper around a raw VYRECOB2 payload (or an ELF object that embeds one)
/// providing ergonomic section assertions for integration tests.
pub(crate) struct ObjectEnvelope {
    bytes: Vec<u8>,
    payload_offset: usize,
    flavor: ObjectFlavor,
}

impl ObjectEnvelope {
    pub(crate) fn detect(bytes: Vec<u8>) -> Self {
        if bytes.starts_with(MAGIC) {
            return Self::from_payload(bytes);
        }
        if bytes.starts_with(b"\x7fELF") {
            return Self::from_elf(bytes);
        }
        panic!("object carrier must be raw VYRECOB2 or ELF with embedded VYRECOB2 payload");
    }

    /// Wrap a raw VYRECOB2 payload directly.
    pub(crate) fn from_payload(bytes: Vec<u8>) -> Self {
        assert!(
            bytes.starts_with(MAGIC),
            "expected raw payload to start with VYRECOB2 magic"
        );
        Self {
            bytes,
            payload_offset: 0,
            flavor: ObjectFlavor::RawVyrecob2,
        }
    }

    /// Wrap a full ELF object, locating the embedded VYRECOB2 payload.
    pub(crate) fn from_elf(bytes: Vec<u8>) -> Self {
        assert!(
            bytes.starts_with(b"\x7fELF"),
            "expected ELF carrier magic at the start of the object"
        );
        let offset = bytes
            .windows(MAGIC.len())
            .position(|window| window == MAGIC)
            .expect("expected ELF carrier to embed a VYRECOB2 payload");
        Self {
            bytes,
            payload_offset: offset,
            flavor: ObjectFlavor::Elf,
        }
    }

    pub(crate) fn flavor(&self) -> ObjectFlavor {
        self.flavor
    }

    pub(crate) fn payload(&self) -> &[u8] {
        &self.bytes[self.payload_offset..]
    }

    pub(crate) fn version(&self) -> u32 {
        let mut offset = MAGIC.len();
        read_u32(self.payload(), &mut offset)
    }

    pub(crate) fn section_count(&self) -> u32 {
        let mut offset = MAGIC.len() + 4;
        read_u32(self.payload(), &mut offset)
    }

    pub(crate) fn section_tags(&self) -> Vec<u32> {
        let mut offset = MAGIC.len() + 4;
        let count = read_u32(self.payload(), &mut offset) as usize;
        let mut tags = Vec::with_capacity(count);
        for _ in 0..count {
            let tag = read_u32(self.payload(), &mut offset);
            let len = read_u32(self.payload(), &mut offset) as usize;
            tags.push(tag);
            offset = offset.saturating_add(len);
            assert!(
                offset <= self.payload().len(),
                "section length stays inside VYRECOB2 payload"
            );
        }
        tags
    }

    pub(crate) fn section(&self, wanted: u32) -> Option<&[u8]> {
        let mut offset = MAGIC.len() + 4;
        let section_count = read_u32(self.payload(), &mut offset);
        for _ in 0..section_count {
            let tag = read_u32(self.payload(), &mut offset);
            let len = read_u32(self.payload(), &mut offset) as usize;
            let end = offset.saturating_add(len);
            assert!(
                end <= self.payload().len(),
                "section length stays inside VYRECOB2 payload"
            );
            if tag == wanted {
                return Some(&self.payload()[offset..end]);
            }
            offset = end;
        }
        None
    }

    pub(crate) fn section_len(&self, wanted: u32) -> usize {
        self.section(wanted).map(|s| s.len()).unwrap_or(0)
    }

    pub(crate) fn assert_magic(&self) {
        assert_eq!(
            &self.payload()[0..MAGIC.len()],
            MAGIC,
            "VYRECOB2 magic mismatch"
        );
    }

    pub(crate) fn assert_carrier(&self) {
        match self.flavor {
            ObjectFlavor::RawVyrecob2 => assert_eq!(&self.bytes[0..MAGIC.len()], MAGIC),
            ObjectFlavor::Elf => assert_eq!(&self.bytes[0..4], b"\x7fELF"),
        }
        self.assert_magic();
    }

    pub(crate) fn assert_version(&self, expected: u32) {
        assert_eq!(self.version(), expected, "VYRECOB2 version mismatch");
    }

    pub(crate) fn assert_section_present(&self, tag: u32) {
        assert!(
            self.section(tag).is_some(),
            "expected VYRECOB2 section {tag} to be present"
        );
    }

    pub(crate) fn assert_section_absent(&self, tag: u32) {
        assert!(
            self.section(tag).is_none(),
            "expected VYRECOB2 section {tag} to be absent"
        );
    }

    pub(crate) fn assert_section_bytes(&self, tag: u32, expected: &[u8]) {
        let actual = self
            .section(tag)
            .unwrap_or_else(|| panic!("missing VYRECOB2 section {tag}"));
        assert_eq!(actual, expected, "VYRECOB2 section {tag} bytes mismatch");
    }

    pub(crate) fn assert_section_words(&self, tag: u32, expected: &[u32]) {
        let actual = self
            .section(tag)
            .unwrap_or_else(|| panic!("missing VYRECOB2 section {tag}"));
        let words = u32_words_from_bytes(actual);
        assert_eq!(words, expected, "VYRECOB2 section {tag} words mismatch");
    }

    pub(crate) fn words(&self, tag: u32) -> Vec<u32> {
        u32_words_from_bytes(
            self.section(tag)
                .unwrap_or_else(|| panic!("missing VYRECOB2 section {tag}")),
        )
    }
}
