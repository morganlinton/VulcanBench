//! CUDA e-graph readback and scratch-slab helpers.
//!
//! Keeps host readback accounting, structural-equivalence scratch packing, and
//! byte decoding out of the launch planner. The planner should decide what work
//! runs; this module owns the host/device byte plumbing around that work.

use crate::backend::ordering::sort_unstable_by_key_if_needed;
use crate::backend::{CudaBackend, CudaResidentBuffer};
use crate::egraph_device_image::{CudaEGraphDeviceByteLayout, CudaEGraphDeviceByteSpan};
use crate::egraph_kernel_plan::CudaEGraphStructuralEquivalenceLaunchArtifact;
use vyre_driver::BackendError;
use vyre_foundation::optimizer::eqsat_gpu::Equivalence;

pub(crate) fn egraph_column_snapshot_spans(
    layout: CudaEGraphDeviceByteLayout,
) -> [CudaEGraphDeviceByteSpan; 6] {
    [
        layout.row_eclass_ids(),
        layout.row_language_op_ids(),
        layout.row_children_offsets(),
        layout.row_children_lens(),
        layout.row_signatures(),
        layout.children(),
    ]
}

pub(crate) fn egraph_column_snapshot_readback_bytes(
    layout: CudaEGraphDeviceByteLayout,
) -> Result<usize, BackendError> {
    let mut total = 0usize;
    for span in egraph_column_snapshot_spans(layout) {
        total = total
            .checked_add(span.byte_len())
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA e-graph resident column snapshot readback byte accounting overflowed host usize."
                    .to_string(),
            })?;
    }
    Ok(total)
}

pub(crate) fn download_structural_equivalence_output_ranges(
    backend: &CudaBackend,
    scratch: &StructuralEquivalenceScratchSlab,
) -> Result<(Vec<u8>, Vec<u8>), BackendError> {
    let ranges = [
        (scratch.handle, scratch.output_count_offset, 8),
        (
            scratch.handle,
            scratch.output_pairs_offset,
            scratch.output_pairs_bytes,
        ),
    ];
    let mut count_bytes = Vec::new();
    let mut pair_bytes = Vec::new();
    let mut outputs: [&mut Vec<u8>; 2] = [&mut count_bytes, &mut pair_bytes];
    backend.download_resident_ranges_into(&ranges, &mut outputs)?;
    Ok((count_bytes, pair_bytes))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StructuralEquivalenceScratchSlab {
    pub(crate) handle: CudaResidentBuffer,
    pub(crate) bucket_words_offset: usize,
    pub(crate) bucket_rows_offset: usize,
    pub(crate) output_pairs_offset: usize,
    pub(crate) output_pairs_bytes: usize,
    pub(crate) output_count_offset: usize,
}

pub(crate) fn upload_structural_equivalence_scratch(
    backend: &CudaBackend,
    artifact: &CudaEGraphStructuralEquivalenceLaunchArtifact,
) -> Result<StructuralEquivalenceScratchSlab, BackendError> {
    Ok(build_structural_equivalence_scratch_bytes(artifact)?.upload(backend)?)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StructuralEquivalenceScratchBytes {
    bytes: Vec<u8>,
    bucket_words_offset: usize,
    bucket_rows_offset: usize,
    output_pairs_offset: usize,
    output_pairs_bytes: usize,
    output_count_offset: usize,
}

impl StructuralEquivalenceScratchBytes {
    fn upload(
        self,
        backend: &CudaBackend,
    ) -> Result<StructuralEquivalenceScratchSlab, BackendError> {
        let handle = upload_resident_bytes(backend, &self.bytes)?;
        Ok(StructuralEquivalenceScratchSlab {
            handle,
            bucket_words_offset: self.bucket_words_offset,
            bucket_rows_offset: self.bucket_rows_offset,
            output_pairs_offset: self.output_pairs_offset,
            output_pairs_bytes: self.output_pairs_bytes,
            output_count_offset: self.output_count_offset,
        })
    }
}

fn build_structural_equivalence_scratch_bytes(
    artifact: &CudaEGraphStructuralEquivalenceLaunchArtifact,
) -> Result<StructuralEquivalenceScratchBytes, BackendError> {
    let bucket_words_bytes = u32_word_bytes_len(
        artifact.bucket_image.bucket_words.len(),
        "signature bucket words",
    )?
    .max(4);
    let bucket_rows_bytes = u32_word_bytes_len(
        artifact.bucket_image.bucket_rows.len(),
        "signature bucket rows",
    )?
    .max(4);
    let output_pairs_bytes = artifact.output.output_pair_bytes.max(8);
    let output_count_bytes = artifact.output.output_counter_bytes.max(8);

    let mut cursor = 0_usize;
    let bucket_words_offset =
        reserve_aligned_scratch_span(&mut cursor, bucket_words_bytes, 4, "bucket words")?;
    let bucket_rows_offset =
        reserve_aligned_scratch_span(&mut cursor, bucket_rows_bytes, 4, "bucket rows")?;
    let output_pairs_offset =
        reserve_aligned_scratch_span(&mut cursor, output_pairs_bytes, 8, "output pairs")?;
    let output_count_offset =
        reserve_aligned_scratch_span(&mut cursor, output_count_bytes, 8, "output count")?;

    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(cursor)
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph fused structural-equivalence scratch could not reserve {cursor} host bytes before resident upload: {error}. Shard the e-graph image before launch."
            ),
        })?;
    bytes.resize(cursor, 0);
    write_u32_words_at(
        &mut bytes,
        bucket_words_offset,
        &artifact.bucket_image.bucket_words,
        "signature bucket words",
    )?;
    write_u32_words_at(
        &mut bytes,
        bucket_rows_offset,
        &artifact.bucket_image.bucket_rows,
        "signature bucket rows",
    )?;
    Ok(StructuralEquivalenceScratchBytes {
        bytes,
        bucket_words_offset,
        bucket_rows_offset,
        output_pairs_offset,
        output_pairs_bytes,
        output_count_offset,
    })
}

