use crate::parsing::c::sema::{
    intern::emit_identifier_intern,
    lookup::emit_declaration_lookup,
    walk::{emit_brace_scope_resolution, emit_function_parameter_scope},
};
use crate::parsing::c::source_bytes::source_haystack_words;
use crate::parsing::composition::child_phase;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

#[cfg(any(test, feature = "cpu-parity"))]
mod reference;
#[cfg(any(test, feature = "cpu-parity"))]
mod witness;

#[allow(deprecated)]
#[cfg(any(test, feature = "cpu-parity"))]
pub use reference::reference_scope_tree;

const OP_ID: &str = "vyre-libs::parsing::c_sema_scope";
const SCOPE_PHASE_OP_ID: &str = "vyre-libs::parsing::c_sema_scope.scope";
const SCOPE_BRACE_PHASE_OP_ID: &str = "vyre-libs::parsing::c_sema_scope.scope.brace";
const SCOPE_FUNCTION_PARAMS_PHASE_OP_ID: &str =
    "vyre-libs::parsing::c_sema_scope.scope.function_parameters";
const DECL_PHASE_OP_ID: &str = "vyre-libs::parsing::c_sema_scope.decl";
const IDENTIFIER_INTERN_PHASE_OP_ID: &str = "vyre-libs::parsing::c_sema_scope.identifier_intern";

#[derive(Clone, Copy, PartialEq, Eq)]
enum CScopePhase {
    Scope,
    ScopeBrace,
    ScopeFunctionParameters,
    Decl,
    IdentifierIntern,
}

/// Map token index `i` to:
///
/// 1. `scope_id`
/// 2. `scope_parent_id`
/// 3. `decl_kind`
/// 4. `identifier_intern_id`
///
/// The output is one 4-word record per token.
#[must_use]
pub fn c_sema_scope(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    haystack: &str,
    haystack_len: Expr,
    num_tokens: Expr,
    out_scope_tree: &str,
) -> Program {
    c_sema_scope_impl(
        tok_types,
        tok_starts,
        tok_lens,
        haystack,
        haystack_len,
        num_tokens,
        out_scope_tree,
        false,
        false,
    )
}

#[must_use]
/// Build semantic scope/interning IR for packed-byte source haystacks.
///
/// Token offsets remain logical byte offsets; `haystack` stores four source
/// bytes per `u32` word for CUDA-first parser pipelines.
pub fn c_sema_scope_packed_haystack(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    haystack: &str,
    haystack_len: Expr,
    num_tokens: Expr,
    out_scope_tree: &str,
) -> Program {
    c_sema_scope_impl(
        tok_types,
        tok_starts,
        tok_lens,
        haystack,
        haystack_len,
        num_tokens,
        out_scope_tree,
        true,
        false,
    )
}

