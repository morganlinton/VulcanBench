//! Backend-neutral multi-query execution planning over one resident graph.
//!
//! The release path cannot run N analyses over the same graph as N host-driven
//! dispatches. This planner groups compatible queries by resident graph layout
//! and traversal key so graph upload, traversal setup, and host fencing are paid
//! once per group instead of once per query.

use std::hash::Hash;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ordering::sort_unstable_by_key_if_needed;
use crate::reservation_policy::{
    reserve_typed_hash_map_to_capacity, reserve_typed_hash_set_to_capacity,
    reserve_typed_vec_to_capacity, reserved_typed_vec, ReservationPolicy,
};
use crate::ResidentGraphReuseTelemetry;

const MULTI_QUERY_RESERVATION: ReservationPolicy = ReservationPolicy::new(
    "multi-query execution",
    "shard the query batch before planning",
);

/// One backend analysis/query planned against a resident graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiQuery {
    /// Stable query id.
    pub query: u32,
    /// Stable resident graph layout hash.
    pub graph_layout_hash: u64,
    /// Traversal compatibility key. Equal keys share traversal work.
    pub traversal_key: u64,
    /// Bytes needed to upload the graph if it is not already resident.
    pub graph_upload_bytes: u64,
    /// Frontier/input bytes for this query.
    pub frontier_bytes: u64,
    /// Scratch bytes for this query.
    pub scratch_bytes: u64,
    /// Meaningful output bytes for this query.
    pub output_bytes: u64,
}

/// One grouped multi-query launch envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiQueryGroup {
    /// Resident graph hash shared by every query in the group.
    pub graph_layout_hash: u64,
    /// Traversal key shared by every query in the group.
    pub traversal_key: u64,
    /// Query ids in deterministic order.
    pub queries: Vec<u32>,
    /// Graph upload bytes paid once for this group.
    pub graph_upload_bytes: u64,
    /// Sum of query frontier bytes.
    pub frontier_bytes: u64,
    /// Peak scratch bytes needed by the fused group.
    pub peak_scratch_bytes: u64,
    /// Sum of meaningful output bytes.
    pub output_bytes: u64,
    /// Total resident bytes required for this group.
    pub resident_bytes: u64,
    /// Launches avoided versus per-query dispatch.
    pub avoided_launches: u32,
    /// Host fences avoided versus per-query dispatch.
    pub avoided_host_fences: u32,
    /// Graph upload bytes avoided by sharing residency inside this group.
    pub avoided_graph_upload_bytes: u64,
    /// Backend-neutral graph residency reuse telemetry for this group.
    pub graph_reuse: ResidentGraphReuseTelemetry,
}

/// Complete multi-query execution plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiQueryExecutionPlan {
    /// Groups in deterministic graph/traversal order.
    pub groups: Vec<MultiQueryGroup>,
    /// Number of backend launches after grouping.
    pub launch_count: u32,
    /// Total launches avoided versus per-query dispatch.
    pub avoided_launches: u32,
    /// Total host fences avoided versus per-query dispatch.
    pub avoided_host_fences: u32,
    /// Total graph upload bytes avoided by shared residency.
    pub avoided_graph_upload_bytes: u64,
    /// Backend-neutral graph residency reuse telemetry for the full plan.
    pub graph_reuse: ResidentGraphReuseTelemetry,
    /// Peak resident bytes across groups.
    pub peak_resident_bytes: u64,
    /// Plan guarantees one final host fence per group, never per query.
    pub final_only_host_fence_per_group: bool,
}

/// Caller-owned scratch for repeated multi-query planning.
#[derive(Debug, Default)]
pub struct MultiQueryExecutionScratch {
    group_indices: FxHashMap<(u64, u64), usize>,
    group_query_counts: FxHashMap<(u64, u64), usize>,
    resident_graphs: FxHashSet<u64>,
    resident_graph_bytes: FxHashMap<u64, u64>,
    grouped_queries: Vec<((u64, u64), Vec<MultiQuery>)>,
    free_query_buckets: Vec<Vec<MultiQuery>>,
    seen_queries: FxHashSet<u32>,
}