fn reserve_aligned_scratch_span(
    cursor: &mut usize,
    byte_len: usize,
    align: usize,
    field: &'static str,
) -> Result<usize, BackendError> {
    let aligned = align_up(*cursor, align, field)?;
    let end = aligned
        .checked_add(byte_len)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph fused scratch {field} span overflowed host usize addressing. Shard the e-graph image before launch."
            ),
        })?;
    *cursor = end;
    Ok(aligned)
}

fn align_up(value: usize, align: usize, field: &'static str) -> Result<usize, BackendError> {
    let mask = align
        .checked_sub(1)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!("Fix: CUDA e-graph fused scratch {field} received zero alignment."),
        })?;
    let added = value.checked_add(mask).ok_or_else(|| BackendError::InvalidProgram {
        fix: format!(
            "Fix: CUDA e-graph fused scratch {field} alignment overflowed host usize addressing. Shard the e-graph image before launch."
        ),
    })?;
    Ok(added & !mask)
}

pub(crate) fn device_ptr_at(
    base_ptr: u64,
    offset: usize,
    field: &'static str,
) -> Result<u64, BackendError> {
    let offset = u64::try_from(offset).map_err(|error| BackendError::InvalidProgram {
        fix: format!(
            "Fix: CUDA e-graph fused scratch {field} offset does not fit device pointer arithmetic: {error}."
        ),
    })?;
    base_ptr.checked_add(offset).ok_or_else(|| BackendError::InvalidProgram {
        fix: format!(
            "Fix: CUDA e-graph fused scratch {field} pointer overflowed u64 device address arithmetic."
        ),
    })
}

pub(crate) fn upload_u32_words(
    backend: &CudaBackend,
    words: &[u32],
) -> Result<CudaResidentBuffer, BackendError> {
    let byte_len = u32_word_bytes_len(words.len(), "u32 scratch words")?.max(4);
    let handle = backend.allocate_resident(byte_len)?;
    if let Err(error) = upload_u32_words_to_resident(backend, handle, words, byte_len) {
        let _ = backend.free_resident(handle);
        return Err(error);
    }
    Ok(handle)
}

fn upload_u32_words_to_resident(
    backend: &CudaBackend,
    handle: CudaResidentBuffer,
    words: &[u32],
    byte_len: usize,
) -> Result<(), BackendError> {
    const EMPTY_U32_UPLOAD: [u8; 4] = [0; 4];
    if words.is_empty() {
        return backend.upload_resident(handle, &EMPTY_U32_UPLOAD);
    }
    #[cfg(target_endian = "little")]
    {
        debug_assert_eq!(byte_len, words.len() * std::mem::size_of::<u32>());
        backend.upload_resident(handle, bytemuck::cast_slice(words))
    }
    #[cfg(not(target_endian = "little"))]
    {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(byte_len)
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA e-graph u32 scratch upload could not reserve {byte_len} host bytes before resident upload: {error}. Shard the metadata before launch."
                ),
            })?;
        bytes.resize(byte_len, 0);
        write_u32_words_at(&mut bytes, 0, words, "u32 scratch words")?;
        backend.upload_resident(handle, &bytes)
    }
}

fn upload_resident_bytes(
    backend: &CudaBackend,
    bytes: &[u8],
) -> Result<CudaResidentBuffer, BackendError> {
    let handle = backend.allocate_resident(bytes.len())?;
    if let Err(error) = backend.upload_resident(handle, bytes) {
        let _ = backend.free_resident(handle);
        return Err(error);
    }
    Ok(handle)
}

pub(crate) fn cleanup_egraph_kernel_handles(
    backend: &CudaBackend,
    handles: &[CudaResidentBuffer],
) -> Result<(), BackendError> {
    for &handle in handles.iter().rev() {
        backend.free_resident(handle)?;
    }
    Ok(())
}

pub(crate) fn read_u64_le(bytes: &[u8], context: &'static str) -> Result<u64, BackendError> {
    let chunk = bytes
        .get(0..8)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!("Fix: CUDA e-graph {context} readback returned fewer than 8 bytes."),
        })?;
    let mut raw = [0u8; 8];
    raw.copy_from_slice(chunk);
    Ok(u64::from_le_bytes(raw))
}

