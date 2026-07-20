use crate::common::{bytes_u32, with_live_backend};
use vyre::DispatchConfig;
use vyre_primitives::parsing::line_splice_classify::{
    line_splice_classify_dispatch_grid, line_splice_classify_u8, reference_line_splice_classify,
};

fn generated_line_splice_u8_source(case: u32, len: usize) -> Vec<u8> {
    let mut state = 0xc2b2_ae35_u32 ^ case.wrapping_mul(0x27d4_eb2d);
    let mut source = Vec::with_capacity(len);
    for index in 0..len {
        state = state
            .rotate_left(7)
            .wrapping_mul(0x85eb_ca6b)
            .wrapping_add(index as u32);
        let byte = match state % 29 {
            0 => b'\\',
            1 => b'\n',
            2 => b'\r',
            3 => 0,
            4 => 0xFF,
            _ => b'a' + ((state >> 8) % 26) as u8,
        };
        source.push(byte);
    }

    for &offset in &[0usize, 1, 2, 254, 255, 256, 510, 511, 768, 1023] {
        if offset + 3 <= source.len() {
            match (case + offset as u32) % 4 {
                0 => {
                    source[offset] = b'\\';
                    source[offset + 1] = b'\n';
                }
                1 => {
                    source[offset] = b'\\';
                    source[offset + 1] = b'\r';
                    source[offset + 2] = b'\n';
                }
                2 => {
                    source[offset] = b'\\';
                    source[offset + 1] = b'\r';
                    source[offset + 2] = b'x';
                }
                _ => {
                    source[offset] = b'\\';
                    source[offset + 1] = b'\\';
                    source[offset + 2] = b'\n';
                }
            }
        }
    }
    source
}

#[test]
fn cuda_line_splice_classify_u8_generated_matrix_matches_cpu() {
    let len = 1025usize;
    let byte_count = len as u32;
    let program = line_splice_classify_u8(byte_count);
    let mut config = DispatchConfig::default();
    config.grid_override = Some(line_splice_classify_dispatch_grid(byte_count));

    with_live_backend("raw-u8 generated line-splice matrix", |backend| {
        let mut checked = 0usize;
        for case in 0..128u32 {
            let source = generated_line_splice_u8_source(case, len);
            let inputs: Vec<Vec<u8>> = vec![source.clone(), vec![0u8; len * 4]];
            let outputs = backend
                .dispatch(&program, &inputs, &config)
                .unwrap_or_else(|error| {
                    panic!("Fix: CUDA raw-u8 line-splice generated case {case} failed: {error}")
                });
            let mut gpu = bytes_u32(&outputs[0]);
            gpu.truncate(len);
            assert_eq!(
                gpu,
                reference_line_splice_classify(&source),
                "Fix: raw-u8 CUDA line-splice mismatch on generated case {case}"
            );
            checked += gpu.len();
        }
        assert_eq!(
            checked,
            128 * len,
            "Fix: generated raw-u8 CUDA line-splice matrix must compare every byte lane."
        );
    });
}