impl MultiQueryExecutionScratch {
    /// Create empty reusable multi-query planning scratch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            group_indices: FxHashMap::default(),
            group_query_counts: FxHashMap::default(),
            resident_graphs: FxHashSet::default(),
            resident_graph_bytes: FxHashMap::default(),
            grouped_queries: Vec::new(),
            free_query_buckets: Vec::new(),
            seen_queries: FxHashSet::default(),
        }
    }

    /// Allocate reusable multi-query planning scratch for a known batch size.
    pub fn try_with_capacity(query_count: usize) -> Result<Self, MultiQueryExecutionError> {
        let mut scratch = Self::new();
        scratch.try_reserve_query_shape(query_count)?;
        Ok(scratch)
    }

    fn try_reserve_query_shape(
        &mut self,
        query_count: usize,
    ) -> Result<(), MultiQueryExecutionError> {
        reserve_map(
            &mut self.group_indices,
            query_count,
            "multi-query group index table",
        )?;
        reserve_map(
            &mut self.group_query_counts,
            query_count,
            "multi-query group size table",
        )?;
        reserve_set(
            &mut self.resident_graphs,
            query_count,
            "multi-query resident graph set",
        )?;
        reserve_map(
            &mut self.resident_graph_bytes,
            query_count,
            "multi-query resident graph byte table",
        )?;
        reserve_vec(
            &mut self.grouped_queries,
            query_count,
            "multi-query grouped-query buckets",
        )?;
        reserve_set(
            &mut self.seen_queries,
            query_count,
            "multi-query seen query ids",
        )
    }

    /// Retained capacity for unique grouping keys.
    #[must_use]
    pub fn group_index_capacity(&self) -> usize {
        self.group_indices.capacity()
    }

    /// Retained capacity for grouped query buckets.
    #[must_use]
    pub fn grouped_query_capacity(&self) -> usize {
        self.grouped_queries.capacity()
    }

    /// Retained capacity for graph-residency tracking.
    #[must_use]
    pub fn resident_graph_capacity(&self) -> usize {
        self.resident_graphs.capacity()
    }

    /// Retained capacity across reusable grouped-query buckets.
    #[must_use]
    pub fn retained_query_bucket_capacity(&self) -> usize {
        self.free_query_buckets
            .iter()
            .map(Vec::capacity)
            .sum::<usize>()
            + self
                .grouped_queries
                .iter()
                .map(|(_, queries)| queries.capacity())
                .sum::<usize>()
    }

    fn clear(&mut self) -> Result<(), MultiQueryExecutionError> {
        self.group_indices.clear();
        self.group_query_counts.clear();
        self.resident_graphs.clear();
        self.resident_graph_bytes.clear();
        let retained_bucket_count = self
            .free_query_buckets
            .len()
            .checked_add(self.grouped_queries.len())
            .ok_or(MultiQueryExecutionError::ByteCountOverflow {
                field: "retained multi-query bucket count",
            })?;
        reserve_vec(
            &mut self.free_query_buckets,
            retained_bucket_count,
            "multi-query retained bucket pool",
        )?;
        for (_, mut queries) in self.grouped_queries.drain(..) {
            queries.clear();
            self.free_query_buckets.push(queries);
        }
        self.seen_queries.clear();
        Ok(())
    }
}

fn take_reserved_query_bucket(
    free_query_buckets: &mut Vec<Vec<MultiQuery>>,
    query_count: usize,
) -> Result<Vec<MultiQuery>, MultiQueryExecutionError> {
    let mut queries = free_query_buckets.pop().unwrap_or_default();
    if let Err(error) = reserve_vec(
        &mut queries,
        query_count,
        "multi-query grouped query bucket",
    ) {
        free_query_buckets.push(queries);
        return Err(error);
    }
    queries.clear();
    Ok(queries)
}

/// multi-query planning errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MultiQueryExecutionError {
    /// Duplicate query id.
    DuplicateQuery {
        /// Duplicate query id.
        query: u32,
    },
    /// Query needs a non-zero graph hash.
    ZeroGraphHash {
        /// Invalid query id.
        query: u32,
    },
    /// Query needs a non-zero traversal compatibility key.
    ZeroTraversalKey {
        /// Invalid query id.
        query: u32,
    },
    /// Query must report non-zero resident graph upload bytes.
    ZeroGraphUploadBytes {
        /// Invalid query id.
        query: u32,
    },
    /// Equal graph hashes must agree on the resident graph byte width.
    GraphUploadBytesMismatch {
        /// Stable resident graph layout hash.
        graph_layout_hash: u64,
        /// First byte width observed for this graph hash.
        expected_bytes: u64,
        /// Conflicting byte width reported by this query.
        actual_bytes: u64,
        /// Query that reported the conflicting byte width.
        query: u32,
    },
    /// Explicit device budget cannot be zero.
    ZeroBudget,
    /// Byte arithmetic overflowed.
    ByteCountOverflow {
        /// Field being computed.
        field: &'static str,
    },
    /// A grouped resident envelope exceeds the explicit device budget.
    OverBudget {
        /// Group graph hash.
        graph_layout_hash: u64,
        /// Group traversal key.
        traversal_key: u64,
        /// Required resident bytes.
        required_bytes: u64,
        /// Caller-provided budget.
        budget_bytes: u64,
    },
    /// Planner storage could not be reserved.
    StorageReserveFailed {
        /// Field being reserved.
        field: &'static str,
        /// Number of entries requested.
        requested: usize,
        /// Allocator error text.
        message: String,
    },
    /// Planner state violated an internal construction invariant.
    InternalInvariant {
        /// Actionable invariant failure text.
        message: &'static str,
    },
}

fn storage_reserve_failed(
    field: &'static str,
    requested: usize,
    message: String,
) -> MultiQueryExecutionError {
    MultiQueryExecutionError::StorageReserveFailed {
        field,
        requested,
        message,
    }
}

fn reserve_vec<T>(
    vec: &mut Vec<T>,
    target_capacity: usize,
    field: &'static str,
) -> Result<(), MultiQueryExecutionError> {
    reserve_typed_vec_to_capacity(
        MULTI_QUERY_RESERVATION,
        vec,
        target_capacity,
        field,
        storage_reserve_failed,
    )
}

fn reserved_vec<T>(
    target_capacity: usize,
    field: &'static str,
) -> Result<Vec<T>, MultiQueryExecutionError> {
    reserved_typed_vec(
        MULTI_QUERY_RESERVATION,
        target_capacity,
        field,
        storage_reserve_failed,
    )
}

fn reserve_set<T>(
    set: &mut FxHashSet<T>,
    target_capacity: usize,
    field: &'static str,
) -> Result<(), MultiQueryExecutionError>
where
    T: Eq + Hash,
{
    reserve_typed_hash_set_to_capacity(
        MULTI_QUERY_RESERVATION,
        set,
        target_capacity,
        field,
        storage_reserve_failed,
    )
}