pub(crate) fn read_resident_u32_range(
    bytes: &[u8],
    count: usize,
    field: &'static str,
) -> Result<Vec<u32>, BackendError> {
    let byte_len = count
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph resident {field} range byte length overflowed host usize."
            ),
        })?;
    if bytes.len() != byte_len {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph resident {field} range returned {} bytes for {count} u32 values, expected {byte_len}.",
                bytes.len()
            ),
        });
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph resident {field} range snapshot could not reserve {count} u32 values: {error}."
            ),
        })?;
    for chunk in bytes.chunks_exact(4) {
        let mut raw = [0u8; 4];
        raw.copy_from_slice(chunk);
        values.push(u32::from_le_bytes(raw));
    }
    Ok(values)
}

pub(crate) fn decode_equivalence_pairs(
    bytes: &[u8],
    count: u64,
) -> Result<Vec<Equivalence>, BackendError> {
    let count = usize::try_from(count).map_err(|error| BackendError::InvalidProgram {
        fix: format!(
            "Fix: CUDA e-graph emitted equivalence count does not fit usize for readback decode: {error}."
        ),
    })?;
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(count)
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph emitted equivalence decode could not reserve {count} pair(s): {error}. Shard output readback before decode."
            ),
        })?;
    for index in 0..count {
        let byte_offset = index.checked_mul(8).ok_or_else(|| BackendError::InvalidProgram {
            fix: "Fix: CUDA e-graph equivalence pair byte offset overflowed while decoding readback."
                .to_string(),
        })?;
        let left = read_u32_le_at(bytes, byte_offset, "left equivalence eclass")?;
        let right = read_u32_le_at(bytes, byte_offset + 4, "right equivalence eclass")?;
        pairs.push(Equivalence { left, right });
    }
    Ok(pairs)
}

pub(crate) fn decode_unique_equivalence_pairs(
    bytes: &[u8],
    count: u64,
) -> Result<(u64, Vec<Equivalence>), BackendError> {
    let emitted_pair_count = count;
    let mut unique = decode_equivalence_pairs(bytes, count)?;
    sort_unstable_by_key_if_needed(&mut unique, |pair| (pair.left, pair.right));
    unique.dedup();
    Ok((emitted_pair_count, unique))
}

fn u32_word_bytes_len(word_count: usize, field: &'static str) -> Result<usize, BackendError> {
    word_count
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph fused scratch {field} byte length overflowed host usize addressing. Shard bucket metadata before launch."
            ),
        })
}

#[cfg(test)]
mod tests {
    use super::decode_unique_equivalence_pairs;
    use vyre_foundation::optimizer::eqsat_gpu::Equivalence;

    fn push_pair(bytes: &mut Vec<u8>, left: u32, right: u32) {
        bytes.extend_from_slice(&left.to_le_bytes());
        bytes.extend_from_slice(&right.to_le_bytes());
    }

    #[test]
    fn decode_unique_equivalence_pairs_compacts_in_place_for_generated_duplicates() {
        for seed in 0_u32..512 {
            let pair_count = 1 + (seed % 64);
            let mut bytes = Vec::new();
            let mut expected = Vec::new();
            for index in 0..pair_count {
                let left = (seed.wrapping_mul(31).wrapping_add(index)) % 97;
                let right = left.wrapping_add(1 + (index % 5));
                push_pair(&mut bytes, left, right);
                expected.push(Equivalence { left, right });
                if index % 3 == 0 {
                    push_pair(&mut bytes, left, right);
                }
            }
            expected.sort_by_key(|pair| (pair.left, pair.right));
            expected.dedup();

            let emitted_count = pair_count + ((pair_count + 2) / 3);
            let (reported, unique) =
                decode_unique_equivalence_pairs(&bytes, u64::from(emitted_count))
                    .expect("Fix: generated CUDA e-graph equivalence readback should decode");

            assert_eq!(reported, u64::from(emitted_count));
            assert!(unique
                .windows(2)
                .all(|pair| (pair[0].left, pair[0].right) < (pair[1].left, pair[1].right)));
            assert_eq!(unique, expected);
        }
    }
}

fn write_u32_words_at(
    bytes: &mut [u8],
    offset: usize,
    words: &[u32],
    field: &'static str,
) -> Result<(), BackendError> {
    let byte_len = u32_word_bytes_len(words.len(), field)?;
    let end = offset
        .checked_add(byte_len)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph fused scratch {field} write range overflowed host usize addressing."
            ),
        })?;
    let Some(dst) = bytes.get_mut(offset..end) else {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph fused scratch {field} write range [{offset}..{end}) exceeded the planned scratch slab."
            ),
        });
    };
    for (chunk, word) in dst.chunks_exact_mut(4).zip(words) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    Ok(())
}

fn read_u32_le_at(bytes: &[u8], offset: usize, context: &'static str) -> Result<u32, BackendError> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!("Fix: CUDA e-graph readback missing {context} at byte offset {offset}."),
        })?;
    let mut raw = [0u8; 4];
    raw.copy_from_slice(chunk);
    Ok(u32::from_le_bytes(raw))
}
