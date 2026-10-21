use super::*;

pub(super) fn emit_object_carrier(
    backend: &dyn VyreBackend,
    path: &Path,
    dest: &Path,
    decoded: &token_decode::DecodedObjectTokens,
    structure: structure_stage::ObjectStructure,
    preproc_mask: Vec<u8>,
    abi_blob: Vec<u8>,
    ast_blob: Vec<u8>,
    semantic: semantic_stage::ObjectSemantics,
    cfg_blob: Vec<u8>,
    trace: &mut trace::CompileTrace,
) -> Result<(), String> {
    let (compiler_bytes, compiler_word_count) = compiler_bytes_from_sections(
        &[
            semantic.vast_blob.as_slice(),
            semantic.pg_blob.as_slice(),
            semantic.semantic_pg_nodes.as_slice(),
            semantic.semantic_pg_edges.as_slice(),
        ],
        ELF_LOWERING_MAX_INPUT_WORDS,
    )?;
    let elf_blob = try_dispatch_elf(backend, &compiler_bytes, compiler_word_count)?;
    trace.log("try_dispatch_elf");
    validate_gpu_elf_blob(&elf_blob)?;
    trace.log("validate_gpu_elf_blob");

    let lex_section = crate::object_format::build_vyrecob1_lex_section(
        path,
        &decoded.types_logical,
        &decoded.starts_logical,
        &decoded.lens_logical,
        decoded.n_tokens,
    )?;
    let cfg_word_count = u32::try_from(cfg_blob.len() / 4)
        .map_err(|_| "CFG section exceeds u32 count".to_string())?;
    const SECTION_ORDER: [SectionTag; 18] = [
        SectionTag::Lex,
        SectionTag::ParenPairs,
        SectionTag::BracePairs,
        SectionTag::Functions,
        SectionTag::Calls,
        SectionTag::Elf,
        SectionTag::PreprocMask,
        SectionTag::MacroTypes,
        SectionTag::AbiLayout,
        SectionTag::Ast,
        SectionTag::Cfg,
        SectionTag::Megakernel,
        SectionTag::Vast,
        SectionTag::ProgramGraph,
        SectionTag::SemaScope,
        SectionTag::ExpressionShape,
        SectionTag::SemanticProgramGraphNodes,
        SectionTag::SemanticProgramGraphEdges,
    ];
    let section_tags = SECTION_ORDER.map(|tag| tag as u32);
    let mega_bytes = megakernel_section_bytes(
        decoded.n_tokens,
        structure.n_fn,
        cfg_word_count,
        &section_tags,
    )?;
    let sections = [
        (SectionTag::Lex, lex_section.as_slice()),
        (SectionTag::ParenPairs, structure.paren_bytes.as_slice()),
        (SectionTag::BracePairs, structure.brace_bytes.as_slice()),
        (SectionTag::Functions, structure.fn_records.as_slice()),
        (SectionTag::Calls, structure.call_records.as_slice()),
        (SectionTag::Elf, elf_blob.as_slice()),
        (SectionTag::PreprocMask, preproc_mask.as_slice()),
        (SectionTag::MacroTypes, decoded.types_logical.as_slice()),
        (SectionTag::AbiLayout, abi_blob.as_slice()),
        (SectionTag::Ast, ast_blob.as_slice()),
        (SectionTag::Cfg, cfg_blob.as_slice()),
        (SectionTag::Megakernel, mega_bytes.as_slice()),
        (SectionTag::Vast, semantic.vast_blob.as_slice()),
        (SectionTag::ProgramGraph, semantic.pg_blob.as_slice()),
        (SectionTag::SemaScope, semantic.sema_blob.as_slice()),
        (
            SectionTag::ExpressionShape,
            semantic.expr_shape_blob.as_slice(),
        ),
        (
            SectionTag::SemanticProgramGraphNodes,
            semantic.semantic_pg_nodes.as_slice(),
        ),
        (
            SectionTag::SemanticProgramGraphEdges,
            semantic.semantic_pg_edges.as_slice(),
        ),
    ];
    for (index, ((actual, _), expected)) in sections.iter().zip(SECTION_ORDER).enumerate() {
        if *actual as u32 != expected as u32 {
            return Err(format!(
                "VYRECOB2 section table order drifted at index {index}: expected tag {}, got {}. Fix: update SECTION_ORDER and sections together.",
                expected as u32,
                *actual as u32
            ));
        }
    }
    let vyrecob2 = crate::object_format::serialize_vyrecob2(&sections)
        .map_err(|error| format!("VYRECOB2 serialization failed: {error}"))?;
    trace.log("serialize_vyrecob2");
    let elf_obj = crate::elf_linux::emit_translation_unit_relocatable(&vyrecob2, path)?;
    trace.log("emit_translation_unit_relocatable");
    write_object_atomic(dest, &elf_obj)?;
    trace.log("write_object_atomic");
    Ok(())
}

fn validate_gpu_elf_blob(elf_blob: &[u8]) -> Result<(), String> {
    if elf_blob.len() < 4 {
        return Err(format!(
            "GPU ELF lowering returned {} bytes, shorter than the ELF magic. Fix: repair opt_lower_elf output sizing.",
            elf_blob.len()
        ));
    }
    if &elf_blob[..4] != b"\x7fELF" {
        return Err(format!(
            "GPU ELF lowering returned invalid ELF magic {:02x?}. Fix: repair opt_lower_elf header emission before embedding the section.",
            &elf_blob[..4]
        ));
    }
    if elf_blob.len() % 4 != 0 {
        return Err(format!(
            "GPU ELF lowering returned {} bytes, not u32-aligned. Fix: keep opt_lower_elf output buffers word-aligned.",
            elf_blob.len()
        ));
    }
    Ok(())
}
