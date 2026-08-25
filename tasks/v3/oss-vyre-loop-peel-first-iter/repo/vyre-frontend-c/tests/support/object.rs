// Integration test module for the containing Vyre package.

use std::fs;
use std::path::PathBuf;

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::{compile, VyreCompileOptions};
use vyre_frontend_c::tu_host::prepare_resident_translation_unit_source;

pub(crate) const MAGIC: &[u8; 8] = b"VYREC02\0";
pub(crate) const SECTION_LEX: u32 = 1;
pub(crate) const SECTION_PAREN_PAIRS: u32 = 2;
pub(crate) const SECTION_BRACE_PAIRS: u32 = 3;
pub(crate) const SECTION_FUNCTIONS: u32 = 4;
pub(crate) const SECTION_CALLS: u32 = 5;
pub(crate) const SECTION_PREPROC_MASK: u32 = 7;
pub(crate) const SECTION_MACRO_TYPES: u32 = 8;
pub(crate) const SECTION_AST: u32 = 10;
pub(crate) const SECTION_CFG: u32 = 11;
pub(crate) const SECTION_VAST: u32 = 13;
pub(crate) const SECTION_PROGRAM_GRAPH: u32 = 14;
pub(crate) const SECTION_SEMA_SCOPE: u32 = 15;
pub(crate) const SECTION_EXPRESSION_SHAPE: u32 = 16;
pub(crate) const SECTION_SEMANTIC_PROGRAM_GRAPH_NODES: u32 = 17;
pub(crate) const SECTION_SEMANTIC_PROGRAM_GRAPH_EDGES: u32 = 18;

pub(crate) const VAST_STRIDE_U32: usize = 10;
pub(crate) const PG_STRIDE_U32: usize = 6;
pub(crate) const SEMANTIC_PG_NODE_STRIDE_U32: usize = 10;
pub(crate) const SEMANTIC_PG_EDGE_STRIDE_U32: usize = 6;
pub(crate) const SEMANTIC_PG_EDGE_ROWS_PER_NODE: usize = 5;
pub(crate) const SEMA_STRIDE_U32: usize = 6;
pub(crate) const TYPEDEF_FLAGS_FIELD: usize = 7;
pub(crate) const VISIBLE_TYPEDEF_FLAG: u32 = 1;
pub(crate) const TYPEDEF_DECLARATOR_FLAG: u32 = 1 << 1;
pub(crate) const ORDINARY_DECL_FLAG: u32 = 1 << 2;

pub(crate) struct LexRows {
    pub(crate) tok_types: Vec<u32>,
    pub(crate) starts: Vec<u32>,
    pub(crate) lens: Vec<u32>,
}

pub(crate) struct CompiledObject {
    bytes: Vec<u8>,
}

impl CompiledObject {
    pub(crate) fn assert_elf(&self) {
        assert_eq!(&self.bytes[0..4], b"\x7fELF");
    }

    pub(crate) fn payload(&self) -> &[u8] {
        self.bytes
            .windows(MAGIC.len())
            .position(|window| window == MAGIC)
            .map(|offset| &self.bytes[offset..])
            .expect("compiled object embeds a VYRECOB2 payload")
    }

    pub(crate) fn version(&self) -> u32 {
        let mut offset = MAGIC.len();
        read_u32(self.payload(), &mut offset)
    }

    pub(crate) fn section(&self, wanted: u32) -> &[u8] {
        parse_vyrecob2_section(self.payload(), wanted)
            .unwrap_or_else(|| panic!("VYRECOB2 section {wanted} is present"))
    }

    pub(crate) fn sections(&self) -> Vec<(u32, &[u8])> {
        parse_vyrecob2_sections(self.payload())
    }

    pub(crate) fn lex(&self) -> LexRows {
        let (tok_types, starts, lens) = parse_lex_section(self.section(SECTION_LEX));
        LexRows {
            tok_types,
            starts,
            lens,
        }
    }

    pub(crate) fn words(&self, section: u32) -> Vec<u32> {
        u32_words_from_bytes(self.section(section))
    }