#[must_use]
/// Build semantic scope records for symbol-bearing tokens on packed CUDA haystacks.
///
/// Non-identifier rows are emitted with zero scope/declaration/intern fields so
/// production object consumers can keep stable row indexing without paying
/// brace/parameter scope walks for punctuation and keywords.
pub fn c_sema_scope_symbols_packed_haystack(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    haystack: &str,
    haystack_len: Expr,
    num_tokens: Expr,
    out_scope_tree: &str,
) -> Program {
    c_sema_scope_impl(
        tok_types,
        tok_starts,
        tok_lens,
        haystack,
        haystack_len,
        num_tokens,
        out_scope_tree,
        true,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn c_sema_scope_impl(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    haystack: &str,
    haystack_len: Expr,
    num_tokens: Expr,
    out_scope_tree: &str,
    packed_haystack: bool,
    symbols_only: bool,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let tok_count = match &num_tokens {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    let haystack_words = match &haystack_len {
        Expr::LitU32(n) => source_haystack_words(*n, packed_haystack),
        _ => 1,
    };

    let guarded_body = vec![
        child_phase(
            OP_ID,
            SCOPE_PHASE_OP_ID,
            c_sema_scope_phase_body(
                CScopePhase::Scope,
                tok_types,
                tok_starts,
                tok_lens,
                haystack,
                t.clone(),
                &num_tokens,
                out_scope_tree,
                packed_haystack,
                symbols_only,
            ),
        ),
        child_phase(
            OP_ID,
            DECL_PHASE_OP_ID,
            c_sema_scope_phase_body(
                CScopePhase::Decl,
                tok_types,
                tok_starts,
                tok_lens,
                haystack,
                t.clone(),
                &num_tokens,
                out_scope_tree,
                packed_haystack,
                symbols_only,
            ),
        ),
        child_phase(
            OP_ID,
            IDENTIFIER_INTERN_PHASE_OP_ID,
            c_sema_scope_phase_body(
                CScopePhase::IdentifierIntern,
                tok_types,
                tok_starts,
                tok_lens,
                haystack,
                t.clone(),
                &num_tokens,
                out_scope_tree,
                packed_haystack,
                symbols_only,
            ),
        ),
    ];

    let out_words = tok_count.saturating_mul(4).max(1);

    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_count.max(1)),
            BufferDecl::storage(tok_starts, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_count.max(1)),
            BufferDecl::storage(tok_lens, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_count.max(1)),
            BufferDecl::storage(haystack, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(haystack_words.max(1)),
            BufferDecl::output(out_scope_tree, 4, DataType::U32).with_count(out_words),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            OP_ID,
            vec![Node::if_then(
                Expr::lt(t.clone(), num_tokens.clone()),
                guarded_body,
            )],
        )],
    )
    .with_entry_op_id(OP_ID)
    .with_non_composable_with_self(true)
}

fn c_sema_scope_phase_body(
    phase: CScopePhase,
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    haystack: &str,
    t: Expr,
    num_tokens: &Expr,
    out_scope_tree: &str,
    packed_haystack: bool,
    symbols_only: bool,
) -> Vec<Node> {
    match phase {
        CScopePhase::Scope => {
            let mut body = vec![Node::let_bind("tok_type", Expr::load(tok_types, t.clone()))];
            body.extend([
                Node::let_bind("scope_id", Expr::u32(0)),
                Node::let_bind("scope_parent_id", Expr::u32(0)),
                Node::let_bind("scope_open", Expr::u32(u32::MAX)),
                Node::let_bind("scope_depth", Expr::u32(0)),
            ]);
            let scope_resolution = vec![
                child_phase(
                    SCOPE_PHASE_OP_ID,
                    SCOPE_BRACE_PHASE_OP_ID,
                    c_sema_scope_phase_body(
                        CScopePhase::ScopeBrace,
                        tok_types,
                        tok_starts,
                        tok_lens,
                        haystack,
                        t.clone(),
                        num_tokens,
                        out_scope_tree,
                        packed_haystack,
                        symbols_only,
                    ),
                ),
                child_phase(
                    SCOPE_PHASE_OP_ID,
                    SCOPE_FUNCTION_PARAMS_PHASE_OP_ID,
                    c_sema_scope_phase_body(
                        CScopePhase::ScopeFunctionParameters,
                        tok_types,
                        tok_starts,
                        tok_lens,
                        haystack,
                        t.clone(),
                        num_tokens,
                        out_scope_tree,
                        packed_haystack,
                        symbols_only,
                    ),
                ),
            ];
            if symbols_only {
                body.push(Node::if_then(
                    Expr::eq(
                        Expr::var("tok_type"),
                        Expr::u32(crate::parsing::c::lex::tokens::TOK_IDENTIFIER),
                    ),
                    scope_resolution,
                ));
            } else {
                body.extend(scope_resolution);
            }
            body.extend(store_scope_nodes(out_scope_tree, t));
            body
        }
        CScopePhase::ScopeBrace => emit_brace_scope_resolution(tok_types, t, num_tokens),
        CScopePhase::ScopeFunctionParameters => {
            emit_function_parameter_scope(tok_types, t, num_tokens)
        }
        CScopePhase::Decl => {
            let mut body = vec![Node::let_bind("tok_type", Expr::load(tok_types, t.clone()))];
            let mut decl_body = emit_declaration_lookup(t.clone(), num_tokens);
            decl_body.push(Node::store(
                out_scope_tree,
                Expr::add(Expr::mul(t.clone(), Expr::u32(4)), Expr::u32(2)),
                Expr::var("decl_kind"),
            ));
            if symbols_only {
                body.push(Node::if_then(
                    Expr::eq(
                        Expr::var("tok_type"),
                        Expr::u32(crate::parsing::c::lex::tokens::TOK_IDENTIFIER),
                    ),
                    decl_body,
                ));
            } else {
                body.extend(decl_body);
            }
            body
        }
        CScopePhase::IdentifierIntern => {
            let mut body = vec![Node::let_bind("tok_type", Expr::load(tok_types, t.clone()))];
            let mut intern_body =
                emit_identifier_intern(tok_starts, tok_lens, haystack, t.clone(), packed_haystack);
            intern_body.push(Node::store(
                out_scope_tree,
                Expr::add(Expr::mul(t, Expr::u32(4)), Expr::u32(3)),
                Expr::var("identifier_intern_id"),
            ));
            if symbols_only {
                body.push(Node::if_then(
                    Expr::eq(
                        Expr::var("tok_type"),
                        Expr::u32(crate::parsing::c::lex::tokens::TOK_IDENTIFIER),
                    ),
                    intern_body,
                ));
            } else {
                body.extend(intern_body);
            }
            body
        }
    }
}

