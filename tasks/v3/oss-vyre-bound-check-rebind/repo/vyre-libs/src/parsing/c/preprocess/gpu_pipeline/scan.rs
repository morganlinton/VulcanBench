use vyre::ir::{BufferAccess, Program};
use vyre_primitives::reduce::multi_block_prefix_scan::{
    multi_block_prefix_scan_sum_u32, pass_a_local_scan, pass_c_broadcast_offsets, BLOCK_LANES,
};

use super::GpuDispatcher;

#[derive(Default)]
pub(super) struct PrefixScanScratch {
    small_zero: Vec<u8>,
    small_outputs: Vec<Vec<u8>>,
    pass_a_partials_zero: Vec<u8>,
    pass_a_totals_zero: Vec<u8>,
    pass_a_outputs: Vec<Vec<u8>>,
    block_totals_input: Vec<u8>,
    scanned_block_totals: Vec<u8>,
    nested: Option<Box<PrefixScanScratch>>,
    pass_c_zero: Vec<u8>,
    pass_c_outputs: Vec<Vec<u8>>,
}

impl PrefixScanScratch {
    fn prepare_zero(out: &mut Vec<u8>, byte_len: usize) -> Result<(), String> {
        out.clear();
        out.try_reserve_exact(byte_len).map_err(|error| {
            format!(
                "prefix scan: could not reserve {byte_len} zero-staging bytes: {error:?}. Fix: shard the GPU prefix scan input."
            )
        })?;
        out.resize(byte_len, 0);
        Ok(())
    }
}

fn prefix_scan_word_bytes(word_count: u32, field: &'static str) -> Result<usize, String> {
    (word_count as usize)
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            format!(
                "prefix scan: {field} word count {word_count} overflows host byte sizing. Fix: shard the GPU prefix scan input."
            )
        })
}

fn prefix_scan_product_word_bytes(
    left: u32,
    right: u32,
    field: &'static str,
) -> Result<usize, String> {
    (left as usize)
        .checked_mul(right as usize)
        .and_then(|words| words.checked_mul(std::mem::size_of::<u32>()))
        .ok_or_else(|| {
            format!(
                "prefix scan: {field} word product {left} x {right} overflows host byte sizing. Fix: shard the GPU prefix scan input."
            )
        })
}

pub(super) fn inclusive_prefix_scan_u32_into(
    dispatcher: &dyn GpuDispatcher,
    input_words_le: &[u8],
    n: u32,
    scratch: &mut PrefixScanScratch,
    out: &mut Vec<u8>,
) -> Result<(), String> {
    if n > BLOCK_LANES {
        return inclusive_prefix_scan_u32_large_into(dispatcher, input_words_le, n, scratch, out);
    }
    let scan = multi_block_prefix_scan_sum_u32("scan_in", "scan_out", n);
    if dispatcher.requires_output_inputs() {
        let small_zero_bytes = prefix_scan_word_bytes(n, "small output")?;
        PrefixScanScratch::prepare_zero(&mut scratch.small_zero, small_zero_bytes)?;
        dispatcher.dispatch_borrowed_into(
            &scan,
            &[input_words_le, scratch.small_zero.as_slice()],
            &mut scratch.small_outputs,
        )?;
        if scratch.small_outputs.len() != 1 {
            return Err(format!(
                "prefix scan: expected exactly 1 output, got {}. Fix: backend must return only scan_out.",
                scratch.small_outputs.len()
            ));
        }
    } else {
        dispatcher.dispatch_borrowed_into(&scan, &[input_words_le], &mut scratch.small_outputs)?;
        if scratch.small_outputs.len() != 1 {
            return Err(format!(
                "prefix scan: expected exactly 1 output, got {}. Fix: backend must return only scan_out.",
                scratch.small_outputs.len()
            ));
        }
    }
    out.clear();
    out.extend_from_slice(&scratch.small_outputs[0]);
    Ok(())
}

