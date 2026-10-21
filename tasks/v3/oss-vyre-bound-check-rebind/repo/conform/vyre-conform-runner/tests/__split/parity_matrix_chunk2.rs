fn synthetic_entries() -> Vec<UnifiedEntry> {
    vec![UnifiedEntry {
        id: "vyre-conform::synthetic::expr_variant_contract_bundle",
        build: synthetic_expr_variant_contract_program,
        test_inputs: Some(synthetic_scalar_inputs),
        expected_output: Some(synthetic_zero_output),
    }]
}

fn synthetic_expr_variant_contract_program() -> Program {
    Program::wrapped(
        vec![vyre::ir::BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::If {
                cond: Expr::LitBool(false),
                then: vec![
                    Node::let_bind("lit_i32", Expr::LitI32(-4)),
                    Node::let_bind("workgroup_id", Expr::WorkgroupId { axis: 0 }),
                    Node::let_bind(
                        "call",
                        Expr::Call {
                            op_id: "unknown.op".into(),
                            args: vec![],
                        },
                    ),
                    Node::let_bind(
                        "subgroup_ballot",
                        Expr::SubgroupBallot {
                            cond: Box::new(Expr::LitBool(true)),
                        },
                    ),
                    Node::let_bind(
                        "subgroup_add",
                        Expr::subgroup_add(Expr::LitU32(7)),
                    ),
                    Node::let_bind("opaque", Expr::Opaque(Arc::new(SyntheticOpaqueExpr))),
                ],
                otherwise: vec![],
            },
            Node::store("out", Expr::u32(0), Expr::u32(0)),
            Node::Return,
        ],
    )
}

fn synthetic_scalar_inputs() -> FixtureCases {
    vec![vec![0_u32.to_le_bytes().to_vec()]]
}

fn synthetic_zero_output() -> FixtureCases {
    vec![vec![0_u32.to_le_bytes().to_vec()]]
}

fn expr_variant_rows(entries: &[UnifiedEntry]) -> BTreeMap<&'static str, Vec<&'static str>> {
    let mut rows = BTreeMap::<&'static str, BTreeSet<&'static str>>::new();
    for entry in entries {
        let variants = expr_variants_in_program((entry.build)());
        for variant in variants {
            rows.entry(variant).or_default().insert(entry.id);
        }
    }
    rows.into_iter()
        .map(|(variant, ids)| (variant, ids.into_iter().collect::<Vec<_>>()))
        .collect()
}

fn expr_variants_in_program(program: Program) -> BTreeSet<&'static str> {
    let mut variants = BTreeSet::new();
    for node in program.entry() {
        collect_expr_variants_from_node(node, &mut variants);
    }
    variants
}

fn collect_expr_variants_from_node(node: &Node, variants: &mut BTreeSet<&'static str>) {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => {
            collect_expr_variants(value, variants);
        }
        Node::Store { index, value, .. } => {
            collect_expr_variants(index, variants);
            collect_expr_variants(value, variants);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            collect_expr_variants(cond, variants);
            for child in then {
                collect_expr_variants_from_node(child, variants);
            }
            for child in otherwise {
                collect_expr_variants_from_node(child, variants);
            }
        }
        Node::Loop { from, to, body, .. } => {
            collect_expr_variants(from, variants);
            collect_expr_variants(to, variants);
            for child in body {
                collect_expr_variants_from_node(child, variants);
            }
        }
        Node::Block(children) => {
            for child in children {
                collect_expr_variants_from_node(child, variants);
            }
        }
        Node::Region { body, .. } => {
            for child in body.iter() {
                collect_expr_variants_from_node(child, variants);
            }
        }
        Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
            collect_expr_variants(offset, variants);
            collect_expr_variants(size, variants);
        }
        Node::Trap { address, .. } => collect_expr_variants(address, variants),
        Node::Return
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => {}
        _ => panic!(
            "Fix: parity_matrix node traversal is missing a non-exhaustive Node variant; update expr coverage recursion before landing new IR surface."
        ),
    }
}