    pub(crate) fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

pub(crate) fn compile_source(
    name: &str,
    source: &str,
    macros: Vec<(String, Option<String>)>,
) -> CompiledObject {
    compile_source_with_resident(name, source, macros, Vec::new()).0
}

/// Same as [`compile_source`], but also returns the **resident** translation unit text the GPU
/// lexer sees after `-D` prefixing and bounded `#include` inlining (macros are not CPU-expanded).
pub(crate) fn compile_source_with_resident(
    name: &str,
    source: &str,
    macros: Vec<(String, Option<String>)>,
    include_dirs: Vec<PathBuf>,
) -> (CompiledObject, String) {
    let src = unique_path(name, "c");
    let out = unique_path(name, "o");
    fs::write(&src, source).expect("write C source fixture");

    let mut opts = VyreCompileOptions::default();
    opts.is_compile_only = true;
    opts.input_files = vec![src.clone()];
    opts.output_file = Some(out.clone());
    opts.include_dirs = include_dirs;
    opts.disable_system_include_dirs = true;
    opts.macros = macros;
    let raw = fs::read_to_string(&src).expect("read source back");
    let resident = prepare_resident_translation_unit_source(&src, &raw, &opts)
        .expect("prepare resident translation unit");

    let result = compile(opts);
    let _ = fs::remove_file(&src);
    result.unwrap_or_else(|error| panic!("GPU C11 compile succeeds: {error}"));
    let object = fs::read(&out).expect("read emitted object");
    let _ = fs::remove_file(&out);
    (CompiledObject { bytes: object }, resident)
}

pub(crate) fn read_u32(buf: &[u8], offset: &mut usize) -> u32 {
    let end = offset.saturating_add(4);
    let bytes: [u8; 4] = buf[*offset..end]
        .try_into()
        .expect("VYRECOB2 u32 field is present");
    *offset = end;
    u32::from_le_bytes(bytes)
}

pub(crate) fn parse_vyrecob2_section(payload: &[u8], wanted: u32) -> Option<&[u8]> {
    let mut offset = MAGIC.len() + 4;
    let section_count = read_u32(payload, &mut offset);
    for _ in 0..section_count {
        let tag = read_u32(payload, &mut offset);
        let len = read_u32(payload, &mut offset) as usize;
        let end = offset.saturating_add(len);
        assert!(
            end <= payload.len(),
            "section length stays inside VYRECOB2 payload"
        );
        if tag == wanted {
            return Some(&payload[offset..end]);
        }
        offset = end;
    }
    None
}

pub(crate) fn parse_vyrecob2_sections(payload: &[u8]) -> Vec<(u32, &[u8])> {
    let mut offset = MAGIC.len();
    let version = read_u32(payload, &mut offset);
    assert_eq!(version, 7);
    let section_count = read_u32(payload, &mut offset);
    let mut sections = Vec::with_capacity(section_count as usize);
    for _ in 0..section_count {
        let tag = read_u32(payload, &mut offset);
        let len = read_u32(payload, &mut offset) as usize;
        let end = offset.saturating_add(len);
        assert!(
            end <= payload.len(),
            "section length stays inside VYRECOB2 payload"
        );
        sections.push((tag, &payload[offset..end]));
        offset = end;
    }
    sections
}

pub(crate) fn parse_lex_section(section: &[u8]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    assert_eq!(&section[0..8], b"VYRECOB1");
    let mut offset = 8;
    assert_eq!(read_u32(section, &mut offset), 1);
    let path_len = read_u32(section, &mut offset) as usize;
    offset = offset.saturating_add(path_len);
    while offset % 8 != 0 {
        offset = offset.saturating_add(1);
    }

    let n_tokens = read_u32(section, &mut offset) as usize;
    let mut tok_types = Vec::with_capacity(n_tokens);
    let mut starts = Vec::with_capacity(n_tokens);
    let mut lens = Vec::with_capacity(n_tokens);
    for _ in 0..n_tokens {
        tok_types.push(read_u32(section, &mut offset));
        starts.push(read_u32(section, &mut offset));
        lens.push(read_u32(section, &mut offset));
    }
    (tok_types, starts, lens)
}

pub(crate) fn u32_words_from_bytes(bytes: &[u8]) -> Vec<u32> {
    assert_eq!(bytes.len() % 4, 0, "u32 section has full words");
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("valid u32 chunk")))
        .collect()
}

pub(crate) fn u32_words_to_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn unique_path(name: &str, ext: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "vyre_frontend_c_{name}_{}_{nanos}.{ext}",
        std::process::id()
    ))
}
