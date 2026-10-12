//! Temporary helper to compute pinned constants for cert_regression_pin.rs

use ed25519_dalek::{Signer, SigningKey};
use std::sync::Arc;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_conform_runner::{issue_bundle_cert, CorpusWitness};
use vyre_driver::registry::{
    Category, LoweringTable, OpDef, OpDefRegistration, Signature, TypedParam,
};
use vyre_primitives::wire::pack_u32_slice as bytes_u32;

const TEST_IDENTITY_U32_OP: &str = "vyre-conform.test.identity_u32";

fn identity_u32_cpu_ref(input: &[u8], output: &mut Vec<u8>) {
    output.clear();
    output.extend_from_slice(input.get(..4).unwrap_or(&[0, 0, 0, 0]));
}

const TEST_IDENTITY_U32_SIGNATURE: Signature = Signature {
    inputs: &[TypedParam {
        name: "value",
        ty: "u32",
    }],
    outputs: &[TypedParam {
        name: "out",
        ty: "u32",
    }],
    attrs: &[],
    bytes_extraction: false,
};

inventory::submit! {
    OpDefRegistration::new(|| OpDef {
        id: TEST_IDENTITY_U32_OP,
        dialect: "vyre-conform-test",
        category: Category::Intrinsic,
        signature: TEST_IDENTITY_U32_SIGNATURE,
        lowerings: LoweringTable::new(identity_u32_cpu_ref),
        laws: &[],
        compose: None,
    })
}

fn deterministic_key() -> SigningKey {
    let seed = blake3::hash(b"RELEASE-PROOF-L11-cert-regression-pin");
    let mut seed_arr = [0u8; 32];
    seed_arr.copy_from_slice(seed.as_bytes());
    SigningKey::from_bytes(&seed_arr)
}

fn sign(cert: &mut vyre_conform_runner::BundleCertificate, key: &SigningKey) {
    let signable = serde_json::json!({
        "version": cert.version,
        "bundle_blake3": cert.bundle_blake3,
        "corpus_blake3": cert.corpus_blake3,
        "reference_output_blake3": cert.reference_output_blake3,
        "witness_count": cert.witness_count,
        "timestamp": cert.timestamp,
        "pubkey": hex::encode(key.verifying_key().to_bytes()),
    });
    let signable_bytes = serde_json::to_vec(&signable).unwrap();
    let signature = key.sign(&signable_bytes);
    cert.signature_ed25519 = hex::encode(signature.to_bytes());
    cert.pubkey = hex::encode(key.verifying_key().to_bytes());
}

fn trivial_const() -> Program {
    Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    )
}

fn one_op_add() -> Program {
    Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(Expr::u32(1), Expr::u32(2)),
        )],
    )
}

fn loop_add() -> Program {
    Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::let_bind("acc", Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(4),
                vec![Node::assign(
                    "acc",
                    Expr::add(Expr::var("acc"), Expr::var("i")),
                )],
            ),
            Node::store("out", Expr::u32(0), Expr::var("acc")),
        ],
    )
}

fn composed_nested() -> Program {
    let inner = vec![Node::store("out", Expr::u32(0), Expr::u32(7))];
    let outer = vec![Node::Region {
        generator: "inner".into(),
        source_region: None,
        body: Arc::new(inner),
    }];
    Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Region {
            generator: "outer".into(),
            source_region: None,
            body: Arc::new(outer),
        }],
    )
}

fn region_chain_intrinsic_dialect() -> Program {
    let body = vec![
        Node::let_bind("acc", Expr::u32(0)),
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(3),
            vec![Node::assign(
                "acc",
                Expr::add(Expr::var("acc"), Expr::var("i")),
            )],
        ),
        Node::let_bind(
            "dial",
            Expr::call(TEST_IDENTITY_U32_OP, vec![Expr::var("acc")]),
        ),
        Node::store("out", Expr::u32(0), Expr::var("dial")),
    ];
    Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Region {
            generator: "vyre.intrinsics.math.accum".into(),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

#[test]
fn compute_pins() {
    // Initialize driver registry so dialect ops resolve
    let _ = vyre_driver::registry::DialectRegistry::global();

    let key = deterministic_key();
    let pubkey = hex::encode(key.verifying_key().to_bytes());
    eprintln!("PUBKEY: {}", pubkey);

    let programs: Vec<(&str, Program, Vec<CorpusWitness>)> = vec![
        (
            "trivial_const",
            trivial_const(),
            vec![CorpusWitness {
                name: "tc1".into(),
                inputs: vec![bytes_u32(&[0])],
            }],
        ),
        (
            "one_op_add",
            one_op_add(),
            vec![CorpusWitness {
                name: "add1".into(),
                inputs: vec![bytes_u32(&[0])],
            }],
        ),
        (
            "loop_add",
            loop_add(),
            vec![CorpusWitness {
                name: "loop1".into(),
                inputs: vec![bytes_u32(&[0])],
            }],
        ),
        (
            "composed_nested",
            composed_nested(),
            vec![CorpusWitness {
                name: "nest1".into(),
                inputs: vec![bytes_u32(&[0])],
            }],
        ),
        (
            "region_chain_intrinsic_dialect",
            region_chain_intrinsic_dialect(),
            vec![CorpusWitness {
                name: "rd1".into(),
                inputs: vec![bytes_u32(&[0])],
            }],
        ),
    ];

    for (name, prog, corpus) in programs {
        let mut cert = issue_bundle_cert(&prog, &corpus, "2026-04-23T20:00:00Z", "TBD", "TBD")
            .unwrap_or_else(|e| panic!("issue failed for {}: {}", name, e));
        sign(&mut cert, &key);
        let wire = prog.to_wire().unwrap();
        eprintln!(
            "{}: BUNDLE_HASH={} WIRE_LEN={} SIG={}",
            name,
            cert.bundle_blake3,
            wire.len(),
            cert.signature_ed25519
        );
    }
}