fn reserve_map<K, V>(
    map: &mut FxHashMap<K, V>,
    target_capacity: usize,
    field: &'static str,
) -> Result<(), MultiQueryExecutionError>
where
    K: Eq + Hash,
{
    reserve_typed_hash_map_to_capacity(
        MULTI_QUERY_RESERVATION,
        map,
        target_capacity,
        field,
        storage_reserve_failed,
    )
}

fn checked_add(lhs: u64, rhs: u64, field: &'static str) -> Result<u64, MultiQueryExecutionError> {
    lhs.checked_add(rhs)
        .ok_or(MultiQueryExecutionError::ByteCountOverflow { field })
}

fn checked_add_u32(
    lhs: u32,
    rhs: u32,
    field: &'static str,
) -> Result<u32, MultiQueryExecutionError> {
    lhs.checked_add(rhs)
        .ok_or(MultiQueryExecutionError::ByteCountOverflow { field })
}

impl std::fmt::Display for MultiQueryExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateQuery { query } => write!(
                f,
                "multi-query execution received duplicate query id {query}. Fix: assign unique ids before batch planning."
            ),
            Self::ZeroGraphHash { query } => write!(
                f,
                "multi-query {query} has graph_layout_hash=0. Fix: normalize and hash the resident graph before query batching."
            ),
            Self::ZeroTraversalKey { query } => write!(
                f,
                "multi-query {query} has traversal_key=0. Fix: emit a concrete traversal compatibility key before multi-query batching."
            ),
            Self::ZeroGraphUploadBytes { query } => write!(
                f,
                "multi-query {query} has graph_upload_bytes=0. Fix: pass the concrete resident graph topology byte count before multi-query batching."
            ),
            Self::GraphUploadBytesMismatch {
                graph_layout_hash,
                expected_bytes,
                actual_bytes,
                query,
            } => write!(
                f,
                "multi-query graph hash {graph_layout_hash} reported conflicting resident byte widths: expected {expected_bytes}, query {query} reported {actual_bytes}. Fix: canonicalize graph layout hashing and byte accounting before multi-query batching."
            ),
            Self::ZeroBudget => write!(
                f,
                "multi-query execution received a zero device budget. Fix: pass an explicit resident memory budget before planning."
            ),
            Self::ByteCountOverflow { field } => write!(
                f,
                "multi-query execution overflowed while computing {field}. Fix: shard the query batch before planning."
            ),
            Self::OverBudget {
                graph_layout_hash,
                traversal_key,
                required_bytes,
                budget_bytes,
            } => write!(
                f,
                "multi-query group graph={graph_layout_hash} traversal={traversal_key} requires {required_bytes} bytes but budget allows {budget_bytes}. Fix: split the group or raise the explicit multi-query budget."
            ),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "multi-query execution could not reserve {requested} {field} entries: {message}. Fix: shard the query batch before planning."
            ),
            Self::InternalInvariant { message } => write!(
                f,
                "multi-query execution violated an internal planner invariant: {message}. Fix: keep group counting and bucket indexing in one validated planning pass."
            ),
        }
    }
}

impl std::error::Error for MultiQueryExecutionError {}

/// Plan multi-query execution over shared resident graphs.
pub fn plan_multi_query_execution(
    queries: &[MultiQuery],
    budget_bytes: u64,
) -> Result<MultiQueryExecutionPlan, MultiQueryExecutionError> {
    let mut scratch = MultiQueryExecutionScratch::try_with_capacity(queries.len())?;
    plan_multi_query_execution_with_scratch(queries, budget_bytes, &mut scratch)
}

