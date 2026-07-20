//! Word-level bitset primitive contracts.

use vyre_primitives::bitset::{and::bitset_and, and_not::bitset_and_not};
use vyre_reference::value::Value;

fn pack(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn unpack(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("u32 chunk")))
        .collect()
}

#[test]
fn bitset_and_overwrites_dirty_output_words() {
    let program = bitset_and("lhs", "rhs", "out", 2);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(pack(&[0xFF00_FF00, 0xAAAA_5555])),
            Value::from(pack(&[0x0F0F_F0F0, 0xFFFF_0000])),
            Value::from(pack(&[u32::MAX, u32::MAX])),
        ],
    )
    .expect("bitset_and reference evaluation must succeed");

    assert_eq!(
        unpack(&outputs[0].to_bytes()),
        vec![0x0F00_F000, 0xAAAA_0000]
    );
}

#[test]
fn bitset_and_not_overwrites_dirty_output_words() {
    let program = bitset_and_not("lhs", "rhs", "out", 2);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(pack(&[0xFFFF_0000, 0xAAAA_5555])),
            Value::from(pack(&[0x0F0F_F0F0, 0xFFFF_0000])),
            Value::from(pack(&[u32::MAX, u32::MAX])),
        ],
    )
    .expect("bitset_and_not reference evaluation must succeed");

    assert_eq!(
        unpack(&outputs[0].to_bytes()),
        vec![0xF0F0_0000, 0x0000_5555]
    );
}