fn store_scope_nodes(out_scope_tree: &str, t: Expr) -> Vec<Node> {
    vec![
        Node::store(
            out_scope_tree,
            Expr::mul(t.clone(), Expr::u32(4)),
            Expr::var("scope_id"),
        ),
        Node::store(
            out_scope_tree,
            Expr::add(Expr::mul(t, Expr::u32(4)), Expr::u32(1)),
            Expr::var("scope_parent_id"),
        ),
    ]
}

#[must_use]
fn c_sema_scope_phase(
    phase: CScopePhase,
    phase_op_id: &str,
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    haystack: &str,
    haystack_len: Expr,
    num_tokens: Expr,
    out_scope_tree: &str,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let tok_count = match &num_tokens {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    let haystack_words = match &haystack_len {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    let out_words = tok_count.saturating_mul(4).max(1);
    let mut body = c_sema_scope_phase_body(
        phase,
        tok_types,
        tok_starts,
        tok_lens,
        haystack,
        t.clone(),
        &num_tokens,
        out_scope_tree,
        false,
        false,
    );
    if phase == CScopePhase::ScopeBrace {
        let mut standalone = vec![
            Node::let_bind("scope_id", Expr::u32(0)),
            Node::let_bind("scope_parent_id", Expr::u32(0)),
            Node::let_bind("scope_open", Expr::u32(u32::MAX)),
            Node::let_bind("scope_depth", Expr::u32(0)),
        ];
        standalone.extend(body);
        standalone.extend(store_scope_nodes(out_scope_tree, t.clone()));
        body = standalone;
    } else if phase == CScopePhase::ScopeFunctionParameters {
        let mut standalone = vec![
            Node::let_bind("scope_id", Expr::u32(0)),
            Node::let_bind("scope_parent_id", Expr::u32(0)),
        ];
        standalone.extend(body);
        standalone.extend(store_scope_nodes(out_scope_tree, t.clone()));
        body = standalone;
    }
    let body_op_id = format!("anonymous::{phase_op_id}.body");
    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_count.max(1)),
            BufferDecl::storage(tok_starts, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_count.max(1)),
            BufferDecl::storage(tok_lens, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_count.max(1)),
            BufferDecl::storage(haystack, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(haystack_words.max(1)),
            BufferDecl::output(out_scope_tree, 4, DataType::U32).with_count(out_words),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            phase_op_id,
            vec![Node::if_then(
                Expr::lt(t, num_tokens.clone()),
                vec![child_phase(phase_op_id, &body_op_id, body)],
            )],
        )],
    )
    .with_entry_op_id(phase_op_id)
    .with_non_composable_with_self(true)
}