/// Plan multi-query execution using caller-owned planning scratch.
pub fn plan_multi_query_execution_with_scratch(
    queries: &[MultiQuery],
    budget_bytes: u64,
    scratch: &mut MultiQueryExecutionScratch,
) -> Result<MultiQueryExecutionPlan, MultiQueryExecutionError> {
    if budget_bytes == 0 {
        return Err(MultiQueryExecutionError::ZeroBudget);
    }
    if queries.is_empty() {
        return Ok(MultiQueryExecutionPlan {
            launch_count: 0,
            groups: Vec::new(),
            avoided_launches: 0,
            avoided_host_fences: 0,
            avoided_graph_upload_bytes: 0,
            graph_reuse: ResidentGraphReuseTelemetry::default(),
            peak_resident_bytes: 0,
            final_only_host_fence_per_group: true,
        });
    }
    if queries.len() == 1 {
        let query = queries[0];
        if query.graph_layout_hash == 0 {
            return Err(MultiQueryExecutionError::ZeroGraphHash { query: query.query });
        }
        if query.traversal_key == 0 {
            return Err(MultiQueryExecutionError::ZeroTraversalKey { query: query.query });
        }
        if query.graph_upload_bytes == 0 {
            return Err(MultiQueryExecutionError::ZeroGraphUploadBytes { query: query.query });
        }
        let resident_bytes = group_resident_bytes(
            query.graph_upload_bytes,
            query.frontier_bytes,
            query.scratch_bytes,
            query.output_bytes,
        )?;
        if resident_bytes > budget_bytes {
            return Err(MultiQueryExecutionError::OverBudget {
                graph_layout_hash: query.graph_layout_hash,
                traversal_key: query.traversal_key,
                required_bytes: resident_bytes,
                budget_bytes,
            });
        }
        let mut query_ids = reserved_vec(1, "multi-query singleton query ids")?;
        query_ids.push(query.query);
        let mut groups = reserved_vec(1, "multi-query output groups")?;
        groups.push(MultiQueryGroup {
            graph_layout_hash: query.graph_layout_hash,
            traversal_key: query.traversal_key,
            queries: query_ids,
            graph_upload_bytes: query.graph_upload_bytes,
            frontier_bytes: query.frontier_bytes,
            peak_scratch_bytes: query.scratch_bytes,
            output_bytes: query.output_bytes,
            resident_bytes,
            avoided_launches: 0,
            avoided_host_fences: 0,
            avoided_graph_upload_bytes: 0,
            graph_reuse: ResidentGraphReuseTelemetry::cold_upload(query.graph_upload_bytes),
        });
        return Ok(MultiQueryExecutionPlan {
            launch_count: 1,
            groups,
            avoided_launches: 0,
            avoided_host_fences: 0,
            avoided_graph_upload_bytes: 0,
            graph_reuse: ResidentGraphReuseTelemetry::cold_upload(query.graph_upload_bytes),
            peak_resident_bytes: resident_bytes,
            final_only_host_fence_per_group: true,
        });
    }

    scratch.clear()?;
    scratch.try_reserve_query_shape(queries.len())?;
    for query in queries {
        if !scratch.seen_queries.insert(query.query) {
            return Err(MultiQueryExecutionError::DuplicateQuery { query: query.query });
        }
        if query.graph_layout_hash == 0 {
            return Err(MultiQueryExecutionError::ZeroGraphHash { query: query.query });
        }
        if query.traversal_key == 0 {
            return Err(MultiQueryExecutionError::ZeroTraversalKey { query: query.query });
        }
        if query.graph_upload_bytes == 0 {
            return Err(MultiQueryExecutionError::ZeroGraphUploadBytes { query: query.query });
        }
        match scratch
            .resident_graph_bytes
            .get(&query.graph_layout_hash)
            .copied()
        {
            Some(expected_bytes) if expected_bytes != query.graph_upload_bytes => {
                return Err(MultiQueryExecutionError::GraphUploadBytesMismatch {
                    graph_layout_hash: query.graph_layout_hash,
                    expected_bytes,
                    actual_bytes: query.graph_upload_bytes,
                    query: query.query,
                });
            }
            Some(_) => {}
            None => {
                scratch
                    .resident_graph_bytes
                    .insert(query.graph_layout_hash, query.graph_upload_bytes);
            }
        }
        let key = (query.graph_layout_hash, query.traversal_key);
        let count = scratch.group_query_counts.entry(key).or_insert(0);
        *count = count
            .checked_add(1)
            .ok_or(MultiQueryExecutionError::ByteCountOverflow {
                field: "multi-query grouped query count",
            })?;
    }

    reserve_vec(
        &mut scratch.grouped_queries,
        scratch.group_query_counts.len(),
        "multi-query grouped-query buckets",
    )?;
    for (&key, &query_count) in &scratch.group_query_counts {
        let index = scratch.grouped_queries.len();
        let queries = take_reserved_query_bucket(&mut scratch.free_query_buckets, query_count)?;
        scratch.grouped_queries.push((key, queries));
        scratch.group_indices.insert(key, index);
    }

    for query in queries {
        let key = (query.graph_layout_hash, query.traversal_key);
        let index = scratch.group_indices.get(&key).copied().ok_or(
            MultiQueryExecutionError::InternalInvariant {
                message: "validated multi-query group key missing from exact-capacity bucket index",
            },
        )?;
        scratch.grouped_queries[index].1.push(*query);
    }

    let mut groups = reserved_vec(scratch.grouped_queries.len(), "multi-query output groups")?;
    let mut avoided_launches = 0_u32;
    let mut avoided_host_fences = 0_u32;
    let mut avoided_graph_upload_bytes = 0_u64;
    let mut graph_reuse = ResidentGraphReuseTelemetry::default();
    let mut peak_resident_bytes = 0_u64;

    sort_unstable_by_key_if_needed(&mut scratch.grouped_queries, |(key, _)| *key);
    for ((graph_layout_hash, traversal_key), group_queries) in &mut scratch.grouped_queries {
        sort_unstable_by_key_if_needed(group_queries, |query| query.query);
        let first_new_group = groups.len();
        let graph_already_resident = !scratch.resident_graphs.insert(*graph_layout_hash);
        append_memory_fit_groups(
            *graph_layout_hash,
            *traversal_key,
            group_queries,
            budget_bytes,
            graph_already_resident,
            &mut groups,
        )?;
        for group in &groups[first_new_group..] {
            avoided_launches =
                checked_add_u32(avoided_launches, group.avoided_launches, "avoided launches")?;
            avoided_host_fences = checked_add_u32(
                avoided_host_fences,
                group.avoided_host_fences,
                "avoided host fences",
            )?;
            avoided_graph_upload_bytes = checked_add(
                avoided_graph_upload_bytes,
                group.avoided_graph_upload_bytes,
                "avoided graph upload bytes",
            )?;
            graph_reuse = graph_reuse.checked_add(group.graph_reuse).map_err(|_| {
                MultiQueryExecutionError::ByteCountOverflow {
                    field: "graph reuse telemetry",
                }
            })?;
            peak_resident_bytes = peak_resident_bytes.max(group.resident_bytes);
        }
    }
    let launch_count =
        u32::try_from(groups.len()).map_err(|_| MultiQueryExecutionError::ByteCountOverflow {
            field: "launch count",
        })?;

    Ok(MultiQueryExecutionPlan {
        launch_count,
        groups,
        avoided_launches,
        avoided_host_fences,
        avoided_graph_upload_bytes,
        graph_reuse,
        peak_resident_bytes,
        final_only_host_fence_per_group: true,
    })
}

