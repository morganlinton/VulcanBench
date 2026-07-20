use std::cell::RefCell;
use std::mem;

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};

use vyre_libs::compiler::object_writer::opt_lower_elf;
use vyre_libs::parsing::c::lex::tokens::{TOK_LBRACE, TOK_LPAREN, TOK_RBRACE, TOK_RPAREN};

#[cfg(test)]
use super::buffers::read_u32_stream;
const MATCH_NONE: u32 = u32::MAX;
const ELF_OUT_BYTES: usize = 4096 * 4;
const BRACKET_MATCH_WORKGROUP: u32 = 256;

#[derive(Default)]
struct DispatchScratch {
    tok_bytes: Vec<u8>,
    outputs: Vec<Vec<u8>>,
    paren_pairs: Vec<u32>,
    brace_pairs: Vec<u32>,
}

thread_local! {
    static DISPATCH_SCRATCH: RefCell<DispatchScratch> =
        RefCell::new(DispatchScratch::default());
}

fn elf_out_zeroes() -> &'static [u8] {
    static ZEROES: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    ZEROES.get_or_init(|| vec![0u8; ELF_OUT_BYTES]).as_slice()
}

pub(super) fn dispatch_c11_bracket_pairs(
    backend: &dyn VyreBackend,
    tok_types: &[u32],
    label: &str,
) -> Result<(Vec<u32>, Vec<u32>), String> {
    DISPATCH_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "C11 bracket-pair dispatch scratch was re-entered on the same thread. Fix: call bracket matching from a non-nested parser context or add explicit caller-owned scratch.".to_string()
        })?;
        dispatch_c11_bracket_pairs_with_scratch(backend, tok_types, label, &mut scratch)
    })
}

fn dispatch_c11_bracket_pairs_with_scratch(
    backend: &dyn VyreBackend,
    tok_types: &[u32],
    label: &str,
    scratch: &mut DispatchScratch,
) -> Result<(Vec<u32>, Vec<u32>), String> {
    let n_u32 = u32::try_from(tok_types.len()).map_err(|_| {
        format!(
            "c11_dual_bracket_match token count {} exceeds the u32 GPU index space. Fix: shard the translation unit before bracket-pair dispatch.",
            tok_types.len()
        )
    })?.max(1);
    pack_u32_le_bytes_min_words_into(tok_types, n_u32, &mut scratch.tok_bytes)?;
    let mut cfg = DispatchConfig::default();
    cfg.label = Some(label.to_string());
    cfg.grid_override = Some([n_u32.div_ceil(BRACKET_MATCH_WORKGROUP).max(1), 1, 1]);
    let key = super::stage_pipeline_cache_key(
        "c11_dual_bracket_match",
        &[n_u32 as u64, BRACKET_MATCH_WORKGROUP as u64],
    );
    let inputs = [scratch.tok_bytes.as_slice()];
    super::dispatch_borrowed_stage_cached_into(
        backend,
        key,
        || {
            let prog = c11_dual_bracket_match("tok_types", "paren_pairs", "brace_pairs", n_u32);
            super::validate_internal_stage(&prog, "c11_dual_bracket_match")?;
            Ok(prog)
        },
        &inputs,
        &cfg,
        &mut scratch.outputs,
    )
    .map_err(|e| e.to_string())?;
    if scratch.outputs.len() != 2 {
        return Err(format!(
            "c11_dual_bracket_match returned {} output buffer(s), expected exactly 2. Fix: backend must return paren_pairs and brace_pairs.",
            scratch.outputs.len()
        ));
    }
    read_u32_stream_into(
        &scratch.outputs[0],
        tok_types.len(),
        "c11 paren pairs",
        &mut scratch.paren_pairs,
    )?;
    read_u32_stream_into(
        &scratch.outputs[1],
        tok_types.len(),
        "c11 brace pairs",
        &mut scratch.brace_pairs,
    )?;
    let mut paren_pairs = Vec::new();
    let mut brace_pairs = Vec::new();
    mem::swap(&mut paren_pairs, &mut scratch.paren_pairs);
    mem::swap(&mut brace_pairs, &mut scratch.brace_pairs);
    Ok((paren_pairs, brace_pairs))
}

