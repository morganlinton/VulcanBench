use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// One pending unit of GPU work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeightedWorkItem {
    /// Stable work identifier used by callers to map results back.
    pub id: usize,
    /// Relative cost estimate. Zero-cost work is rejected because it cannot
    /// contribute to a meaningful load balance.
    pub cost: u64,
}

/// Current device load snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceLoad {
    /// Device ordinal in the caller's adapter list.
    pub device_index: usize,
    /// Cost already queued on the device before this partitioning pass.
    pub queued_cost: u64,
}

/// Work assigned to one device.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Partition {
    /// Device ordinal receiving this partition.
    pub device_index: usize,
    /// Work item identifiers assigned to the device.
    pub item_ids: Vec<usize>,
    /// Total assigned cost including pre-existing queued cost.
    pub total_cost: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PartitionHeapEntry {
    total_cost: u64,
    device_index: usize,
    partition_index: usize,
}

impl Ord for PartitionHeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.total_cost
            .cmp(&other.total_cost)
            .then_with(|| self.device_index.cmp(&other.device_index))
            .then_with(|| self.partition_index.cmp(&other.partition_index))
    }
}

impl PartialOrd for PartitionHeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Partition work by repeatedly assigning the largest remaining item to the
/// least-loaded device.
///
/// # Errors
///
/// Returns an actionable error when no devices are available, duplicate device
/// ordinals are supplied, or a work item has zero cost.
pub fn partition_work_stealing(
    devices: &[DeviceLoad],
    items: &[WeightedWorkItem],
) -> Result<Vec<Partition>, String> {
    validate_inputs(devices, items)?;
    let mut partitions = Vec::new();
    vyre_driver::allocation::try_reserve_vec_to_capacity(&mut partitions, devices.len()).map_err(|error| {
        format!(
            "partition table allocation failed for {} GPU devices: {error}. Fix: lower the adapter fanout or memory pressure before scheduling.",
            devices.len()
        )
    })?;
    let mut least_loaded = BinaryHeap::new();
    vyre_foundation::allocation::try_reserve_binary_heap_to_capacity(
        &mut least_loaded,
        devices.len(),
    )
    .map_err(|error| {
        format!(
            "partition heap allocation failed for {} GPU devices: {error}. Fix: lower the adapter fanout or memory pressure before scheduling.",
            devices.len()
        )
    })?;
    let target_item_capacity = items.len().div_ceil(devices.len());
    for device in devices {
        let partition_index = partitions.len();
        let mut item_ids = Vec::new();
        vyre_driver::allocation::try_reserve_vec_to_capacity(&mut item_ids, target_item_capacity).map_err(|error| {
            format!(
                "partition assignment allocation failed for GPU device {} and {} target work slots: {error}. Fix: split the multi-GPU batch.",
                device.device_index, target_item_capacity
            )
        })?;
        partitions.push(Partition {
            device_index: device.device_index,
            item_ids,
            total_cost: device.queued_cost,
        });
        least_loaded.push(Reverse(PartitionHeapEntry {
            total_cost: device.queued_cost,
            device_index: device.device_index,
            partition_index,
        }));
    }

    let mut ordered = Vec::new();
    vyre_driver::allocation::try_reserve_vec_to_capacity(&mut ordered, items.len()).map_err(|error| {
        format!(
            "partition work-order allocation failed for {} items: {error}. Fix: split the multi-GPU batch.",
            items.len()
        )
    })?;
    ordered.extend(items.iter());
    ordered.sort_by(|left, right| {
        right
            .cost
            .cmp(&left.cost)
            .then_with(|| left.id.cmp(&right.id))
    });

    for item in ordered {
        let Some(mut target) = least_loaded.pop().map(|entry| entry.0) else {
            return Err(
                "partition target not found. Fix: validate non-empty device list before partitioning."
                    .to_string()
            );
        };
        let partition = &mut partitions[target.partition_index];
        ensure_vec_spare(&mut partition.item_ids, 1, "partition assignment list")?;
        partition.item_ids.push(item.id);
        partition.total_cost = partition.total_cost.checked_add(item.cost).ok_or_else(|| {
            "partition cost overflow. Fix: split the batch before multi-GPU scheduling.".to_string()
        })?;
        target.total_cost = partition.total_cost;
        least_loaded.push(Reverse(target));
    }
    Ok(partitions)
}