fn append_memory_fit_groups(
    graph_layout_hash: u64,
    traversal_key: u64,
    queries: &[MultiQuery],
    budget_bytes: u64,
    graph_already_resident: bool,
    groups: &mut Vec<MultiQueryGroup>,
) -> Result<(), MultiQueryExecutionError> {
    let mut start = 0usize;
    let resident_graph_bytes = queries[0].graph_upload_bytes;
    while start < queries.len() {
        let graph_upload_bytes = if start == 0 && !graph_already_resident {
            resident_graph_bytes
        } else {
            0
        };
        let mut avoided_graph_upload_bytes = if graph_upload_bytes == 0 {
            queries[start].graph_upload_bytes
        } else {
            0
        };
        let mut warm_reuses = if graph_upload_bytes == 0 { 1 } else { 0 };
        let mut frontier_bytes = 0_u64;
        let mut peak_scratch_bytes = 0_u64;
        let mut output_bytes = 0_u64;
        let mut resident_bytes = graph_upload_bytes;
        let mut cursor = start;

        while cursor < queries.len() {
            let query = queries[cursor];
            let candidate_frontier =
                checked_add(frontier_bytes, query.frontier_bytes, "frontier bytes")?;
            let candidate_scratch = peak_scratch_bytes.max(query.scratch_bytes);
            let candidate_output = checked_add(output_bytes, query.output_bytes, "output bytes")?;
            let candidate_resident = group_resident_bytes(
                resident_graph_bytes,
                candidate_frontier,
                candidate_scratch,
                candidate_output,
            )?;

            if candidate_resident > budget_bytes {
                if cursor == start {
                    return Err(MultiQueryExecutionError::OverBudget {
                        graph_layout_hash,
                        traversal_key,
                        required_bytes: candidate_resident,
                        budget_bytes,
                    });
                }
                break;
            }

            if cursor != start {
                avoided_graph_upload_bytes = checked_add(
                    avoided_graph_upload_bytes,
                    query.graph_upload_bytes,
                    "avoided graph upload bytes",
                )?;
                warm_reuses = checked_add(warm_reuses, 1, "warm resident graph reuse count")?;
            }
            frontier_bytes = candidate_frontier;
            peak_scratch_bytes = candidate_scratch;
            output_bytes = candidate_output;
            resident_bytes = candidate_resident;
            cursor += 1;
        }

        let chunk_len =
            cursor
                .checked_sub(start)
                .ok_or(MultiQueryExecutionError::InternalInvariant {
                    message: "multi-query chunk cursor moved before chunk start",
                })?;
        let mut query_ids = reserved_vec(chunk_len, "multi-query chunk query ids")?;
        for query in &queries[start..cursor] {
            query_ids.push(query.query);
        }

        let avoided = u32::try_from(chunk_len - 1).map_err(|_| {
            MultiQueryExecutionError::ByteCountOverflow {
                field: "avoided launches",
            }
        })?;
        groups.push(MultiQueryGroup {
            graph_layout_hash,
            traversal_key,
            queries: query_ids,
            graph_upload_bytes,
            frontier_bytes,
            peak_scratch_bytes,
            output_bytes,
            resident_bytes,
            avoided_launches: avoided,
            avoided_host_fences: avoided,
            avoided_graph_upload_bytes,
            graph_reuse: ResidentGraphReuseTelemetry::from_counters(
                u64::from(graph_upload_bytes != 0),
                warm_reuses,
                graph_upload_bytes,
                avoided_graph_upload_bytes,
            ),
        });
        start = cursor;
    }
    Ok(())
}

