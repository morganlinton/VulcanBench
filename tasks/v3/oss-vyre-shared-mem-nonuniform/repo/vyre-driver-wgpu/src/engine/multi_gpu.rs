//! Multi-GPU adapter probing, backend acquisition, and work partitioning.
//!
//! The pure partitioner stays separately testable, but production callers can
//! now derive device loads from live wgpu adapters and acquire one backend per
//! selected adapter instead of stopping at scheduling math.
//!
//! ## Two allocation modes
//!
//! 1. **Batch + cost-aware** (`partition_work_stealing`): caller
//!    knows every work item + its cost up front; LPT greedy assigns
//!    the heaviest item to the least-loaded device.
//! 2. **Stream + content-addressed**
//!    (`shard_by_blake3` + `StreamShardAllocator`): caller yields
//!    `(key, cost)` pairs one at a time from a walker. The initial
//!    device is `blake3(key)[0] % n_gpus` for deterministic
//!    affinity  -  files with the same path always land on the same
//!    GPU across runs, which enables cache-warm re-scans. Overflow
//!    (queue on the target GPU is already loaded above threshold)
//!    spills to the least-loaded neighbor to keep tail latency
//!    bounded.

mod partition;
mod stream_shard;

use crate::staging_reserve::{reserve_multi_gpu_vec, reserve_smallvec, reserve_vec};

pub use partition::{partition_work_stealing, DeviceLoad, Partition, WeightedWorkItem};
pub use stream_shard::{shard_by_blake3, StreamShardAllocator};

fn empty_gpu_work_result_slots(
    len: usize,
) -> Result<Vec<Option<Result<GpuWorkOutput, vyre_driver::BackendError>>>, vyre_driver::BackendError>
{
    let mut slots = Vec::new();
    reserve_vec(
        &mut slots,
        len,
        "multi-GPU executor",
        "borrowed result slot",
        "split the multi-GPU batch before dispatch",
    )?;
    slots.resize_with(len, || None);
    Ok(slots)
}

fn finalize_gpu_work_results(
    slots: Vec<Option<Result<GpuWorkOutput, vyre_driver::BackendError>>>,
) -> Result<Vec<Result<GpuWorkOutput, vyre_driver::BackendError>>, vyre_driver::BackendError> {
    let mut results = Vec::new();
    reserve_vec(
        &mut results,
        slots.len(),
        "multi-GPU executor",
        "final borrowed result",
        "split the multi-GPU batch before dispatch",
    )?;
    for slot in slots {
        results.push(slot.unwrap_or_else(|| {
            Err(vyre_driver::BackendError::new(
                "multi-GPU borrowed dispatch result slot was not filled. Fix: ensure partitioning assigns every job exactly once.",
            ))
        }));
    }
    Ok(results)
}

/// Enumerate live wgpu GPU adapters as zero-load scheduling targets.
///
/// # Errors
///
/// Returns an error when wgpu exposes no real GPU adapters. On supported
/// production hosts that means adapter probing or driver setup is broken.
pub fn live_gpu_loads() -> Result<Vec<DeviceLoad>, String> {
    let adapters = crate::runtime::device::enumerate_adapters();
    let mut loads = Vec::new();
    vyre_driver::allocation::try_reserve_vec_to_capacity(&mut loads, adapters.len()).map_err(
        |source| {
            format!(
                "live GPU load enumeration could not reserve {} adapter slot(s): {source}. Fix: reduce adapter fanout or repair driver memory pressure before scheduling.",
                adapters.len()
            )
        },
    )?;
    loads.extend(
        adapters
            .iter()
            .enumerate()
            .filter_map(|(device_index, info)| {
                crate::capabilities::is_real_gpu(info).then_some(DeviceLoad {
                    device_index,
                    queued_cost: 0,
                })
            }),
    );
    if loads.is_empty() {
        return Err(format!(
            "wgpu enumerated {} adapters but none were real GPU execution targets. Fix: inspect driver setup and adapter filtering.",
            adapters.len()
        ));
    }
    Ok(loads)
}

/// A live GPU selected for multi-device scheduling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveGpu {
    /// Adapter index accepted by `runtime::device::init_device_for_adapter`.
    pub adapter_index: usize,
    /// Stable adapter information captured during enumeration.
    pub info: wgpu::AdapterInfo,
}

