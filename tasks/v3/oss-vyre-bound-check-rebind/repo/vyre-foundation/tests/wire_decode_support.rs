//! Shared wire-decoder adversarial input support for foundation integration tests.

use vyre::ir::{BufferDecl, DataType, Node, Program};

pub(crate) fn minimal_program_bytes() -> Vec<u8> {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    )
    .to_wire()
    .expect("Fix: minimal wire-hostile fixture must encode")
}

pub(crate) fn assert_decode_is_safe(bytes: &[u8], context: &str) {
    let result = std::panic::catch_unwind(|| Program::from_wire(bytes));

    match result {
        Ok(Ok(program)) => {
            let reencoded = program
                .to_wire()
                .expect("Fix: any program accepted from wire must re-encode cleanly");
            Program::from_wire(&reencoded)
                .expect("Fix: any re-encoded accepted wire Program must decode again cleanly");
        }
        Ok(Err(error)) => {
            assert!(
                is_structured_wire_error(&error.to_string()),
                "Fix: {context} returned non-actionable error: {error}"
            );
        }
        Err(_) => panic!("Fix: Program::from_wire panicked on {context}"),
    }
}

pub(crate) fn decode_error_string(bytes: &[u8], context: &str) -> String {
    let result = std::panic::catch_unwind(|| Program::from_wire(bytes));
    match result {
        Ok(Ok(_)) => {
            panic!("Fix: Program::from_wire accepted {context}; decoder must fail closed.")
        }
        Ok(Err(error)) => {
            let error = error.to_string();
            assert!(
                is_structured_wire_error(&error),
                "Fix: {context} returned non-actionable error: {error}"
            );
            error
        }
        Err(_) => panic!("Fix: Program::from_wire panicked on {context}"),
    }
}

pub(crate) fn hostile_bytes(seed: u64) -> Vec<u8> {
    let mut state = seed ^ 0xA5A5_5A5A_D3C1_B2A0;
    let len = (next_u64(&mut state) as usize) & 0x1ff;
    let mut bytes = Vec::with_capacity(len);
    for _ in 0..len {
        bytes.push(next_u64(&mut state) as u8);
    }

    match seed & 7 {
        0 if len >= 4 => bytes[0..4].copy_from_slice(b"VIR0"),
        1 if len >= 8 => bytes[0..8].copy_from_slice(&(u64::MAX / 2).to_le_bytes()),
        2 if len >= 8 => bytes[0..8].copy_from_slice(&(64_u64 * 1024 * 1024 + 1).to_le_bytes()),
        3 if len >= 40 => {
            bytes[0..4].copy_from_slice(b"VIR0");
            bytes[4..8].copy_from_slice(&[0xff; 4]);
        }
        4 if len >= 16 => bytes[8..16].copy_from_slice(&[0xff; 8]),
        _ => {}
    }
    bytes
}

pub(crate) fn mutation_dictionary(valid: &[u8]) -> Vec<Vec<u8>> {
    let mut fragments = vec![
        Vec::new(),
        b"VIR0".to_vec(),
        b"VIR1".to_vec(),
        vec![0xff; 4],
        vec![0xff; 8],
        vec![0xff; 32],
        0_u16.to_le_bytes().to_vec(),
        1_u16.to_le_bytes().to_vec(),
        u16::MAX.to_le_bytes().to_vec(),
        0_u32.to_le_bytes().to_vec(),
        1_u32.to_le_bytes().to_vec(),
        u32::MAX.to_le_bytes().to_vec(),
        0_u64.to_le_bytes().to_vec(),
        1_u64.to_le_bytes().to_vec(),
        u64::MAX.to_le_bytes().to_vec(),
        (64_u64 * 1024 * 1024 + 1).to_le_bytes().to_vec(),
        (0_u8..=18).collect(),
        vec![0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
        vec![0x80, 0xff, 0xff, 0xff, 0x7f],
    ];
    for window in [4usize, 8, 16, 32, 40, 64] {
        if valid.len() >= window {
            fragments.push(valid[..window].to_vec());
            fragments.push(valid[valid.len() - window..].to_vec());
        }
    }
    fragments
}

pub(crate) fn next_u64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn is_structured_wire_error(error: &str) -> bool {
    error.contains("Fix:")
        || error.contains("TruncatedPayload")
        || error.contains("InvalidDiscriminant")
        || error.contains("IntegrityMismatch")
        || error.contains("MagicMismatch")
        || error.contains("VersionMismatch")
        || error.contains("TooLarge")
        || error.contains("wire")
        || error.contains("Wire")
}