fn group_resident_bytes(
    graph_upload_bytes: u64,
    frontier_bytes: u64,
    peak_scratch_bytes: u64,
    output_bytes: u64,
) -> Result<u64, MultiQueryExecutionError> {
    let graph_plus_frontier = checked_add(
        graph_upload_bytes,
        frontier_bytes,
        "graph plus frontier resident bytes",
    )?;
    let with_scratch = checked_add(
        graph_plus_frontier,
        peak_scratch_bytes,
        "resident bytes with scratch",
    )?;
    checked_add(with_scratch, output_bytes, "resident bytes with outputs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_query_batches_compatible_queries_over_one_resident_graph() {
        let plan = plan_multi_query_execution(
            &[
                query(3, 0xabc, 0x10, 4_096, 64, 128, 32),
                query(1, 0xabc, 0x10, 4_096, 32, 64, 16),
                query(2, 0xabc, 0x10, 4_096, 48, 96, 24),
            ],
            8_192,
        )
        .expect("Fix: compatible queries should batch");

        assert_eq!(plan.launch_count, 1);
        assert_eq!(plan.avoided_launches, 2);
        assert_eq!(plan.avoided_host_fences, 2);
        assert_eq!(plan.avoided_graph_upload_bytes, 8_192);
        assert_eq!(
            plan.graph_reuse,
            ResidentGraphReuseTelemetry::from_counters(1, 2, 4_096, 8_192)
        );
        assert_eq!(plan.groups[0].queries, vec![1, 2, 3]);
        assert_eq!(
            plan.groups[0].graph_reuse,
            ResidentGraphReuseTelemetry::from_counters(1, 2, 4_096, 8_192)
        );
        assert_eq!(plan.groups[0].frontier_bytes, 144);
        assert_eq!(plan.groups[0].peak_scratch_bytes, 128);
        assert_eq!(plan.groups[0].output_bytes, 72);
        assert!(plan.final_only_host_fence_per_group);
    }

    #[test]
    fn multi_query_splits_compatible_group_to_fit_cuda_budget_without_reuploading_graph() {
        let plan = plan_multi_query_execution(
            &[
                query(1, 0xabc, 0x10, 100, 100, 10, 10),
                query(2, 0xabc, 0x10, 100, 100, 10, 10),
                query(3, 0xabc, 0x10, 100, 100, 10, 10),
            ],
            350,
        )
        .expect("Fix: compatible multi-query queries should split into budget-fit resident chunks");

        assert_eq!(plan.launch_count, 2);
        assert_eq!(plan.avoided_launches, 1);
        assert_eq!(plan.avoided_host_fences, 1);
        assert_eq!(plan.avoided_graph_upload_bytes, 200);
        assert_eq!(
            plan.graph_reuse,
            ResidentGraphReuseTelemetry::from_counters(1, 2, 100, 200)
        );
        assert_eq!(plan.peak_resident_bytes, 330);
        assert_eq!(plan.groups[0].queries, vec![1, 2]);
        assert_eq!(plan.groups[0].graph_upload_bytes, 100);
        assert_eq!(plan.groups[0].resident_bytes, 330);
        assert_eq!(plan.groups[1].queries, vec![3]);
        assert_eq!(plan.groups[1].graph_upload_bytes, 0);
        assert_eq!(plan.groups[1].resident_bytes, 220);
        assert!(plan.final_only_host_fence_per_group);
    }

    #[test]
    fn multi_query_later_chunks_still_count_resident_graph_memory() {
        assert_eq!(
            plan_multi_query_execution(
                &[
                    query(1, 0xabc, 0x10, 100, 100, 10, 10),
                    query(2, 0xabc, 0x10, 100, 100, 10, 10),
                ],
                150,
            )
            .expect_err("later resident chunk still needs graph memory and should exceed budget"),
            MultiQueryExecutionError::OverBudget {
                graph_layout_hash: 0xabc,
                traversal_key: 0x10,
                required_bytes: 220,
                budget_bytes: 150,
            }
        );
    }

    #[test]
    fn multi_query_split_chunks_reserve_only_actual_chunk_ids() {
        let plan = plan_multi_query_execution(
            &[
                query(1, 0xabc, 0x10, 100, 100, 10, 10),
                query(2, 0xabc, 0x10, 100, 100, 10, 10),
                query(3, 0xabc, 0x10, 100, 100, 10, 10),
                query(4, 0xabc, 0x10, 100, 100, 10, 10),
            ],
            220,
        )
        .expect("Fix: multi-query planner should split into single-query chunks");

        assert_eq!(plan.launch_count, 4);
        assert!(plan.groups.iter().all(|group| group.queries.len() == 1));
        assert_eq!(plan.avoided_launches, 0);
        assert_eq!(plan.avoided_host_fences, 0);
        assert_eq!(plan.avoided_graph_upload_bytes, 300);

        let src = include_str!("multi_query_execution.rs");
        assert!(
            src.contains("let chunk_len =")
                && src.contains("reserved_vec(chunk_len, \"multi-query chunk query ids\")")
                && !src.contains(concat!("reserved_vec(queries.len()", " - start")),
            "Fix: split multi-query chunks must reserve only the actual chunk size, not the whole remaining tail."
        );
    }

    #[test]
    fn multi_query_splits_incompatible_graph_or_traversal_keys() {
        let plan = plan_multi_query_execution(
            &[
                query(1, 0xdef, 0x10, 1_024, 32, 64, 16),
                query(2, 0xabc, 0x20, 1_024, 32, 64, 16),
                query(3, 0xabc, 0x10, 1_024, 32, 64, 16),
            ],
            4_096,
        )
        .expect("Fix: incompatible queries should become separate groups");

        assert_eq!(plan.launch_count, 3);
        assert_eq!(plan.avoided_launches, 0);
        assert_eq!(plan.avoided_graph_upload_bytes, 1_024);
        assert_eq!(
            plan.graph_reuse,
            ResidentGraphReuseTelemetry::from_counters(2, 1, 2_048, 1_024)
        );
        assert_eq!(plan.groups[0].graph_upload_bytes, 1_024);
        assert_eq!(plan.groups[1].graph_upload_bytes, 0);
        assert_eq!(plan.groups[2].graph_upload_bytes, 1_024);
        assert_eq!(
            plan.groups
                .iter()
                .map(|group| (group.graph_layout_hash, group.traversal_key))
                .collect::<Vec<_>>(),
            vec![(0xabc, 0x10), (0xabc, 0x20), (0xdef, 0x10)]
        );
    }

    #[test]
    fn multi_query_grouping_avoids_tree_lookup_per_query() {
        let src = include_str!("multi_query_execution.rs");
        assert!(
            !src.contains(concat!("BTree", "Map")),
            "Fix: multi-query grouping should hash query ids and group indices, then sort final groups once for deterministic output."
        );
    }

    #[test]
    fn multi_query_planner_reuses_caller_owned_grouping_scratch() {
        let mut scratch = MultiQueryExecutionScratch::try_with_capacity(128)
            .expect("Fix: multi-query scratch should reserve");
        let wide = (0..128)
            .map(|index| query(index, 0xabc, 0x10, 4_096, 4, 8, 4))
            .collect::<Vec<_>>();
        let first = plan_multi_query_execution_with_scratch(&wide, 16_384, &mut scratch)
            .expect("Fix: wide compatible query batch should plan");
        let group_index_capacity = scratch.group_index_capacity();
        let grouped_query_capacity = scratch.grouped_query_capacity();
        let resident_graph_capacity = scratch.resident_graph_capacity();
        let query_bucket_capacity = scratch.retained_query_bucket_capacity();

        assert_eq!(first.launch_count, 1);
        assert_eq!(first.groups[0].queries.len(), 128);
        assert!(
            query_bucket_capacity >= 128,
            "Fix: multi-query scratch must retain inner grouped-query bucket capacity across planning calls"
        );

        let second = plan_multi_query_execution_with_scratch(
            &[
                query(9, 0xdef, 0x20, 1_024, 16, 32, 8),
                query(7, 0xabc, 0x10, 1_024, 16, 32, 8),
            ],
            4_096,
            &mut scratch,
        )
        .expect("Fix: smaller incompatible query batch should reuse previous scratch");

        assert_eq!(second.launch_count, 2);
        assert!(scratch.group_index_capacity() >= group_index_capacity);
        assert!(scratch.grouped_query_capacity() >= grouped_query_capacity);
        assert!(scratch.resident_graph_capacity() >= resident_graph_capacity);
        assert!(scratch.retained_query_bucket_capacity() >= query_bucket_capacity);

        let src = include_str!("multi_query_execution.rs");
        assert!(
            src.contains("pub fn plan_multi_query_execution_with_scratch"),
            "Fix: release callers need a scratch-aware multi-query planning path"
        );
        assert!(
            src.contains("scratch.grouped_queries.sort_unstable_by_key"),
            "Fix: deterministic multi-query output should sort retained scratch buckets in place"
        );
    }

    #[test]
    fn reused_query_bucket_returns_to_pool_when_reservation_fails() {
        let retained = vec![query(42, 0xabc, 0x10, 4_096, 8, 16, 4)];
        let mut free_query_buckets = vec![retained.clone()];

        let err = take_reserved_query_bucket(&mut free_query_buckets, usize::MAX)
            .expect_err("impossible query bucket reservation must fail");

        assert!(
            matches!(
                err,
                MultiQueryExecutionError::StorageReserveFailed {
                    field: "multi-query grouped query bucket",
                    ..
                }
            ),
            "Fix: query bucket reservation failure must surface the grouped-bucket field"
        );
        assert_eq!(
            free_query_buckets,
            vec![retained],
            "failed reservation must return the reusable multi-query query bucket to scratch"
        );
    }

    #[test]
    fn multi_query_planner_staging_reserves_fallibly() {
        let production = include_str!("multi_query_execution.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: multi-query production source must precede tests");

        assert!(
            production.contains("MultiQueryExecutionScratch::try_with_capacity(queries.len())?")
                && production.contains("scratch.try_reserve_query_shape(queries.len())?")
                && production.contains("use crate::reservation_policy::{")
                && production.contains("reserve_typed_vec_to_capacity")
                && production.contains("reserve_typed_hash_map_to_capacity")
                && production.contains("reserve_typed_hash_set_to_capacity")
                && production.contains("StorageReserveFailed")
                && production.contains("const MULTI_QUERY_RESERVATION"),
            "Fix: multi-query execution planning must reserve scratch and output staging fallibly."
        );
        assert!(
            !production.contains(concat!("FxHashMap::with_capacity", "_and_hasher"))
                && !production.contains(concat!("FxHashSet::with_capacity", "_and_hasher"))
                && !production.contains(concat!("Vec::with_capacity", "(query_count)"))
                && !production.contains(concat!(
                    "Vec::with_capacity",
                    "(scratch.grouped_queries.len())"
                ))
                && !production.contains(concat!("Vec::with_capacity", "(queries.len() - start)"))
                && !production.contains(concat!("groups: vec![", "MultiQueryGroup"))
                && !production.contains(concat!("queries: vec![", "query.query]"))
                && !production
                    .contains(concat!("scratch.group_indices", ".reserve(queries.len())"))
                && !production.contains(concat!(
                    "scratch.grouped_queries",
                    ".reserve(queries.len())"
                ))
                && !production.contains(concat!("scratch.seen_queries", ".reserve(queries.len())")),
            "Fix: multi-query release planning must not use infallible staging allocation."
        );
    }

    #[test]
    fn multi_query_planner_uses_shared_monotonic_sort_fast_path() {
        let production = include_str!("multi_query_execution.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: multi-query production source must precede tests");

        assert!(
            production.contains("use crate::ordering::sort_unstable_by_key_if_needed;")
                && production.contains("sort_unstable_by_key_if_needed(&mut scratch.grouped_queries")
                && production.contains("sort_unstable_by_key_if_needed(group_queries"),
            "Fix: multi-query planning must reuse the shared monotonic sort fast path for release-order batches."
        );
        assert!(
            !production.contains(".sort_unstable_by_key("),
            "Fix: multi-query planning must not sort already monotonic release batches unconditionally."
        );
    }

    #[test]
    fn generated_multi_query_plans_preserve_grouping_budget_and_identity_contracts() {
        let mut state = 0x6a09_e667_f3bc_c909_u64;
        for case_index in 0..768usize {
            let query_count = 1 + (next_u64(&mut state) as usize % 64);
            let mut graph_bytes_by_hash = [0_u64; 8];
            let mut queries = Vec::new();
            for index in 0..query_count {
                let graph_slot = (next_u64(&mut state) as usize % graph_bytes_by_hash.len()) + 1;
                let graph_upload_bytes = if graph_bytes_by_hash[graph_slot - 1] == 0 {
                    128 + next_u64(&mut state) % 16_384
                } else {
                    graph_bytes_by_hash[graph_slot - 1]
                };
                graph_bytes_by_hash[graph_slot - 1] = graph_upload_bytes;
                queries.push(query(
                    index as u32,
                    graph_slot as u64,
                    1 + next_u64(&mut state) % 5,
                    graph_upload_bytes,
                    next_u64(&mut state) % 512,
                    next_u64(&mut state) % 1_024,
                    next_u64(&mut state) % 256,
                ));
            }

            let budget = graph_bytes_by_hash.iter().copied().sum::<u64>()
                + (query_count as u64 * 2_048)
                + 16_384;
            let plan = plan_multi_query_execution(&queries, budget)
                .expect("Fix: generated multi-query plan should fit generous budget");
            assert_eq!(
                plan.launch_count as usize,
                plan.groups.len(),
                "case {case_index}"
            );
            assert!(plan.final_only_host_fence_per_group, "case {case_index}");
            assert!(
                plan.groups.windows(2).all(|pair| (
                    pair[0].graph_layout_hash,
                    pair[0].traversal_key
                ) <= (
                    pair[1].graph_layout_hash,
                    pair[1].traversal_key
                )),
                "case {case_index}"
            );
            let mut seen = vec![false; query_count];
            let mut avoided_launches = 0_u32;
            let mut avoided_host_fences = 0_u32;
            let mut peak_resident_bytes = 0_u64;
            for group in &plan.groups {
                assert!(group.resident_bytes <= budget, "case {case_index}");
                assert!(
                    group.queries.windows(2).all(|pair| pair[0] <= pair[1]),
                    "case {case_index}"
                );
                avoided_launches = avoided_launches
                    .checked_add(group.avoided_launches)
                    .expect("Fix: generated avoided launch sum should fit u32");
                avoided_host_fences = avoided_host_fences
                    .checked_add(group.avoided_host_fences)
                    .expect("Fix: generated avoided fence sum should fit u32");
                peak_resident_bytes = peak_resident_bytes.max(group.resident_bytes);
                for query in &group.queries {
                    let slot = *query as usize;
                    assert!(slot < query_count, "case {case_index}");
                    assert!(!seen[slot], "case {case_index}");
                    seen[slot] = true;
                }
            }
            assert!(seen.into_iter().all(|value| value), "case {case_index}");
            assert_eq!(plan.avoided_launches, avoided_launches, "case {case_index}");
            assert_eq!(
                plan.avoided_host_fences, avoided_host_fences,
                "case {case_index}"
            );
            assert_eq!(
                plan.peak_resident_bytes, peak_resident_bytes,
                "case {case_index}"
            );
        }
    }

    #[test]
    fn multi_query_rejects_invalid_inputs_and_budget_overflow() {
        assert_eq!(
            plan_multi_query_execution(&[query(1, 0, 1, 8, 1, 1, 1)], 128)
                .expect_err("missing graph hash should fail"),
            MultiQueryExecutionError::ZeroGraphHash { query: 1 }
        );
        assert_eq!(
            plan_multi_query_execution(&[query(1, 1, 1, 0, 1, 1, 1)], 128)
                .expect_err("zero graph bytes should fail"),
            MultiQueryExecutionError::ZeroGraphUploadBytes { query: 1 }
        );
        assert_eq!(
            plan_multi_query_execution(
                &[query(1, 1, 1, 8, 1, 1, 1), query(2, 1, 2, 16, 1, 1, 1)],
                128,
            )
            .expect_err("same graph hash with conflicting bytes should fail"),
            MultiQueryExecutionError::GraphUploadBytesMismatch {
                graph_layout_hash: 1,
                expected_bytes: 8,
                actual_bytes: 16,
                query: 2,
            }
        );
        assert_eq!(
            plan_multi_query_execution(
                &[query(1, 1, 1, 8, 1, 1, 1), query(1, 1, 1, 8, 1, 1, 1)],
                128,
            )
            .expect_err("duplicate query should fail"),
            MultiQueryExecutionError::DuplicateQuery { query: 1 }
        );
        assert_eq!(
            plan_multi_query_execution(&[query(2, 1, 1, 128, 16, 16, 16)], 127)
                .expect_err("over-budget group should fail"),
            MultiQueryExecutionError::OverBudget {
                graph_layout_hash: 1,
                traversal_key: 1,
                required_bytes: 176,
                budget_bytes: 127,
            }
        );
    }

    fn query(
        query: u32,
        graph_layout_hash: u64,
        traversal_key: u64,
        graph_upload_bytes: u64,
        frontier_bytes: u64,
        scratch_bytes: u64,
        output_bytes: u64,
    ) -> MultiQuery {
        MultiQuery {
            query,
            graph_layout_hash,
            traversal_key,
            graph_upload_bytes,
            frontier_bytes,
            scratch_bytes,
            output_bytes,
        }
    }

    fn next_u64(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }
}