/// One executable multi-GPU work packet.
pub struct GpuWorkItem {
    /// Stable work identifier returned with the output.
    pub id: usize,
    /// Relative scheduling cost.
    pub cost: u64,
    /// Program to compile and dispatch on the selected adapter.
    pub program: vyre_foundation::ir::Program,
    /// Input buffers for the dispatch.
    pub inputs: Vec<Vec<u8>>,
    /// Dispatch policy for this item.
    pub config: vyre_driver::DispatchConfig,
}

/// Output from one dispatched [`GpuWorkItem`].
#[derive(Debug)]
pub struct GpuWorkOutput {
    /// Work identifier.
    pub id: usize,
    /// Adapter index that executed the work.
    pub adapter_index: usize,
    /// Dispatch outputs.
    pub outputs: Vec<Vec<u8>>,
}

/// Borrowed multi-GPU work packet for hot scan paths.
pub struct BorrowedGpuWorkItem<'a> {
    /// Stable work identifier returned with the output.
    pub id: usize,
    /// Relative scheduling cost.
    pub cost: u64,
    /// Program to compile and dispatch on the selected adapter.
    pub program: &'a vyre_foundation::ir::Program,
    /// Input buffers for the dispatch.
    pub inputs: &'a [&'a [u8]],
    /// Dispatch policy for this item.
    pub config: &'a vyre_driver::DispatchConfig,
}

/// Real multi-GPU executor backed by one [`crate::WgpuBackend`] per adapter.
pub struct MultiGpuExecutor {
    devices: Vec<ExecutorDevice>,
}

struct ExecutorDevice {
    adapter_index: usize,
    backend: crate::WgpuBackend,
    queued_cost: u64,
}

impl MultiGpuExecutor {
    /// Enumerate real GPU adapters visible to wgpu.
    #[must_use]
    pub fn enumerate_live_gpus() -> Vec<LiveGpu> {
        let adapters = crate::runtime::device::enumerate_adapters();
        let mut live = Vec::new();
        let _ = vyre_driver::allocation::try_reserve_vec_to_capacity(&mut live, adapters.len());
        for (adapter_index, info) in adapters.into_iter().enumerate() {
            if crate::capabilities::is_real_gpu(&info) {
                live.push(LiveGpu {
                    adapter_index,
                    info,
                });
            }
        }
        live
    }

    /// Build an executor over every real adapter that can create a device.
    ///
    /// # Errors
    ///
    /// Returns a backend error when no real adapters are visible or when a
    /// visible adapter fails device creation.
    pub fn acquire_all() -> Result<Self, vyre_driver::BackendError> {
        let live = Self::enumerate_live_gpus();
        if live.is_empty() {
            return Err(vyre_driver::BackendError::new(
                "no real GPU adapters found for multi-GPU execution. Fix: expose at least one discrete, integrated, or virtual GPU through wgpu before calling MultiGpuExecutor::acquire_all.",
            ));
        }
        let mut devices = Vec::new();
        reserve_multi_gpu_vec(&mut devices, live.len(), "executor device")?;
        for gpu in live {
            let backend = crate::WgpuBackend::acquire_adapter(gpu.adapter_index)?;
            devices.push(ExecutorDevice {
                adapter_index: gpu.adapter_index,
                backend,
                queued_cost: 0,
            });
        }
        Ok(Self { devices })
    }

    /// Build an executor over explicit adapter indices.
    ///
    /// # Errors
    ///
    /// Returns a backend error if the list is empty, duplicated, or contains an
    /// adapter that cannot create a real GPU device.
    pub fn acquire_indices(indices: &[usize]) -> Result<Self, vyre_driver::BackendError> {
        if indices.is_empty() {
            return Err(vyre_driver::BackendError::new(
                "no adapter indices supplied for multi-GPU execution. Fix: pass indices returned by runtime::device::enumerate_adapters().",
            ));
        }
        let mut seen = rustc_hash::FxHashSet::default();
        vyre_foundation::allocation::try_reserve_hash_set_to_capacity(&mut seen, indices.len())
            .map_err(|source| {
                vyre_driver::BackendError::new(format!(
                    "multi-GPU adapter-index validation could not reserve {} seen slot(s): {source}. Fix: reduce adapter fanout before acquisition.",
                    indices.len()
                ))
            })?;
        let mut devices = Vec::new();
        reserve_multi_gpu_vec(&mut devices, indices.len(), "executor device")?;
        for &index in indices {
            if !seen.insert(index) {
                return Err(vyre_driver::BackendError::new(format!(
                    "duplicate adapter index {index} supplied for multi-GPU execution. Fix: pass each adapter once."
                )));
            }
            let backend = crate::WgpuBackend::acquire_adapter(index)?;
            devices.push(ExecutorDevice {
                adapter_index: index,
                backend,
                queued_cost: 0,
            });
        }
        Ok(Self { devices })
    }

