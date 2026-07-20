//! Exact-input materialized output cache for compiled CUDA graph replay.
//!
//! This cache is deliberately host-side: it avoids redundant cudaGraph launches
//! when a compiled pipeline receives an identical batch again, including cases
//! where the batch is larger than the concurrent graph-lane pool. Entries carry
//! a BLAKE3 key for fixed-width hot-path lookup and retain owned input bytes so
//! key matches are still collision-checked before outputs are reused.

use std::sync::Arc;

use smallvec::SmallVec;
use vyre_driver::accounting::checked_add_usize_lazy;
use vyre_driver::{BackendError, OutputBuffers};

pub(crate) use crate::input_identity::{
    exact_input_key as materialized_input_key, ExactInputKey as MaterializedInputKey,
};

use super::MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE;

pub(crate) const MAX_MATERIALIZED_OUTPUT_CACHE_BYTES_PER_PIPELINE: usize = 8 * 1024 * 1024;

#[derive(Debug, Default)]
pub(crate) struct MaterializedPipelineOutputCache {
    entries: SmallVec<[MaterializedPipelineOutputCacheEntry; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE]>,
    byte_len: usize,
}

impl MaterializedPipelineOutputCache {
    pub(crate) fn hit_into(
        &self,
        inputs: &[&[u8]],
        outputs: &mut OutputBuffers,
    ) -> Result<bool, BackendError> {
        let Some(snapshot) = self.snapshot(inputs)? else {
            return Ok(false);
        };
        snapshot.copy_into(outputs)?;
        Ok(true)
    }

    pub(crate) fn snapshot(
        &self,
        inputs: &[&[u8]],
    ) -> Result<Option<MaterializedOutputSnapshot>, BackendError> {
        let input_key = materialized_input_key(inputs)?;
        Ok(self.snapshot_with_key(inputs, &input_key))
    }

    pub(crate) fn snapshot_with_key(
        &self,
        inputs: &[&[u8]],
        input_key: &MaterializedInputKey,
    ) -> Option<MaterializedOutputSnapshot> {
        for entry in self.entries.iter().rev() {
            if entry.input_key() == input_key && entry.matches_inputs(inputs) {
                return Some(entry.snapshot());
            }
        }
        None
    }

    pub(crate) fn remember(
        &mut self,
        inputs: &[&[u8]],
        outputs: &OutputBuffers,
    ) -> Result<(), BackendError> {
        let Some(entry) = MaterializedPipelineOutputCacheEntry::new_if_cacheable(inputs, outputs)?
        else {
            return Ok(());
        };
        self.remember_entry(entry)
    }

    pub(crate) fn remember_entry(
        &mut self,
        entry: MaterializedPipelineOutputCacheEntry,
    ) -> Result<(), BackendError> {
        let entry_byte_len = entry.byte_len();
        if entry_byte_len > MAX_MATERIALIZED_OUTPUT_CACHE_BYTES_PER_PIPELINE {
            return Ok(());
        }
        if let Some(index) = self.entries.iter().position(|cached| {
            cached.input_key() == entry.input_key() && cached.matches_owned_inputs(&entry.inputs)
        }) {
            self.remove_accounted(index)?;
        }
        while !self.entries.is_empty()
            && (self.entries.len() >= MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE
                || add_materialized_cache_bytes(self.byte_len, entry_byte_len)?
                    > MAX_MATERIALIZED_OUTPUT_CACHE_BYTES_PER_PIPELINE)
        {
            self.remove_accounted(0)?;
        }
        self.entries.push(entry);
        self.byte_len = add_materialized_cache_bytes(self.byte_len, entry_byte_len)?;
        Ok(())
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn byte_len(&self) -> usize {
        self.byte_len
    }

    fn remove_accounted(
        &mut self,
        index: usize,
    ) -> Result<MaterializedPipelineOutputCacheEntry, BackendError> {
        let removed = self.entries.remove(index);
        self.byte_len = self
            .byte_len
            .checked_sub(removed.byte_len())
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA compiled-pipeline materialized output cache byte accounting underflowed; rebuild the compiled pipeline cache.".to_string(),
            })?;
        Ok(removed)
    }
}