fn inclusive_prefix_scan_u32_large_into(
    dispatcher: &dyn GpuDispatcher,
    input_words_le: &[u8],
    n: u32,
    scratch: &mut PrefixScanScratch,
    out: &mut Vec<u8>,
) -> Result<(), String> {
    let num_blocks = n.div_ceil(BLOCK_LANES);
    let mut pass_a = pass_a_local_scan(
        "scan_in",
        "scan_partials",
        "scan_block_totals",
        n,
        num_blocks,
    );
    if dispatcher.requires_output_inputs() {
        pass_a = live_out_readwrite_buffers(pass_a, &["scan_partials", "scan_block_totals"]);
        let pass_a_partials_bytes =
            prefix_scan_product_word_bytes(num_blocks, BLOCK_LANES, "pass A partials")?;
        let pass_a_totals_bytes = prefix_scan_word_bytes(num_blocks, "pass A block totals")?;
        PrefixScanScratch::prepare_zero(&mut scratch.pass_a_partials_zero, pass_a_partials_bytes)?;
        PrefixScanScratch::prepare_zero(&mut scratch.pass_a_totals_zero, pass_a_totals_bytes)?;
        dispatcher
            .dispatch_borrowed_into(
                &pass_a,
                &[
                    input_words_le,
                    scratch.pass_a_partials_zero.as_slice(),
                    scratch.pass_a_totals_zero.as_slice(),
                ],
                &mut scratch.pass_a_outputs,
            )
            .map_err(|e| format!("pass A: {e}"))?;
    } else {
        dispatcher
            .dispatch_borrowed_into(&pass_a, &[input_words_le], &mut scratch.pass_a_outputs)
            .map_err(|e| format!("pass A: {e}"))?;
    }
    if scratch.pass_a_outputs.len() != 2 {
        return Err(format!(
            "pass A: expected exactly 2 outputs, got {}. Fix: backend must return scan_partials/scan_block_totals and no extras.",
            scratch.pass_a_outputs.len()
        ));
    }

    scratch.block_totals_input.clear();
    scratch
        .block_totals_input
        .extend_from_slice(&scratch.pass_a_outputs[1]);
    let nested = scratch
        .nested
        .get_or_insert_with(|| Box::new(PrefixScanScratch::default()));
    inclusive_prefix_scan_u32_into(
        dispatcher,
        scratch.block_totals_input.as_slice(),
        num_blocks,
        nested,
        &mut scratch.scanned_block_totals,
    )?;

    let pass_c = pass_c_broadcast_offsets(
        "scan_partials",
        "scan_block_totals_scanned",
        "scan_out",
        n,
        num_blocks,
    );
    if dispatcher.requires_output_inputs() {
        let pass_c_zero_bytes = prefix_scan_word_bytes(n, "pass C output")?;
        PrefixScanScratch::prepare_zero(&mut scratch.pass_c_zero, pass_c_zero_bytes)?;
        dispatcher
            .dispatch_borrowed_into(
                &pass_c,
                &[
                    scratch.pass_a_outputs[0].as_slice(),
                    scratch.scanned_block_totals.as_slice(),
                    scratch.pass_c_zero.as_slice(),
                ],
                &mut scratch.pass_c_outputs,
            )
            .map_err(|e| format!("pass C: {e}"))?;
    } else {
        dispatcher
            .dispatch_borrowed_into(
                &pass_c,
                &[
                    scratch.pass_a_outputs[0].as_slice(),
                    scratch.scanned_block_totals.as_slice(),
                ],
                &mut scratch.pass_c_outputs,
            )
            .map_err(|e| format!("pass C: {e}"))?;
    }
    if scratch.pass_c_outputs.len() != 1 {
        return Err(format!(
            "pass C: expected exactly 1 output, got {}. Fix: backend must return only scan_out.",
            scratch.pass_c_outputs.len()
        ));
    }
    out.clear();
    out.extend_from_slice(&scratch.pass_c_outputs[0]);
    Ok(())
}

fn live_out_readwrite_buffers(program: Program, names: &[&str]) -> Program {
    let buffers = program
        .buffers()
        .iter()
        .map(|buffer| {
            let mut buffer = buffer.clone();
            if names.iter().any(|name| *name == buffer.name()) {
                buffer.is_output = false;
                buffer.pipeline_live_out = true;
                buffer.output_byte_range = None;
                buffer.access = BufferAccess::ReadWrite;
            }
            buffer
        })
        .collect();
    program.with_rewritten_buffers(buffers)
}

#[cfg(test)]
mod tests {
    use super::{
        prefix_scan_product_word_bytes, prefix_scan_word_bytes, PrefixScanScratch, BLOCK_LANES,
    };

    #[test]
    fn prefix_scan_word_bytes_uses_checked_u32_sizing() {
        assert_eq!(
            prefix_scan_word_bytes(3, "test").expect("Fix: small word count should fit"),
            12
        );
        assert_eq!(
            prefix_scan_product_word_bytes(2, BLOCK_LANES, "partials")
                .expect("Fix: small partial count should fit"),
            (2 * BLOCK_LANES as usize) * std::mem::size_of::<u32>()
        );
    }

    #[test]
    fn prefix_scan_zero_staging_reserves_before_resize() {
        let mut out = Vec::with_capacity(16);
        PrefixScanScratch::prepare_zero(&mut out, 12)
            .expect("Fix: small zero staging reservation should fit");
        assert_eq!(out, vec![0; 12]);
        assert!(out.capacity() >= 12);
    }
}