    /// Number of live device backends owned by this executor.
    #[must_use]
    pub fn len(&self) -> usize {
        self.devices.len()
    }

    /// Whether the executor owns no devices.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    /// Adapter indices owned by this executor.
    #[must_use]
    pub fn adapter_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();
        let _ =
            vyre_driver::allocation::try_reserve_vec_to_capacity(&mut indices, self.devices.len());
        indices.extend(self.devices.iter().map(|device| device.adapter_index));
        indices
    }

    /// Dispatch a batch across the selected adapters using the LPT partitioner.
    ///
    /// Dispatches are submitted from one host thread per selected adapter. wgpu
    /// devices are independent, so separate physical adapters can compile,
    /// submit, and read back concurrently.
    pub fn dispatch_batch(
        &mut self,
        items: Vec<GpuWorkItem>,
    ) -> Result<Vec<GpuWorkOutput>, vyre_driver::BackendError> {
        let mut devices = smallvec::SmallVec::<[DeviceLoad; 8]>::new();
        reserve_smallvec(
            &mut devices,
            self.devices.len(),
            "multi-GPU executor",
            "device-load descriptor",
            "split the multi-GPU batch before dispatch",
        )?;
        devices.extend(self.devices.iter().map(|device| DeviceLoad {
            device_index: device.adapter_index,
            queued_cost: device.queued_cost,
        }));
        let mut work = smallvec::SmallVec::<[WeightedWorkItem; 32]>::new();
        reserve_smallvec(
            &mut work,
            items.len(),
            "multi-GPU executor",
            "weighted work descriptor",
            "split the multi-GPU batch before partitioning",
        )?;
        work.extend(items.iter().map(|item| WeightedWorkItem {
            id: item.id,
            cost: item.cost,
        }));
        let partitions =
            partition_work_stealing(&devices, &work).map_err(vyre_driver::BackendError::new)?;
        let mut by_id = rustc_hash::FxHashMap::default();
        vyre_foundation::allocation::try_reserve_hash_map_to_capacity(&mut by_id, items.len())
            .map_err(|source| {
                vyre_driver::BackendError::new(format!(
                    "multi-GPU work-item lookup could not reserve {} owned item slot(s): {source}. Fix: split the multi-GPU batch.",
                    items.len()
                ))
            })?;
        by_id.extend(items.into_iter().map(|item| (item.id, item)));
        let mut outputs = Vec::new();
        reserve_multi_gpu_vec(&mut outputs, by_id.len(), "owned output")?;
        std::thread::scope(|scope| {
            let mut handles = smallvec::SmallVec::<[_; 8]>::new();
            reserve_smallvec(
                &mut handles,
                partitions.len(),
                "multi-GPU executor",
                "worker thread handle",
                "split the multi-GPU batch before dispatch",
            )?;
            for (partition, device) in partitions.into_iter().zip(self.devices.iter_mut()) {
                if device.adapter_index != partition.device_index {
                    return Err(vyre_driver::BackendError::new(format!(
                        "partition targeted missing adapter {}. Fix: keep partition device indices synchronized with executor devices.",
                        partition.device_index
                    )));
                }
                device.queued_cost = partition.total_cost;
                let backend = device.backend.clone();
                let adapter_index = device.adapter_index;
                let mut assigned = smallvec::SmallVec::<[_; 8]>::new();
                reserve_smallvec(
                    &mut assigned,
                    partition.item_ids.len(),
                    "multi-GPU executor",
                    "assigned owned work item",
                    "split the multi-GPU batch before dispatch",
                )?;
                for id in partition.item_ids {
                    let item = by_id.remove(&id).ok_or_else(|| {
                        vyre_driver::BackendError::new(format!(
                            "partition referenced unknown work item {id}. Fix: partition only ids from the submitted batch."
                        ))
                    })?;
                    assigned.push(item);
                }
                handles.push(scope.spawn(move || {
                    let mut local = Vec::new();
                    reserve_multi_gpu_vec(&mut local, assigned.len(), "worker-local output")?;
                    for item in assigned {
                        let outputs = vyre_driver::VyreBackend::dispatch(
                            &backend,
                            &item.program,
                            &item.inputs,
                            &item.config,
                        )?;
                        local.push(GpuWorkOutput {
                            id: item.id,
                            adapter_index,
                            outputs,
                        });
                    }
                    Ok::<_, vyre_driver::BackendError>(local)
                }));
            }
            for handle in handles {
                let mut local = handle.join().map_err(|_| {
                    vyre_driver::BackendError::new(
                        "multi-GPU worker thread panicked. Fix: inspect adapter-specific dispatch failure handling.",
                    )
                })??;
                outputs.append(&mut local);
            }
            Ok::<_, vyre_driver::BackendError>(())
        })?;
        if !by_id.is_empty() {
            return Err(vyre_driver::BackendError::new(
                "multi-GPU partition left unassigned work items. Fix: partition every submitted item exactly once.",
            ));
        }
        outputs.sort_by_key(|output| output.id);
        Ok(outputs)
    }

    /// Dispatch a borrowed batch across selected adapters without cloning
    /// input buffers into owned work packets.
    ///
    /// Each adapter receives one backend-local batched submission, so wgpu's
    /// per-device command buffers stay valid while independent adapters run on
    /// separate host threads.
    pub fn dispatch_borrowed_batch(
        &mut self,
        items: &[BorrowedGpuWorkItem<'_>],
    ) -> Result<Vec<Result<GpuWorkOutput, vyre_driver::BackendError>>, vyre_driver::BackendError>
    {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let mut devices = smallvec::SmallVec::<[DeviceLoad; 8]>::new();
        reserve_smallvec(
            &mut devices,
            self.devices.len(),
            "multi-GPU executor",
            "borrowed device-load descriptor",
            "split the multi-GPU batch before dispatch",
        )?;
        devices.extend(self.devices.iter().map(|device| DeviceLoad {
            device_index: device.adapter_index,
            queued_cost: device.queued_cost,
        }));
        let mut work = smallvec::SmallVec::<[WeightedWorkItem; 32]>::new();
        reserve_smallvec(
            &mut work,
            items.len(),
            "multi-GPU executor",
            "borrowed weighted work descriptor",
            "split the multi-GPU batch before partitioning",
        )?;
        work.extend(items.iter().map(|item| WeightedWorkItem {
            id: item.id,
            cost: item.cost,
        }));
        let partitions =
            partition_work_stealing(&devices, &work).map_err(vyre_driver::BackendError::new)?;
        let mut by_id = rustc_hash::FxHashMap::default();
        vyre_foundation::allocation::try_reserve_hash_map_to_capacity(&mut by_id, items.len())
            .map_err(|source| {
                vyre_driver::BackendError::new(format!(
                    "multi-GPU work-item lookup could not reserve {} borrowed item slot(s): {source}. Fix: split the multi-GPU batch.",
                    items.len()
                ))
            })?;
        by_id.extend(items.iter().enumerate().map(|(slot, item)| (item.id, slot)));
        let mut results = empty_gpu_work_result_slots(items.len())?;

        std::thread::scope(|scope| {
            let mut handles = smallvec::SmallVec::<[_; 8]>::new();
            reserve_smallvec(
                &mut handles,
                partitions.len(),
                "multi-GPU executor",
                "borrowed worker thread handle",
                "split the multi-GPU batch before dispatch",
            )?;
            for (partition, device) in partitions.into_iter().zip(self.devices.iter_mut()) {
                if device.adapter_index != partition.device_index {
                    return Err(vyre_driver::BackendError::new(format!(
                        "partition targeted missing adapter {}. Fix: keep partition device indices synchronized with executor devices.",
                        partition.device_index
                    )));
                }
                device.queued_cost = partition.total_cost;
                let backend = device.backend.clone();
                let adapter_index = device.adapter_index;
                let mut assigned_slots = smallvec::SmallVec::<[_; 8]>::new();
                reserve_smallvec(
                    &mut assigned_slots,
                    partition.item_ids.len(),
                    "multi-GPU executor",
                    "assigned borrowed work slot",
                    "split the multi-GPU batch before dispatch",
                )?;
                for id in partition.item_ids {
                    let slot = by_id.remove(&id).ok_or_else(|| {
                        vyre_driver::BackendError::new(format!(
                            "partition referenced unknown work item {id}. Fix: partition only ids from the submitted batch."
                        ))
                    })?;
                    assigned_slots.push(slot);
                }
                handles.push(scope.spawn(move || {
                    let mut backend_jobs = smallvec::SmallVec::<
                        [(
                            &vyre_foundation::ir::Program,
                            &[&[u8]],
                            &vyre_driver::DispatchConfig,
                        ); 8],
                    >::new();
                    reserve_smallvec(
                        &mut backend_jobs,
                        assigned_slots.len(),
                        "multi-GPU executor",
                        "backend-local borrowed job descriptor",
                        "split the multi-GPU batch before dispatch",
                    )?;
                    backend_jobs.extend(assigned_slots.iter().map(|&slot| {
                        let item = &items[slot];
                        (item.program, item.inputs, item.config)
                    }));
                    let local = backend.dispatch_borrowed_batch(&backend_jobs)?;
                    Ok::<_, vyre_driver::BackendError>((adapter_index, assigned_slots, local))
                }));
            }
            for handle in handles {
                let (adapter_index, assigned_slots, local) = handle.join().map_err(|_| {
                    vyre_driver::BackendError::new(
                        "multi-GPU borrowed worker thread panicked. Fix: inspect adapter-specific dispatch failure handling.",
                    )
                })??;
                if assigned_slots.len() != local.len() {
                    return Err(vyre_driver::BackendError::new(format!(
                        "adapter {adapter_index} returned {} results for {} assigned jobs. Fix: keep backend batch metadata synchronized.",
                        local.len(),
                        assigned_slots.len()
                    )));
                }
                for (slot, output_result) in assigned_slots.into_iter().zip(local) {
                    let id = items[slot].id;
                    results[slot] = Some(output_result.map(|outputs| GpuWorkOutput {
                        id,
                        adapter_index,
                        outputs,
                    }));
                }
            }
            Ok::<_, vyre_driver::BackendError>(())
        })?;
        if !by_id.is_empty() {
            return Err(vyre_driver::BackendError::new(
                "multi-GPU borrowed partition left unassigned work items. Fix: partition every submitted item exactly once.",
            ));
        }

        finalize_gpu_work_results(results)
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn live_gpu_enumeration_uses_wgpu_adapters() {
        let live = MultiGpuExecutor::enumerate_live_gpus();
        assert!(
            !live.is_empty(),
            "Fix: multi-GPU runtime must enumerate at least one real GPU adapter on this fleet host."
        );
        for gpu in live {
            assert!(
                crate::capabilities::is_real_gpu(&gpu.info),
                "Fix: multi-GPU executor must filter CPU/Other adapters before scheduling: {:?}",
                gpu.info
            );
        }
    }

    #[test]
    fn acquire_indices_rejects_duplicate_live_ordinals_before_dispatch() {
        let live = MultiGpuExecutor::enumerate_live_gpus();
        let first = live
            .first()
            .expect("Fix: duplicate-index test requires the live GPU adapter promised by the fleet")
            .adapter_index;
        let error = match MultiGpuExecutor::acquire_indices(&[first, first]) {
            Ok(_) => panic!("Fix: duplicate adapter indices must be rejected"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("duplicate adapter index"),
            "Fix: duplicate-index diagnostic must be actionable, got: {error}"
        );
    }

    #[test]
    fn generated_borrowed_result_finalization_preserves_slots_and_reports_missing_work() {
        for case in 0..4096usize {
            let len = (case % 17) + 1;
            let mut slots = empty_gpu_work_result_slots(len)
                .expect("Fix: generated multi-GPU slot test must reserve result slots");
            for slot in 0..len {
                if (slot + case) % 5 == 0 {
                    continue;
                }
                slots[slot] = Some(Ok(GpuWorkOutput {
                    id: slot,
                    adapter_index: case % 3,
                    outputs: vec![vec![slot as u8, case as u8]],
                }));
            }

            let finalized = finalize_gpu_work_results(slots)
                .expect("Fix: generated multi-GPU finalization must reserve final results");
            assert_eq!(
                finalized.len(),
                len,
                "generated multi-GPU case {case} must preserve slot count"
            );
            for (slot, result) in finalized.into_iter().enumerate() {
                if (slot + case) % 5 == 0 {
                    let error = result
                        .expect_err("Fix: unfilled generated multi-GPU slot must be an error");
                    assert!(
                        error.to_string().contains("result slot was not filled"),
                        "Fix: missing generated multi-GPU slot must explain partition coverage, got {error}"
                    );
                } else {
                    let output =
                        result.expect("Fix: filled generated multi-GPU slot must stay successful");
                    assert_eq!(output.id, slot);
                    assert_eq!(output.adapter_index, case % 3);
                    assert_eq!(output.outputs, vec![vec![slot as u8, case as u8]]);
                }
            }
        }
    }
}