#[derive(Debug)]
pub(crate) struct MaterializedPipelineOutputCacheEntry {
    input_key: MaterializedInputKey,
    inputs: SmallVec<[Vec<u8>; 4]>,
    outputs: Arc<[Vec<u8>]>,
    byte_len: usize,
}

impl MaterializedPipelineOutputCacheEntry {
    pub(crate) fn new_if_cacheable(
        inputs: &[&[u8]],
        outputs: &[Vec<u8>],
    ) -> Result<Option<Self>, BackendError> {
        let input_key = materialized_input_key(inputs)?;
        let Some(byte_len) = materialized_cache_entry_byte_len_if_admissible(inputs, outputs)?
        else {
            return Ok(None);
        };
        Ok(Some(Self::new_with_key_and_byte_len(
            inputs, input_key, outputs, byte_len,
        )?))
    }

    pub(crate) fn new_with_key_if_cacheable(
        inputs: &[&[u8]],
        input_key: &MaterializedInputKey,
        outputs: &[Vec<u8>],
    ) -> Result<Option<Self>, BackendError> {
        let Some(byte_len) = materialized_cache_entry_byte_len_if_admissible(inputs, outputs)?
        else {
            return Ok(None);
        };
        Ok(Some(Self::new_with_key_and_byte_len(
            inputs, *input_key, outputs, byte_len,
        )?))
    }

    pub(crate) fn new(inputs: &[&[u8]], outputs: &[Vec<u8>]) -> Result<Self, BackendError> {
        let input_key = materialized_input_key(inputs)?;
        Self::new_with_key(inputs, &input_key, outputs)
    }

    pub(crate) fn new_with_key(
        inputs: &[&[u8]],
        input_key: &MaterializedInputKey,
        outputs: &[Vec<u8>],
    ) -> Result<Self, BackendError> {
        let byte_len = materialized_cache_entry_byte_len(inputs, outputs)?;
        Self::new_with_key_and_byte_len(inputs, *input_key, outputs, byte_len)
    }

    fn new_with_key_and_byte_len(
        inputs: &[&[u8]],
        input_key: MaterializedInputKey,
        outputs: &[Vec<u8>],
        byte_len: usize,
    ) -> Result<Self, BackendError> {
        let mut owned_inputs = SmallVec::<[Vec<u8>; 4]>::new();
        owned_inputs
            .try_reserve(inputs.len())
            .map_err(|error| materialized_cache_allocation_failed("input slots", error))?;
        for input in inputs {
            owned_inputs.push(clone_materialized_cache_bytes(input, "input bytes")?);
        }

        let mut owned_outputs = Vec::new();
        owned_outputs
            .try_reserve(outputs.len())
            .map_err(|error| materialized_cache_allocation_failed("output slots", error))?;
        for output in outputs {
            owned_outputs.push(clone_materialized_cache_bytes(output, "output bytes")?);
        }

        Ok(Self {
            input_key,
            inputs: owned_inputs,
            outputs: Arc::from(owned_outputs),
            byte_len,
        })
    }

    pub(crate) fn input_key(&self) -> &MaterializedInputKey {
        &self.input_key
    }

    pub(crate) fn matches_inputs(&self, inputs: &[&[u8]]) -> bool {
        self.inputs.len() == inputs.len()
            && self
                .inputs
                .iter()
                .zip(inputs.iter())
                .all(|(cached, input)| cached.as_slice() == *input)
    }

    fn matches_owned_inputs(&self, inputs: &[Vec<u8>]) -> bool {
        self.inputs.len() == inputs.len()
            && self
                .inputs
                .iter()
                .zip(inputs.iter())
                .all(|(cached, input)| cached.as_slice() == input.as_slice())
    }

    pub(crate) fn snapshot(&self) -> MaterializedOutputSnapshot {
        MaterializedOutputSnapshot {
            outputs: Arc::clone(&self.outputs),
        }
    }

