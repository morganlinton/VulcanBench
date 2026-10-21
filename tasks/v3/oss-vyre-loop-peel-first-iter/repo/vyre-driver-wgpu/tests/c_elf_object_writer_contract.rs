//! GPU contracts for the C compiler ELF64 object writer.

#![cfg(feature = "c-parser")]

mod common;
use common::words_to_bytes;

use vyre::ir::Expr;
use vyre::VyreBackend;
use vyre_driver_wgpu::WgpuBackend;
use vyre_libs::compiler::object_writer::opt_lower_elf;

const TEXT_WORD_OFFSET: usize = 64;

fn bytes_to_words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

#[test]
fn gpu_emits_minimal_elf64_relocatable_container() {
    let backend = WgpuBackend::new()
        .expect("Fix: WgpuBackend::new failed on a machine that must have a GPU.");
    let encoded = [0xC0DE_0001, 0xC0DE_0002, 0xC0DE_0003, 0xC0DE_0004];
    let encoded_bytes = words_to_bytes(&encoded);
    let program = opt_lower_elf(
        "encoded_words",
        "object_words",
        Expr::u32(encoded.len() as u32),
    );
    let inputs: Vec<&[u8]> = vec![&encoded_bytes];
    let outputs = backend
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU ELF object writer dispatch must succeed");

    assert_eq!(
        outputs.len(),
        2,
        "object writer must expose object and scratch outputs"
    );
    let object = bytes_to_words(&outputs[0]);

    assert_eq!(object[0], 0x464C_457F, "ELF magic must be present");
    assert_eq!(
        object[1], 0x0001_0102,
        "ELF class/data/version must be ELF64 little-endian"
    );
    assert_eq!(object[4], 0x003E_0001, "object must be ET_REL for x86_64");
    assert_eq!(
        object[10], 0x40,
        "section table must start immediately after the ELF header"
    );
    assert_eq!(
        object[15], 0x0002_0003,
        "ELF must declare null, .text, and .shstrtab sections"
    );

    assert_eq!(object[32], 1, ".text sh_name must point into .shstrtab");
    assert_eq!(object[33], 1, ".text must be SHT_PROGBITS");
    assert_eq!(object[34], 0x6, ".text must be alloc+exec");
    assert_eq!(
        object[38],
        (TEXT_WORD_OFFSET as u32) * 4,
        ".text file offset must be byte-addressed"
    );
    assert_eq!(
        object[40],
        (encoded.len() as u32) * 4,
        ".text size must match encoded payload bytes"
    );
    assert_eq!(
        &object[TEXT_WORD_OFFSET..TEXT_WORD_OFFSET + encoded.len()],
        encoded.as_slice(),
        "encoded compiler words must be copied contiguously into .text"
    );

    let shstr = &object[TEXT_WORD_OFFSET + encoded.len()..TEXT_WORD_OFFSET + encoded.len() + 5];
    assert_eq!(
        shstr,
        [0x6574_2E00, 0x2E00_7478, 0x7473_6873, 0x6261_7472, 0].as_slice(),
        ".shstrtab must contain NUL, .text, and .shstrtab"
    );
}
