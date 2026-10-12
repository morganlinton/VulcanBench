//! Generated identity coverage for static adversarial, golden, and KAT records.

use std::collections::HashSet;

use vyre_spec::{AdversarialInput, GoldenSample, KatVector};

const EMPTY: &[u8] = b"";
const ZERO: &[u8] = b"\0";
const HIGH_BITS: &[u8] = &[0xff, 0x80, 0x7f, 0x00];
const CUDA_EDGE: &[u8] = b"cuda-resident-edge";
const CUDA_EDGE_COPY: &[u8] = b"cuda-resident-edge";

const INPUTS: &[&[u8]] = &[EMPTY, ZERO, HIGH_BITS, CUDA_EDGE, CUDA_EDGE_COPY];
const EXPECTED: &[&[u8]] = &[EMPTY, b"\x01", &[0x10, 0x20, 0x30, 0x40], b"reference"];
const REASONS: &[&str] = &[
    "empty boundary",
    "zero byte boundary",
    "high-bit payload",
    "resident CUDA edge",
    "unicode boundary: 数据",
];
const OP_IDS: &[&str] = &[
    "vyre.spec.generated.identity.add",
    "vyre.spec.generated.identity.select",
    "vyre.spec.generated.identity.cuda_resident",
];

#[test]
fn generated_static_vectors_compare_by_contents_not_pointer_identity() {
    let first = KatVector {
        input: CUDA_EDGE,
        expected: b"reference",
        source: "same-content-slice",
    };
    let second = KatVector {
        input: CUDA_EDGE_COPY,
        expected: b"reference",
        source: "same-content-slice",
    };

    assert_eq!(
        first, second,
        "Fix: static vector equality must compare byte contents, not slice addresses."
    );
}

#[test]
fn generated_static_vector_hash_and_equality_matrix_distinguishes_all_contract_fields() {
    let mut adversarial = HashSet::new();
    let mut golden = HashSet::new();
    let mut kat = HashSet::new();
    let mut checked = 0usize;

    for input in INPUTS {
        for expected in EXPECTED {
            for reason in REASONS {
                adversarial.insert(AdversarialInput { input, reason });
                kat.insert(KatVector {
                    input,
                    expected,
                    source: reason,
                });
                for op_id in OP_IDS {
                    golden.insert(GoldenSample {
                        op_id,
                        input,
                        expected,
                        reason,
                    });
                    checked += 1;
                }
            }
        }
    }

    assert_eq!(
        checked,
        INPUTS.len() * EXPECTED.len() * REASONS.len() * OP_IDS.len()
    );
    assert_eq!(
        adversarial.len(),
        4 * REASONS.len(),
        "Fix: AdversarialInput hash identity must include input bytes and reason, while equal byte slices collapse."
    );
    assert_eq!(
        kat.len(),
        4 * EXPECTED.len() * REASONS.len(),
        "Fix: KatVector hash identity must include input bytes, expected bytes, and source."
    );
    assert_eq!(
        golden.len(),
        4 * EXPECTED.len() * REASONS.len() * OP_IDS.len(),
        "Fix: GoldenSample hash identity must include op id, input bytes, expected bytes, and reason."
    );
}
