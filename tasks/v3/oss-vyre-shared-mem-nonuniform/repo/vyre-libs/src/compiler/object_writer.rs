use crate::region::{wrap_anonymous, wrap_child};
use vyre_foundation::ir::model::expr::GeneratorRef;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Registered op id for GPU ELF container emission.
pub const ELF_LOWERING_OP_ID: &str = "vyre-libs::parsing::opt_lower_elf";

const ELF64_HEADER_WORDS: u32 = 16;
const ELF64_SECTION_HEADER_WORDS: u32 = 16;
const ELF_SECTION_COUNT: u32 = 3;
const TEXT_SECTION_INDEX: u32 = 1;
const SHSTRTAB_SECTION_INDEX: u32 = 2;
const TEXT_SECTION_WORD_OFFSET: u32 =
    ELF64_HEADER_WORDS + ELF64_SECTION_HEADER_WORDS * ELF_SECTION_COUNT;

/// Phase 4: ELF64 relocatable container emission.
///
/// Copies already-encoded 32-bit compiler words into a valid ELF64 ET_REL
/// `.text` section and emits the section table plus `.shstrtab` in VRAM.
#[must_use]
pub fn opt_lower_elf(ssa_nodes: &str, target_object_bytes: &str, num_nodes: Expr) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let node_count = match &num_nodes {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    // The output buffer must hold TEXT_SECTION_WORD_OFFSET + node_count + 5
    // words: the text section starts at TEXT_SECTION_WORD_OFFSET, the five
    // consecutive .shstrtab stores follow immediately after the last text word.
    // Using 4096 as the floor keeps small programs from allocating a tiny
    // buffer; the ceiling is not capped because a caller with node_count >=
    // 4028 would silently write past index 4095 otherwise.
    let required_words = TEXT_SECTION_WORD_OFFSET
        .saturating_add(node_count)
        .saturating_add(5);
    let buf_words = required_words.max(4096);
    let visible_object_bytes = (required_words as usize) * 4;

    let loop_body = vec![
        Node::let_bind("encoded_word", Expr::load(ssa_nodes, t.clone())),
        Node::let_bind(
            "write_offset",
            Expr::add(Expr::u32(TEXT_SECTION_WORD_OFFSET), t.clone()),
        ),
        Node::store(
            target_object_bytes,
            Expr::var("write_offset"),
            Expr::var("encoded_word"),
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(ssa_nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(node_count),
            BufferDecl::storage(
                target_object_bytes,
                1,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(buf_words)
            .with_output_byte_range(0..visible_object_bytes),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            ELF_LOWERING_OP_ID,
            vec![wrap_child(
                vyre_primitives::decode::rle_segment_lengths::OP_ID,
                GeneratorRef {
                    name: ELF_LOWERING_OP_ID.to_string(),
                },
                vec![
                    Node::if_then(
                        Expr::eq(t.clone(), Expr::u32(0)),
                        vec![
                            // e_ident (16 bytes)
                            Node::store(target_object_bytes, Expr::u32(0), Expr::u32(0x464C457F)), // "\x7fELF"
                            Node::store(target_object_bytes, Expr::u32(1), Expr::u32(0x00010102)), // 64-bit, LSB, SystemV ABI
                            Node::store(target_object_bytes, Expr::u32(2), Expr::u32(0x00000000)), // Padding
                            Node::store(target_object_bytes, Expr::u32(3), Expr::u32(0x00000000)), // Padding
                            // e_type, e_machine, e_version
                            Node::store(target_object_bytes, Expr::u32(4), Expr::u32(0x003E0001)), // ET_REL (1), EM_X86_64 (62)
                            Node::store(target_object_bytes, Expr::u32(5), Expr::u32(0x00000001)), // Version 1
                            // e_entry (8 bytes), e_phoff (8 bytes)
                            Node::store(target_object_bytes, Expr::u32(6), Expr::u32(0x00000000)),
                            Node::store(target_object_bytes, Expr::u32(7), Expr::u32(0x00000000)),
                            Node::store(target_object_bytes, Expr::u32(8), Expr::u32(0x00000000)), // No Program Headers
                            Node::store(target_object_bytes, Expr::u32(9), Expr::u32(0x00000000)),
                            // e_shoff (8 bytes) -> Section Header Table immediately follows header (64)
                            Node::store(target_object_bytes, Expr::u32(10), Expr::u32(0x00000040)),
                            Node::store(target_object_bytes, Expr::u32(11), Expr::u32(0x00000000)),
                            // e_flags (4 bytes)
                            Node::store(target_object_bytes, Expr::u32(12), Expr::u32(0x00000000)),
                            // e_ehsize (2), e_phentsize (2), e_phnum (2), e_shentsize (2)
                            Node::store(target_object_bytes, Expr::u32(13), Expr::u32(0x00000040)), // Ehdr size = 64
                            Node::store(target_object_bytes, Expr::u32(14), Expr::u32(0x00400000)), // Phnum = 0, Shdr size = 64
                            // e_shnum (2), e_shstrndx (2)
                            Node::store(
                                target_object_bytes,
                                Expr::u32(15),
                                Expr::u32((SHSTRTAB_SECTION_INDEX << 16) | ELF_SECTION_COUNT),
                            ),
                            // Section 0: SHT_NULL, all zero.
                            Node::loop_for(
                                "null_sh_word",
                                Expr::u32(0),
                                Expr::u32(ELF64_SECTION_HEADER_WORDS),
                                vec![Node::store(
                                    target_object_bytes,
                                    Expr::add(
                                        Expr::u32(ELF64_HEADER_WORDS),
                                        Expr::var("null_sh_word"),
                                    ),
                                    Expr::u32(0),
                                )],
                            ),
                            // Section 1: .text
                            Node::store(
                                target_object_bytes,
                                Expr::u32(
                                    ELF64_HEADER_WORDS
                                        + TEXT_SECTION_INDEX * ELF64_SECTION_HEADER_WORDS,
                                ),
                                Expr::u32(1),
                            ), // sh_name = ".text"
                            Node::store(
                                target_object_bytes,
                                Expr::u32(
                                    ELF64_HEADER_WORDS
                                        + TEXT_SECTION_INDEX * ELF64_SECTION_HEADER_WORDS
                                        + 1,
                                ),
                                Expr::u32(1),
                            ), // SHT_PROGBITS
                            Node::store(
                                target_object_bytes,
                                Expr::u32(
                                    ELF64_HEADER_WORDS
                                        + TEXT_SECTION_INDEX * ELF64_SECTION_HEADER_WORDS
                                        + 2,
                                ),
                                Expr::u32(0x6),
                            ), // SHF_ALLOC | SHF_EXECINSTR
                            Node::store(
                                target_object_bytes,
                                Expr::u32(
                                    ELF64_HEADER_WORDS
                                        + TEXT_SECTION_INDEX * ELF64_SECTION_HEADER_WORDS
                                        + 6,
                                ),
                                Expr::u32(TEXT_SECTION_WORD_OFFSET * 4),
                            ),
                            Node::store(
                                target_object_bytes,
                                Expr::u32(
                                    ELF64_HEADER_WORDS
                                        + TEXT_SECTION_INDEX * ELF64_SECTION_HEADER_WORDS
                                        + 8,
                                ),
                                Expr::u32(node_count * 4),
                            ),
                            Node::store(
                                target_object_bytes,
                                Expr::u32(
                                    ELF64_HEADER_WORDS
                                        + TEXT_SECTION_INDEX * ELF64_SECTION_HEADER_WORDS
                                        + 12,
                                ),
                                Expr::u32(4),
                            ),
                            // Section 2: .shstrtab
                            Node::store(
                                target_object_bytes,
                                Expr::u32(
                                    ELF64_HEADER_WORDS
                                        + SHSTRTAB_SECTION_INDEX * ELF64_SECTION_HEADER_WORDS,
                                ),
                                Expr::u32(7),
                            ), // sh_name = ".shstrtab"
                            Node::store(
                                target_object_bytes,
                                Expr::u32(
                                    ELF64_HEADER_WORDS
                                        + SHSTRTAB_SECTION_INDEX * ELF64_SECTION_HEADER_WORDS
                                        + 1,
                                ),
                                Expr::u32(3),
                            ), // SHT_STRTAB
                            Node::store(
                                target_object_bytes,
                                Expr::u32(
                                    ELF64_HEADER_WORDS
                                        + SHSTRTAB_SECTION_INDEX * ELF64_SECTION_HEADER_WORDS
                                        + 6,
                                ),
                                Expr::u32((TEXT_SECTION_WORD_OFFSET + node_count) * 4),
                            ),
                            Node::store(
                                target_object_bytes,
                                Expr::u32(
                                    ELF64_HEADER_WORDS
                                        + SHSTRTAB_SECTION_INDEX * ELF64_SECTION_HEADER_WORDS
                                        + 8,
                                ),
                                Expr::u32(17),
                            ),
                            Node::store(
                                target_object_bytes,
                                Expr::u32(
                                    ELF64_HEADER_WORDS
                                        + SHSTRTAB_SECTION_INDEX * ELF64_SECTION_HEADER_WORDS
                                        + 12,
                                ),
                                Expr::u32(1),
                            ),
                            Node::store(
                                target_object_bytes,
                                Expr::u32(TEXT_SECTION_WORD_OFFSET + node_count),
                                Expr::u32(0x65742E00),
                            ), // "\0.te"
                            Node::store(
                                target_object_bytes,
                                Expr::u32(TEXT_SECTION_WORD_OFFSET + node_count + 1),
                                Expr::u32(0x2E007478),
                            ), // "xt\0."
                            Node::store(
                                target_object_bytes,
                                Expr::u32(TEXT_SECTION_WORD_OFFSET + node_count + 2),
                                Expr::u32(0x74736873),
                            ), // "shst"
                            Node::store(
                                target_object_bytes,
                                Expr::u32(TEXT_SECTION_WORD_OFFSET + node_count + 3),
                                Expr::u32(0x62617472),
                            ), // "rtab"
                            Node::store(
                                target_object_bytes,
                                Expr::u32(TEXT_SECTION_WORD_OFFSET + node_count + 4),
                                Expr::u32(0),
                            ),
                        ],
                    ),
                    Node::if_then(Expr::lt(t.clone(), num_nodes), loop_body),
                ],
            )],
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: the five .shstrtab stores are at indices
    /// TEXT_SECTION_WORD_OFFSET + node_count + {0..4}.  When node_count >= 4028
    /// the last store would land at or past index 4096 if the buffer were capped
    /// at 4096 words.  Verify that the declared buffer is always large enough.
    #[test]
    fn elf_shstrtab_stores_stay_in_bounds_at_max_node_count() {
        let node_count = 4028u32;
        let program = opt_lower_elf("ssa", "obj", Expr::u32(node_count));
        // The buffer for `obj` must have at least TEXT_SECTION_WORD_OFFSET + node_count + 5
        // words so none of the five .shstrtab stores write out of bounds.
        let required = (TEXT_SECTION_WORD_OFFSET + node_count + 5) as usize;
        let obj_buf_count = program
            .buffers()
            .iter()
            .find(|b| b.name() == "obj")
            .map(|b| b.count() as usize)
            .expect("obj buffer must be present in the ELF-lowering program");
        assert!(
            obj_buf_count >= required,
            "obj buffer count {obj_buf_count} < required {required}; \
             shstrtab stores at TEXT_SECTION_WORD_OFFSET({TEXT_SECTION_WORD_OFFSET}) \
             + node_count({node_count}) + 4 = {} would write OOB",
            TEXT_SECTION_WORD_OFFSET + node_count + 4
        );
    }

    /// With the fix, a large node_count still declares a strictly larger buffer;
    /// the visible_object_bytes is exactly required_words * 4, not capped.
    #[test]
    fn elf_output_byte_range_covers_shstrtab_tail() {
        // node_count = 4000 fits in the original 4096-word buffer; check the
        // output byte range is tight (required_words, not 4096).
        let node_count = 4000u32;
        let required_bytes = ((TEXT_SECTION_WORD_OFFSET + node_count + 5) as usize) * 4;
        let program = opt_lower_elf("ssa", "obj", Expr::u32(node_count));
        let obj_buf = program
            .buffers()
            .iter()
            .find(|b| b.name() == "obj")
            .expect("obj buffer must be present");
        let range = obj_buf
            .output_byte_range()
            .expect("obj buffer must have an output_byte_range");
        assert_eq!(
            range.end, required_bytes,
            "output_byte_range end must equal required_words*4 = {required_bytes}, got {}",
            range.end
        );
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: ELF_LOWERING_OP_ID,
        build: || opt_lower_elf("ssa", "obj", Expr::u32(4)),
        // Small deterministic fixture: 4 encoded words and a 4096-word object buffer.
        test_inputs: Some(|| vec![vec![
            vyre_primitives::wire::pack_u32_slice(&[
                0xC0DE_0001u32,
                0xC0DE_0002,
                0xC0DE_0003,
                0xC0DE_0004,
            ]),
            vec![0u8; 4_096 * 4],
        ]]),
        expected_output: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            let mut obj = vec![0u32; 4096];
            obj[0] = 0x464C_457F;
            obj[1] = 0x0001_0102;
            obj[4] = 0x003E_0001;
            obj[5] = 0x0000_0001;
            obj[10] = 0x0000_0040;
            obj[13] = 0x0000_0040;
            obj[14] = 0x0040_0000;
            obj[15] = 0x0002_0003;
            obj[32] = 1;
            obj[33] = 1;
            obj[34] = 0x6;
            obj[38] = TEXT_SECTION_WORD_OFFSET * 4;
            obj[40] = 16;
            obj[44] = 4;
            obj[48] = 7;
            obj[49] = 3;
            obj[54] = (TEXT_SECTION_WORD_OFFSET + 4) * 4;
            obj[56] = 17;
            obj[60] = 1;
            obj[64] = 0xC0DE_0001;
            obj[65] = 0xC0DE_0002;
            obj[66] = 0xC0DE_0003;
            obj[67] = 0xC0DE_0004;
            obj[68] = 0x6574_2E00;
            obj[69] = 0x2E00_7478;
            obj[70] = 0x7473_6873;
            obj[71] = 0x6261_7472;
            obj.truncate((TEXT_SECTION_WORD_OFFSET + 4 + 5) as usize);
            vec![vec![to_bytes(&obj)]]
        }),
        category: Some("compiler"),
    }
}
