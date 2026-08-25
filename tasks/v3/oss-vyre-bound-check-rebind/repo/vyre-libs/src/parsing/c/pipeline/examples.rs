use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// The Capstone: Fully GPU-Native C11 Pipeline
///
/// Implements the "Lego Block" principle by statically linking the entire Semantic
/// Parsing → Middle-End Optimization → Lowering pipeline as a single hardware
/// compilation job with zero CPU context switching.
///
/// **Status: registered-stage composition sketch.** The body below records the
/// dispatch order using op IDs that exist in this crate. The individual stages
/// have their own `inventory::submit!` entries and byte-identity fixtures; this
/// outer composition remains unregistered until the IR can pass buffer
/// references through `Expr::Call` without stringly buffer variables.
///
/// This composition itself is **not** registered with the harness because the
/// buffer-reference passing in `Expr::Call` args here is a compile-time
/// sketch that the reference interpreter does not execute  -  the real
/// executable form is the fused megakernel produced by downstream analyzer against the
/// individual sub-op registrations. Register this wrapper only after `Expr::Call`
/// can carry buffer references or the inline-expansion pass can splice the
/// callee bodies directly.
#[must_use]
pub fn build_c11_compiler_megakernel(
    source_characters: &str, // Input raw C11/GNU-C
    target_bytecode: &str,   // Output executable Vyre IR
    max_tokens: u32,
    max_statements: u32,
) -> Program {
    let sequence_control = Expr::var("global_sequence_step");

    // The pipeline sketch uses a sequential dispatcher structure over identical thread groups.
    // Instead of spawning host commands perfectly synchronizing barriers,
    // we use a single unified VRAM job that executes all pipeline elements locally!

    let loop_body = vec![
        // NOTE: In the true hardware execution backend, we would compose these using
        // `crate::region::Node::Region` linking their local registries.
        // For demonstration of the chained parser invariants:
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(0)),
            vec![
                // 0. Lexer (DFA state machine and source-ordered token stream)
                Node::call(
                    "vyre-libs::parsing::c_lexer",
                    vec![
                        Expr::var(source_characters),
                        Expr::var("tmp_tok_types"),
                        Expr::var("tmp_tok_starts"),
                        Expr::var("tmp_tok_lens"),
                        Expr::var("tmp_counts"),
                    ],
                ),
                // 1. Phase-3 digraph normalization runs after tokenization.
                Node::call(
                    "vyre-libs::parsing::c11_lex_digraphs",
                    vec![
                        Expr::var("tmp_tok_types"),
                        Expr::var("tmp_tok_starts"),
                        Expr::var("tmp_tok_lens"),
                        Expr::buf_len("tmp_tok_types"),
                    ],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(1)),
            vec![
                // 2. Preprocessor macro expansion over the source-manager prepared token stream.
                Node::call(
                    "vyre-libs::parsing::opt_named_macro_expansion_materialized",
                    vec![
                        Expr::var("tmp_tok_types"),
                        Expr::var("tmp_tok_starts"),
                        Expr::var("tmp_tok_lens"),
                        Expr::var(source_characters),
                        Expr::var("macro_name_hashes"),
                        Expr::var("macro_name_starts"),
                        Expr::var("macro_name_lens"),
                        Expr::var("macro_name_words"),
                        Expr::var("macro_vals"),
                        Expr::var("macro_sizes"),
                        Expr::var("macro_kinds"),
                        Expr::var("macro_param_counts"),
                        Expr::var("macro_replacement_params"),
                        Expr::var("macro_replacement_starts"),
                        Expr::var("macro_replacement_lens"),
                        Expr::var("macro_replacement_words"),
                        Expr::var("tmp_tok_types_exp"),
                        Expr::var("tmp_tok_starts_exp"),
                        Expr::var("tmp_tok_lens_exp"),
                        Expr::var("tmp_source_words_exp"),
                        Expr::var("tmp_counts"),
                        Expr::var("tmp_source_counts"),
                        Expr::var("tmp_token_count"),
                        Expr::var("source_len"),
                        Expr::var("macro_replacement_source_len"),
                        Expr::u32(max_tokens),
                        Expr::var("max_expanded_source_len"),
                    ],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(2)),
            vec![
                // 3. Keyword & Types Re-Classification
                Node::call(
                    "vyre-libs::parsing::c_keyword",
                    vec![Expr::var("tmp_tok_types_exp")],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(3)),
            vec![
                // 4. Alignment System V ABI Mapping & Struct Sizing
                Node::call(
                    "vyre-libs::parsing::c11_compute_alignments",
                    vec![
                        Expr::var("tmp_tok_types_exp"),
                        Expr::var("tmp_align_sizes"),
                        Expr::var("tmp_align_pads"),
                        Expr::u32(100),
                    ],
                ),
                // 5. Structure, Scopes, Symbol Resolution
                Node::call(
                    "vyre-libs::parsing::c_sema_scope",
                    vec![
                        Expr::var("tmp_tok_types_exp"),
                        Expr::var("tmp_scope_ids"),
                        Expr::var("tmp_scope_parents"),
                    ],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(4)),
            vec![
                // 6. Mathematical Flat Shunting Yard (Zero Stack Divergence)
                Node::call(
                    "vyre-libs::parsing::ast_shunting_yard",
                    vec![
                        Expr::var("tmp_tok_types_exp"),
                        Expr::var("tmp_ast_opcodes"),
                        Expr::var("tmp_ast_children"),
                    ],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(5)),
            vec![
                // 7.1 GNU-C extension block: builtins
                Node::call(
                    "vyre-libs::parsing::c11_gnu_builtins_pass",
                    vec![Expr::var("tmp_ast_opcodes"), Expr::var("tmp_ast_opcodes")],
                ),
                // 7.2 GNU-C extension block: capture asm volatile() blocks
                Node::call(
                    "vyre-libs::parsing::c11_gnu_inline_asm_pass",
                    vec![Expr::var("tmp_ast_opcodes"), Expr::var("tmp_asm_blocks")],
                ),
                // 7.3 Data & BSS Static Instantiations
                Node::call(
                    "vyre-libs::parsing::c11_build_vast_nodes",
                    vec![
                        Expr::var("tmp_ast_opcodes"),
                        Expr::var("tmp_data_segs"),
                        Expr::var("tmp_bss_segs"),
                    ],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(6)),
            vec![
                // 8. SSA Transformation & Dominance Fronts
                Node::call(
                    "vyre-libs::parsing::c::lower::ast_to_pg_nodes",
                    vec![Expr::var("tmp_ast_opcodes"), Expr::var("tmp_ssa_nodes")],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(7)),
            vec![
                // 9. Control Flow Graph & Global Goto Resolver
                Node::call(
                    "vyre-libs::parsing::c11_build_cfg_and_gotos",
                    vec![
                        Expr::var("tmp_ssa_nodes"),
                        Expr::var("tmp_cfg_blocks"),
                        Expr::var("tmp_goto_labels"),
                        Expr::u32(100),
                    ],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(8)),
            vec![
                // 10. Common Structural Elimination & Folding
                Node::call(
                    "vyre-libs::parsing::c11_build_expression_shape_nodes",
                    vec![Expr::var("tmp_cfg_blocks"), Expr::var("tmp_opt_nodes")],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(9)),
            vec![
                // 11. X86_64 Register Allocation Phase
                Node::call(
                    "vyre-libs::parsing::opt_x86_64_register_allocation",
                    vec![
                        Expr::var("tmp_opt_nodes"),
                        Expr::var("tmp_physical_regs"),
                        Expr::u32(100),
                    ],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(10)),
            vec![
                // 12. GPU Stack Layout Spilling (Prologue/Epilogue)
                Node::call(
                    "vyre-libs::parsing::opt_stack_layout_generation",
                    vec![
                        Expr::var("tmp_physical_regs"),
                        Expr::var("tmp_spill_offsets"),
                        Expr::u32(100),
                    ],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(11)),
            vec![
                // 13. Final Boss ABI Lowering: Full ELF Target Emission Object Generation
                Node::call(
                    "vyre-libs::parsing::opt_lower_elf",
                    vec![
                        Expr::var("tmp_spill_offsets"),
                        Expr::var(target_bytecode),
                        Expr::u32(100),
                    ],
                ),
                // Emit Diagnostics concurrently
                Node::call(
                    "vyre-libs::parsing::c11_classify_vast_node_kinds",
                    vec![
                        Expr::var("tmp_tok_starts_exp"),
                        Expr::var("tmp_error_flags"),
                        Expr::var("tmp_out_errors"),
                    ],
                ),
            ],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(sequence_control.clone(), Expr::u32(12)),
            vec![
                // 14. Object merger: stitch output relocatables into the final image.
                Node::call(
                    "vyre-libs::parsing::opt_lower_elf",
                    vec![
                        Expr::var("tmp_elf_array"),
                        Expr::var("tmp_global_symtab"),
                        Expr::var("linked_image_out"),
                        Expr::u32(10),
                    ],
                ),
            ],
        ),
        Node::barrier(),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(source_characters, 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(target_bytecode, 1, BufferAccess::ReadWrite, DataType::U32),
            // Scratchpad buffers shifted to Shared Memory (O(1) Warp-Arena)
            BufferDecl::workgroup("tmp_tok_types", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_tok_starts", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_tok_lens", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_counts", 1, DataType::U32),
            BufferDecl::workgroup("tmp_tok_types_exp", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_tok_starts_exp", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_tok_lens_exp", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_align_sizes", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_align_pads", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_scope_ids", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_scope_parents", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_ast_opcodes", max_statements, DataType::U32),
            BufferDecl::workgroup("tmp_ast_children", max_statements, DataType::U32),
            BufferDecl::workgroup("tmp_data_segs", max_statements, DataType::U32),
            BufferDecl::workgroup("tmp_bss_segs", max_statements, DataType::U32),
            BufferDecl::workgroup("tmp_ssa_nodes", max_statements, DataType::U32),
            BufferDecl::workgroup("tmp_opt_nodes", max_statements, DataType::U32),
            BufferDecl::workgroup("tmp_cfg_blocks", max_statements, DataType::U32),
            BufferDecl::workgroup("tmp_goto_labels", max_statements, DataType::U32),
            BufferDecl::workgroup("tmp_physical_regs", max_statements, DataType::U32),
            BufferDecl::workgroup("tmp_spill_offsets", max_statements, DataType::U32),
            BufferDecl::workgroup("tmp_error_flags", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_out_errors", max_tokens, DataType::U32),
            BufferDecl::workgroup("tmp_asm_blocks", max_statements, DataType::U32),
            BufferDecl::workgroup("out_asm_counts", 1, DataType::U32),
            BufferDecl::workgroup("tmp_elf_array", max_statements, DataType::U32),
            BufferDecl::workgroup("tmp_global_symtab", max_statements, DataType::U32),
            BufferDecl::workgroup("linked_image_out", max_statements, DataType::U32),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::c11_megakernel",
            vec![Node::loop_for(
                "global_sequence_step",
                Expr::u32(0),
                Expr::u32(13),
                loop_body,
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::c11_megakernel")
    .with_non_composable_with_self(true)
}

// NOTE: `build_c11_compiler_megakernel` is intentionally NOT registered with
// the `inventory` harness. See the function's docstring for why  -  the sub-ops
// it calls are the byte-identity verified units; this outer fn is the
// written-down pipeline spec. Once the IR gains a `BufferRef` expression (or
// this fn is rewritten against the inline-expansion pass), re-register here.
