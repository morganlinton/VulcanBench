//! Linux-native **ELF64 relocatable** objects so `cc` / `ld` accept `vyre-frontend-c` outputs.
//!
//! Each translation unit is emitted as ET_REL with a one-instruction `.text`
//! carrier and a custom section holding the full `VYRECOB2` blob. Link mode uses a tiny `_start`
//! object (`exit(0)` syscall) plus `-nostdlib`.

use std::path::Path;

use object::write::{Object, StandardSection, Symbol, SymbolSection};
use object::{
    Architecture, BinaryFormat, Endianness, SectionKind, SymbolFlags, SymbolKind, SymbolScope,
};

use crate::hash::blake3_128;

fn section_name_for_tu(source: &Path) -> Vec<u8> {
    let tag = blake3_128(source.as_os_str().as_encoded_bytes());
    let mut name = String::from(".vyrecob2.");
    for byte in tag {
        use std::fmt::Write as _;

        let _ = write!(&mut name, "{byte:02x}");
    }
    name.into_bytes()
}

/// x86_64 ET_REL: `.text` = `ret`, custom section = `vyrecob2` payload,
/// local carrier symbol.
pub fn emit_translation_unit_relocatable(
    vyrecob2: &[u8],
    source_path: &Path,
) -> Result<Vec<u8>, String> {
    match std::env::consts::ARCH {
        "x86_64" => emit_tu_x86_64(vyrecob2, source_path),
        "aarch64" => emit_tu_aarch64(vyrecob2, source_path),
        other => Err(format!(
            "vyre-frontend-c: ELF emission is unsupported for host arch `{other}` (supported: x86_64, aarch64)"
        )),
    }
}

fn emit_tu_x86_64(vyrecob2: &[u8], source_path: &Path) -> Result<Vec<u8>, String> {
    let mut obj = Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);
    let text = obj.section_id(StandardSection::Text);
    let off = obj.append_section_data(text, &[0xC3], 1); // ret
    obj.add_symbol(Symbol {
        name: b"vyre_tu_entry".to_vec(),
        value: off,
        size: 1,
        kind: SymbolKind::Text,
        scope: SymbolScope::Compilation,
        weak: false,
        section: SymbolSection::Section(text),
        flags: SymbolFlags::None,
    });

    let sec_name = section_name_for_tu(source_path);
    let vsec = obj.add_section(Vec::new(), sec_name, SectionKind::Data);
    obj.append_section_data(vsec, vyrecob2, 1);

    obj.write().map_err(|e| e.to_string())
}

fn emit_tu_aarch64(vyrecob2: &[u8], source_path: &Path) -> Result<Vec<u8>, String> {
    let mut obj = Object::new(BinaryFormat::Elf, Architecture::Aarch64, Endianness::Little);
    let text = obj.section_id(StandardSection::Text);
    // aarch64: `ret` = 0xd65f03c0
    let off = obj.append_section_data(text, &[0xC0, 0x03, 0x5F, 0xD6], 4);
    obj.add_symbol(Symbol {
        name: b"vyre_tu_entry".to_vec(),
        value: off,
        size: 4,
        kind: SymbolKind::Text,
        scope: SymbolScope::Compilation,
        weak: false,
        section: SymbolSection::Section(text),
        flags: SymbolFlags::None,
    });

    let sec_name = section_name_for_tu(source_path);
    let vsec = obj.add_section(Vec::new(), sec_name, SectionKind::Data);
    obj.append_section_data(vsec, vyrecob2, 1);

    obj.write().map_err(|e| e.to_string())
}

/// Minimal relocatable object defining global `_start` as `exit(0)` for the host arch.
pub fn emit_link_startup_relocatable() -> Result<Vec<u8>, String> {
    match std::env::consts::ARCH {
        "x86_64" => emit_start_x86_64(),
        "aarch64" => emit_start_aarch64(),
        other => Err(format!(
            "vyre-frontend-c: link startup object is unsupported for `{other}` (supported: x86_64, aarch64)"
        )),
    }
}

/// Linux x86_64: `mov $60,%rax; xor %edi,%edi; syscall` (exit(0)).
fn emit_start_x86_64() -> Result<Vec<u8>, String> {
    let mut obj = Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);
    let text = obj.section_id(StandardSection::Text);
    // mov $60,%rax (7) + xor %edi,%edi (3) + syscall (2) = 12 bytes
    let code: [u8; 12] = [
        0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00, 0x48, 0x31, 0xff, 0x0f, 0x05,
    ];
    let off = obj.append_section_data(text, &code, 1);
    obj.add_symbol(Symbol {
        name: b"_start".to_vec(),
        value: off,
        size: code.len() as u64,
        kind: SymbolKind::Text,
        scope: SymbolScope::Linkage,
        weak: false,
        section: SymbolSection::Section(text),
        flags: SymbolFlags::None,
    });
    obj.write().map_err(|e| e.to_string())
}

/// Linux aarch64: `mov x8, #93; mov x0, #0; svc #0` (exit(0)).
fn emit_start_aarch64() -> Result<Vec<u8>, String> {
    let mut obj = Object::new(BinaryFormat::Elf, Architecture::Aarch64, Endianness::Little);
    let text = obj.section_id(StandardSection::Text);
    // movz x8, #93  ->  d2801248 in standard encoding
    // movz x0, #0   ->  d2800000
    // svc #0        ->  d4000001
    let code: [u8; 12] = [
        0x48, 0x12, 0x80, 0xd2, // mov x8, #93
        0x00, 0x00, 0x80, 0xd2, // mov x0, #0
        0x01, 0x00, 0x00, 0xd4, // svc #0
    ];
    let off = obj.append_section_data(text, &code, 4);
    obj.add_symbol(Symbol {
        name: b"_start".to_vec(),
        value: off,
        size: code.len() as u64,
        kind: SymbolKind::Text,
        scope: SymbolScope::Linkage,
        weak: false,
        section: SymbolSection::Section(text),
        flags: SymbolFlags::None,
    });
    obj.write().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tu_object_has_elf_magic() {
        let bytes = emit_translation_unit_relocatable(b"VYREC02\0", Path::new("x.c")).unwrap();
        assert_eq!(&bytes[0..4], b"\x7fELF");
    }

    #[test]
    fn startup_object_has_elf_magic() {
        let bytes = emit_link_startup_relocatable().unwrap();
        assert_eq!(&bytes[0..4], b"\x7fELF");
    }

    #[test]
    fn tu_section_name_uses_128_bit_path_tag() {
        let name = section_name_for_tu(Path::new("src/main.c"));
        let name = std::str::from_utf8(&name).expect("Fix: section name must be ASCII");
        assert!(name.starts_with(".vyrecob2."));
        assert_eq!(name.len(), ".vyrecob2.".len() + 32);
    }
}