fn validate_inputs(devices: &[DeviceLoad], items: &[WeightedWorkItem]) -> Result<(), String> {
    if devices.is_empty() {
        return Err(
            "no GPU devices supplied. Fix: probe adapters before partitioning.".to_string(),
        );
    }
    let mut seen = rustc_hash::FxHashSet::default();
    vyre_foundation::allocation::try_reserve_hash_set_to_capacity(&mut seen, devices.len()).map_err(|error| {
        format!(
            "GPU device validation allocation failed for {} devices: {error}. Fix: lower adapter fanout or memory pressure before scheduling.",
            devices.len()
        )
    })?;
    for device in devices {
        if !seen.insert(device.device_index) {
            return Err(format!(
                "duplicate GPU device index {}. Fix: pass each adapter exactly once.",
                device.device_index
            ));
        }
    }
    for item in items {
        if item.cost == 0 {
            return Err(format!(
                "work item {} has zero cost. Fix: assign at least one cost unit or remove it.",
                item.id
            ));
        }
    }
    let mut seen_items = rustc_hash::FxHashSet::default();
    vyre_foundation::allocation::try_reserve_hash_set_to_capacity(&mut seen_items, items.len()).map_err(|error| {
        format!(
            "work item validation allocation failed for {} items: {error}. Fix: split the multi-GPU batch.",
            items.len()
        )
    })?;
    for item in items {
        if !seen_items.insert(item.id) {
            return Err(format!(
                "duplicate work item id {}. Fix: assign a unique stable id to every multi-GPU work item.",
                item.id
            ));
        }
    }
    Ok(())
}

fn ensure_vec_spare<T>(vec: &mut Vec<T>, additional: usize, label: &str) -> Result<(), String> {
    let spare = vec.capacity().saturating_sub(vec.len());
    if spare >= additional {
        return Ok(());
    }
    vec.try_reserve_exact(additional - spare).map_err(|error| {
        format!(
            "{label} allocation failed while extending by {additional} entries: {error}. Fix: split the multi-GPU batch."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_gpu_partition_unit() {
        let devices = [
            DeviceLoad {
                device_index: 0,
                queued_cost: 0,
            },
            DeviceLoad {
                device_index: 1,
                queued_cost: 4,
            },
        ];
        let items = [
            WeightedWorkItem { id: 10, cost: 9 },
            WeightedWorkItem { id: 11, cost: 4 },
            WeightedWorkItem { id: 12, cost: 4 },
            WeightedWorkItem { id: 13, cost: 1 },
        ];

        let partitions = partition_work_stealing(&devices, &items)
            .expect("Fix: valid synthetic device loads must partition");
        let mut assigned = partitions
            .iter()
            .flat_map(|partition| partition.item_ids.iter().copied())
            .collect::<Vec<_>>();
        assigned.sort_unstable();

        assert_eq!(assigned, vec![10, 11, 12, 13]);
        let spread = partitions
            .iter()
            .map(|partition| partition.total_cost)
            .max()
            .zip(
                partitions
                    .iter()
                    .map(|partition| partition.total_cost)
                    .min(),
            )
            .map(|(max, min)| max - min)
            .expect("Fix: partitions must be non-empty");
        assert!(
            spread <= 5,
            "synthetic work stealing left an avoidable load spread: {partitions:?}"
        );
    }

    #[test]
    fn rejects_duplicate_device_ordinals() {
        let devices = [
            DeviceLoad {
                device_index: 0,
                queued_cost: 0,
            },
            DeviceLoad {
                device_index: 0,
                queued_cost: 1,
            },
        ];

        let error = partition_work_stealing(&devices, &[WeightedWorkItem { id: 1, cost: 1 }])
            .expect_err("Fix: duplicate synthetic device indices must be rejected");
        assert!(error.contains("duplicate GPU device index"));
    }

    #[test]
    fn partition_source_has_no_release_path_infallible_allocation() {
        let source = include_str!("partition.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: partition production source must precede tests");
        assert!(
            !production.contains("Vec::with_capacity")
                && !production.contains("BinaryHeap::with_capacity")
                && !production.contains("with_capacity_and_hasher"),
            "Fix: WGPU multi-GPU partitioning must report allocation pressure instead of aborting on infallible capacity constructors."
        );
        assert!(
            production.contains("try_reserve_vec_to_capacity")
                && production.contains("try_reserve_binary_heap_to_capacity")
                && production.contains("try_reserve_hash_set_to_capacity")
                && production.contains("ensure_vec_spare"),
            "Fix: WGPU multi-GPU partitioning must use fallible allocation for schedule tables and per-device assignments."
        );
    }
}