    pub(crate) fn byte_len(&self) -> usize {
        self.byte_len
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MaterializedOutputSnapshot {
    outputs: Arc<[Vec<u8>]>,
}

impl MaterializedOutputSnapshot {
    pub(crate) fn copy_into(&self, dst: &mut OutputBuffers) -> Result<(), BackendError> {
        copy_materialized_outputs_into(&self.outputs, dst)
    }
}

fn copy_materialized_outputs_into(
    outputs: &[Vec<u8>],
    dst: &mut OutputBuffers,
) -> Result<(), BackendError> {
    let existing_slots_to_copy = dst.len().min(outputs.len());
    if dst.len() < outputs.len() {
        dst.try_reserve(outputs.len() - dst.len())
            .map_err(|error| {
                materialized_cache_allocation_failed("output destination slots", error)
            })?;
    }

    for (target, source) in dst
        .iter_mut()
        .take(existing_slots_to_copy)
        .zip(outputs.iter())
    {
        if source.len() > target.capacity() {
            target
                .try_reserve_exact(source.len() - target.capacity())
                .map_err(|error| {
                    materialized_cache_allocation_failed("output destination bytes", error)
                })?;
        }
    }

    let mut appended_outputs = Vec::new();
    if outputs.len() > dst.len() {
        appended_outputs
            .try_reserve(outputs.len() - dst.len())
            .map_err(|error| {
                materialized_cache_allocation_failed("new output destination slots", error)
            })?;
        for source in outputs.iter().skip(dst.len()) {
            appended_outputs.push(clone_materialized_cache_bytes(
                source,
                "new output destination bytes",
            )?);
        }
    }

    dst.truncate(outputs.len());
    for (target, source) in dst.iter_mut().zip(outputs.iter()) {
        target.clear();
        target.extend_from_slice(source);
    }
    dst.extend(appended_outputs);
    Ok(())
}

fn clone_materialized_cache_bytes(
    bytes: &[u8],
    label: &'static str,
) -> Result<Vec<u8>, BackendError> {
    let mut cloned = Vec::new();
    cloned
        .try_reserve(bytes.len())
        .map_err(|error| materialized_cache_allocation_failed(label, error))?;
    cloned.extend_from_slice(bytes);
    Ok(cloned)
}

fn materialized_cache_entry_byte_len(
    inputs: &[&[u8]],
    outputs: &[Vec<u8>],
) -> Result<usize, BackendError> {
    let mut byte_len = 0usize;
    for input in inputs {
        byte_len = add_materialized_cache_bytes(byte_len, input.len())?;
    }
    for output in outputs {
        byte_len = add_materialized_cache_bytes(byte_len, output.len())?;
    }
    Ok(byte_len)
}

fn materialized_cache_entry_byte_len_if_admissible(
    inputs: &[&[u8]],
    outputs: &[Vec<u8>],
) -> Result<Option<usize>, BackendError> {
    let mut byte_len = 0usize;
    for input in inputs {
        byte_len = add_materialized_cache_bytes(byte_len, input.len())?;
        if byte_len > MAX_MATERIALIZED_OUTPUT_CACHE_BYTES_PER_PIPELINE {
            return Ok(None);
        }
    }
    for output in outputs {
        byte_len = add_materialized_cache_bytes(byte_len, output.len())?;
        if byte_len > MAX_MATERIALIZED_OUTPUT_CACHE_BYTES_PER_PIPELINE {
            return Ok(None);
        }
    }
    Ok(Some(byte_len))
}

fn add_materialized_cache_bytes(total: usize, next: usize) -> Result<usize, BackendError> {
    checked_add_usize_lazy(total, next, || {
        BackendError::InvalidProgram {
        fix: "Fix: CUDA compiled-pipeline materialized output cache byte accounting overflowed; split the batch or disable graph replay for this shape.".to_string(),
    }
    })
}

pub(crate) fn materialized_cache_allocation_failed<E: std::fmt::Debug>(
    label: &'static str,
    error: E,
) -> BackendError {
    BackendError::DispatchFailed {
        code: None,
        message: format!(
            "CUDA compiled-pipeline materialized output cache could not reserve {label}: {error:?}. Fix: reduce batch size or disable graph replay for oversized outputs."
        ),
    }
}