fn c11_dual_bracket_match(
    tok_types: &str,
    paren_pairs: &str,
    brace_pairs: &str,
    n: u32,
) -> Program {
    let i = Expr::InvocationId { axis: 0 };
    let body = vec![
        Node::store(paren_pairs, i.clone(), Expr::u32(MATCH_NONE)),
        Node::store(brace_pairs, i.clone(), Expr::u32(MATCH_NONE)),
        Node::let_bind("tok", Expr::load(tok_types, i.clone())),
        forward_match_open(
            tok_types,
            paren_pairs,
            i.clone(),
            n,
            TOK_LPAREN,
            TOK_RPAREN,
            "paren",
        ),
        backward_match_close(
            tok_types,
            paren_pairs,
            i.clone(),
            TOK_LPAREN,
            TOK_RPAREN,
            "paren",
        ),
        forward_match_open(
            tok_types,
            brace_pairs,
            i.clone(),
            n,
            TOK_LBRACE,
            TOK_RBRACE,
            "brace",
        ),
        backward_match_close(
            tok_types,
            brace_pairs,
            i.clone(),
            TOK_LBRACE,
            TOK_RBRACE,
            "brace",
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl {
                is_output: true,
                ..BufferDecl::storage(paren_pairs, 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(n)
                    .with_pipeline_live_out(true)
            },
            BufferDecl::storage(brace_pairs, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n)
                .with_pipeline_live_out(true),
        ],
        [BRACKET_MATCH_WORKGROUP, 1, 1],
        vec![Node::if_then(Expr::lt(i, Expr::u32(n)), body)],
    )
}

fn forward_match_open(
    tok_types: &str,
    pairs: &str,
    i: Expr,
    n: u32,
    open_tok: u32,
    close_tok: u32,
    label: &str,
) -> Node {
    let depth = format!("{label}_forward_depth");
    let found = format!("{label}_forward_found");
    let j = format!("{label}_forward_j");
    let tok = format!("{label}_forward_tok");
    Node::if_then(
        Expr::eq(Expr::var("tok"), Expr::u32(open_tok)),
        vec![
            Node::let_bind(&depth, Expr::u32(1)),
            Node::let_bind(&found, Expr::u32(0)),
            Node::loop_for(
                &j,
                Expr::add(i.clone(), Expr::u32(1)),
                Expr::u32(n),
                vec![Node::if_then(
                    Expr::eq(Expr::var(&found), Expr::u32(0)),
                    vec![
                        Node::let_bind(&tok, Expr::load(tok_types, Expr::var(&j))),
                        Node::if_then(
                            Expr::eq(Expr::var(&tok), Expr::u32(open_tok)),
                            vec![Node::assign(
                                &depth,
                                Expr::add(Expr::var(&depth), Expr::u32(1)),
                            )],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var(&tok), Expr::u32(close_tok)),
                            vec![
                                Node::assign(&depth, Expr::sub(Expr::var(&depth), Expr::u32(1))),
                                Node::if_then(
                                    Expr::eq(Expr::var(&depth), Expr::u32(0)),
                                    vec![
                                        Node::store(pairs, i.clone(), Expr::var(&j)),
                                        Node::assign(&found, Expr::u32(1)),
                                    ],
                                ),
                            ],
                        ),
                    ],
                )],
            ),
        ],
    )
}