fn collect_expr_variants(expr: &vyre::ir::Expr, variants: &mut BTreeSet<&'static str>) {
    use vyre::ir::Expr;

    match expr {
        Expr::LitU32(_) => {
            variants.insert("LitU32");
        }
        Expr::LitI32(_) => {
            variants.insert("LitI32");
        }
        Expr::LitF32(_) => {
            variants.insert("LitF32");
        }
        Expr::LitBool(_) => {
            variants.insert("LitBool");
        }
        Expr::Var(_) => {
            variants.insert("Var");
        }
        Expr::Load { index, .. } => {
            variants.insert("Load");
            collect_expr_variants(index, variants);
        }
        Expr::BufLen { .. } => {
            variants.insert("BufLen");
        }
        Expr::InvocationId { .. } => {
            variants.insert("InvocationId");
        }
        Expr::WorkgroupId { .. } => {
            variants.insert("WorkgroupId");
        }
        Expr::LocalId { .. } => {
            variants.insert("LocalId");
        }
        Expr::BinOp { left, right, .. } => {
            variants.insert("BinOp");
            collect_expr_variants(left, variants);
            collect_expr_variants(right, variants);
        }
        Expr::UnOp { operand, .. } => {
            variants.insert("UnOp");
            collect_expr_variants(operand, variants);
        }
        Expr::Call { args, .. } => {
            variants.insert("Call");
            for arg in args {
                collect_expr_variants(arg, variants);
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            variants.insert("Select");
            collect_expr_variants(cond, variants);
            collect_expr_variants(true_val, variants);
            collect_expr_variants(false_val, variants);
        }
        Expr::Cast { value, .. } => {
            variants.insert("Cast");
            collect_expr_variants(value, variants);
        }
        Expr::Fma { a, b, c } => {
            variants.insert("Fma");
            collect_expr_variants(a, variants);
            collect_expr_variants(b, variants);
            collect_expr_variants(c, variants);
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            variants.insert("Atomic");
            collect_expr_variants(index, variants);
            if let Some(expected) = expected {
                collect_expr_variants(expected, variants);
            }
            collect_expr_variants(value, variants);
        }
        Expr::SubgroupBallot { cond } => {
            variants.insert("SubgroupBallot");
            collect_expr_variants(cond, variants);
        }
        Expr::SubgroupShuffle { value, lane } => {
            variants.insert("SubgroupShuffle");
            collect_expr_variants(value, variants);
            collect_expr_variants(lane, variants);
        }
        Expr::SubgroupReduce { value, .. } => {
            variants.insert("SubgroupReduce");
            collect_expr_variants(value, variants);
        }
        Expr::SubgroupLocalId => {
            variants.insert("SubgroupLocalId");
        }
        Expr::SubgroupSize => {
            variants.insert("SubgroupSize");
        }
        Expr::Opaque(_) => {
            variants.insert("Opaque");
        }
        _ => panic!(
            "Fix: parity_matrix expr traversal is missing a non-exhaustive Expr variant; add it to vyre-spec expr_variants() and the coverage walker."
        ),
    }
}

fn assert_valid(op_id: &str, program: &Program, runners: &[BackendRunner]) {
    if op_id == "vyre-conform::synthetic::expr_variant_contract_bundle" {
        return;
    }
    let backend_capabilities = BackendCapabilities {
        supports_subgroup_ops: runners.iter().any(|runner| match &runner.kind {
            BackendKind::ReferenceBackend => true,
            BackendKind::Registered(backend) => backend.supports_subgroup_ops(),
        }),
        supports_indirect_dispatch: false,
        supports_specialization_constants: false,
        // BackendCapabilities grew 8 additional fields (has_dual_issue_fp32_int32,
        // has_mul_high, has_native_f16, etc.) since this fixture was written;
        // the parity-matrix discipline only cares about the three above so
        // populate the rest from `Default` to stay forward-compatible.
        ..Default::default()
    };
    let errors = validate_with_options(
        program,
        ValidationOptions::default().with_backend_capabilities(backend_capabilities),
    )
    .errors;
    assert!(
        errors.is_empty(),
        "Fix: {} validation failed before parity run: {:?}",
        op_id,
        errors
            .into_iter()
            .map(|error| error.message().to_string())
            .collect::<Vec<_>>()
    );
}

fn assert_region_chain(op_id: &str, program: &Program) {
    let first = program.entry().first().unwrap_or_else(|| {
        panic!(
            "Fix: {} built an empty Program; OpEntry::build must return a region-wrapped body.",
            op_id
        )
    });
    match first {
        Node::Region { .. } => {}
        other => panic!(
            "Fix: {} top-level entry node must be Node::Region to preserve the region chain invariant, got {other:?}.",
            op_id
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_outputs(
    op_id: &'static str,
    backend_a: &'static str,
    backend_b: &'static str,
    input_hash: Hash,
    outputs_a: &[Vec<u8>],
    outputs_b: &[Vec<u8>],
    program: &Program,
    divergences: &mut Vec<Divergence>,
) {
    let hash_a = hash_buffers(outputs_a);
    let hash_b = hash_buffers(outputs_b);
    if hash_a == hash_b {
        return;
    }

    if let BufferParity::Mismatch(detail) = compare_output_buffers(program, outputs_a, outputs_b) {
        divergences.push(Divergence {
            op_id,
            backend_a,
            backend_b,
            input_hash,
            output_a_hash: hash_a,
            output_b_hash: hash_b,
            detail,
        });
    }
}

fn hash_program(program: &Program) -> Hash {
    let wire = program.to_wire().unwrap_or_else(|error| {
        panic!("Fix: failed to encode Program wire image for parity hash: {error}")
    });
    blake3::hash(&wire)
}

fn hash_buffers(buffers: &[Vec<u8>]) -> Hash {
    let mut hasher = blake3::Hasher::new();
    for buffer in buffers {
        hasher.update(&(buffer.len() as u64).to_le_bytes());
        hasher.update(buffer);
    }
    hasher.finalize()
}

fn format_divergences(divergences: &[Divergence]) -> String {
    let mut message = String::from("Cross-backend parity divergences detected:\n");
    for divergence in divergences {
        message.push_str(&format!(
            "op_id={} backend_a={} backend_b={} input_hash={} output_a_hash={} output_b_hash={} detail={}\n",
            divergence.op_id,
            divergence.backend_a,
            divergence.backend_b,
            divergence.input_hash,
            divergence.output_a_hash,
            divergence.output_b_hash,
            divergence.detail
        ));
    }
    message
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}