fn backward_match_close(
    tok_types: &str,
    pairs: &str,
    i: Expr,
    open_tok: u32,
    close_tok: u32,
    label: &str,
) -> Node {
    let depth = format!("{label}_backward_depth");
    let found = format!("{label}_backward_found");
    let k = format!("{label}_backward_k");
    let j = format!("{label}_backward_j");
    let tok = format!("{label}_backward_tok");
    Node::if_then(
        Expr::eq(Expr::var("tok"), Expr::u32(close_tok)),
        vec![
            Node::let_bind(&depth, Expr::u32(1)),
            Node::let_bind(&found, Expr::u32(0)),
            Node::loop_for(
                &k,
                Expr::u32(0),
                i.clone(),
                vec![Node::if_then(
                    Expr::eq(Expr::var(&found), Expr::u32(0)),
                    vec![
                        Node::let_bind(
                            &j,
                            Expr::sub(Expr::sub(i.clone(), Expr::u32(1)), Expr::var(&k)),
                        ),
                        Node::let_bind(&tok, Expr::load(tok_types, Expr::var(&j))),
                        Node::if_then(
                            Expr::eq(Expr::var(&tok), Expr::u32(close_tok)),
                            vec![Node::assign(
                                &depth,
                                Expr::add(Expr::var(&depth), Expr::u32(1)),
                            )],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var(&tok), Expr::u32(open_tok)),
                            vec![
                                Node::assign(&depth, Expr::sub(Expr::var(&depth), Expr::u32(1))),
                                Node::if_then(
                                    Expr::eq(Expr::var(&depth), Expr::u32(0)),
                                    vec![
                                        Node::store(pairs, i.clone(), Expr::var(&j)),
                                        Node::assign(&found, Expr::u32(1)),
                                    ],
                                ),
                            ],
                        ),
                    ],
                )],
            ),
        ],
    )
}

pub(super) fn try_dispatch_elf(
    backend: &dyn VyreBackend,
    compiler_bytes: &[u8],
    node_count: u32,
) -> Result<Vec<u8>, String> {
    DISPATCH_SCRATCH.with(|scratch| {
        let mut scratch = scratch.try_borrow_mut().map_err(|_| {
            "ELF lowering dispatch scratch was re-entered on the same thread. Fix: call ELF lowering from a non-nested compile path or add explicit caller-owned scratch.".to_string()
        })?;
        try_dispatch_elf_with_scratch(backend, compiler_bytes, node_count, &mut scratch)
    })
}

fn try_dispatch_elf_with_scratch(
    backend: &dyn VyreBackend,
    compiler_bytes: &[u8],
    node_count: u32,
    scratch: &mut DispatchScratch,
) -> Result<Vec<u8>, String> {
    let node_count = node_count.max(1);
    let mut cfg = DispatchConfig::default();
    cfg.label = Some("vyre-frontend-c opt_lower_elf".to_string());
    let key = super::stage_pipeline_cache_key("opt_lower_elf", &[node_count as u64]);
    let inputs = [compiler_bytes, elf_out_zeroes()];
    super::dispatch_borrowed_stage_cached_into(
        backend,
        key,
        || {
            let prog = opt_lower_elf("ssa_nodes", "elf_out", Expr::u32(node_count));
            super::validate_internal_stage(&prog, "opt_lower_elf")?;
            Ok(prog)
        },
        &inputs,
        &cfg,
        &mut scratch.outputs,
    )
    .map_err(|e| e.to_string())?;
    if scratch.outputs.is_empty() {
        return Err("ELF lowering: missing output buffer".to_string());
    }
    let mut elf = Vec::new();
    mem::swap(&mut elf, &mut scratch.outputs[0]);
    Ok(elf)
}

fn pack_u32_le_bytes_min_words_into(
    words: &[u32],
    min_words: u32,
    out: &mut Vec<u8>,
) -> Result<(), String> {
    vyre_primitives::wire::pack_u32_slice_min_words_into(words, min_words, out)
}

fn read_u32_stream_into(
    buf: &[u8],
    words: usize,
    label: &str,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    vyre_primitives::wire::unpack_u32_slice_into(buf, words, label, out)
}

#[cfg(test)]
#[path = "dispatch_gpu_bracket_tests.rs"]
mod gpu_bracket_tests;
