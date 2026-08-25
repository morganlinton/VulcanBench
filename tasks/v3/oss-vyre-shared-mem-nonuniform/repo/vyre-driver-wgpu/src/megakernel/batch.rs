//! Device-resident multi-file batch containers for the megakernel path.
//!
//! `FileBatch` packs many files into one contiguous haystack buffer,
//! uploads the prefix-sum offsets + metadata tables once, and keeps a
//! persistent device-derived work schedule + sparse hit ring alive across dispatches.

use super::dispatcher::TransitionWidth;
use super::segmentation;
use crate::buffer::GpuBufferHandle;
use crate::staging_reserve::reserve_vec_exact_for_len;
use std::sync::Arc;
use vyre_runtime::PipelineError;

/// Number of `u32` words stored per file metadata record.
pub const FILE_METADATA_WORDS: usize = 4;
/// Number of `u32` words stored per work item.
pub const WORK_TRIPLE_WORDS: usize = 3;
/// Number of `u32` words stored per sparse hit record.
pub const HIT_RECORD_WORDS: usize = 4;
/// Number of control words stored in the persistent queue-state buffer.
pub const QUEUE_STATE_WORDS: usize = 6;
/// Maximum device work-item claims accepted by one uploaded file batch.
pub const MAX_BATCH_WORK_ITEMS: usize = u32::MAX as usize;
/// Maximum sparse hit records accepted by one uploaded file batch.
pub const MAX_BATCH_HIT_CAPACITY: u32 = 16 * 1024 * 1024;

pub(crate) fn persistent_storage_binding_usage() -> wgpu::BufferUsages {
    wgpu::BufferUsages::STORAGE
        | wgpu::BufferUsages::COPY_SRC
        | wgpu::BufferUsages::COPY_DST
        | wgpu::BufferUsages::INDIRECT
}

/// Queue-state word indices.
pub mod queue_state_word {
    /// Next work-item index to claim.
    pub const HEAD: usize = 0;
    /// Total work items available in the queue.
    pub const QUEUE_LEN: usize = 1;
    /// Next sparse-hit slot to publish.
    pub const HIT_HEAD: usize = 2;
    /// Sparse-hit ring capacity.
    pub const HIT_CAPACITY: usize = 3;
    /// Total work items completed by the device.
    pub const DONE_COUNT: usize = 4;
    /// Rule fanout used to derive `(seg_idx, rule_idx)` from a claim id.
    /// `seg_idx = claim / rule_count` indexes the `segments` table, whose row
    /// `[file_idx, scan_start, emit_start, emit_end]` (file-relative) fully
    /// describes the window — no segmentation control words live here, the
    /// device decode reads the table directly (see `dispatcher.rs`).
    pub const RULE_COUNT: usize = 5;
}

/// Host-side file input for batch construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchFile {
    /// Stable hash of the file path.
    pub path_hash: u64,
    /// Decoded-layer index this file belongs to.
    pub decoded_layer_index: u32,
    /// Raw file bytes.
    pub bytes: Vec<u8>,
}

impl BatchFile {
    /// Build one batchable file record.
    #[must_use]
    pub fn new(path_hash: u64, decoded_layer_index: u32, bytes: Vec<u8>) -> Self {
        Self {
            path_hash,
            decoded_layer_index,
            bytes,
        }
    }
}

/// Per-file metadata mirrored into the device metadata table.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FileMetadata {
    /// Low 32 bits of the path hash.
    pub path_hash_lo: u32,
    /// High 32 bits of the path hash.
    pub path_hash_hi: u32,
    /// File byte length.
    pub size_bytes: u32,
    /// Decoded-layer index.
    pub decoded_layer_index: u32,
}

impl FileMetadata {
    fn from_file(file: &BatchFile) -> Result<Self, PipelineError> {
        Ok(Self {
            path_hash_lo: file.path_hash as u32,
            path_hash_hi: (file.path_hash >> 32) as u32,
            size_bytes: u32::try_from(file.bytes.len()).map_err(|_| PipelineError::QueueFull {
                queue: "submission",
                fix: "file size exceeds u32::MAX; split the batch into smaller files before megakernel batching",
            })?,
            decoded_layer_index: file.decoded_layer_index,
        })
    }
}

/// Device work item `(file_idx, rule_idx, layer_idx)`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WorkTriple {
    /// File-table index.
    pub file_idx: u32,
    /// Rule-table index.
    pub rule_idx: u32,
    /// Decoded-layer index.
    pub layer_idx: u32,
}

impl WorkTriple {
    /// Build one queue entry.
    #[must_use]
    pub const fn new(file_idx: u32, rule_idx: u32, layer_idx: u32) -> Self {
        Self {
            file_idx,
            rule_idx,
            layer_idx,
        }
    }
}

/// Sparse hit emitted by the batched kernel.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HitRecord {
    /// File-table index.
    pub file_idx: u32,
    /// Rule-table index.
    pub rule_idx: u32,
    /// Decoded-layer index.
    pub layer_idx: u32,
    /// Byte offset relative to the file start.
    pub match_offset: u32,
}

/// Persistent device-owned batch buffers.
#[derive(Clone)]
pub struct FileBatch {
    device_queue: Arc<(wgpu::Device, wgpu::Queue)>,
    file_metadata: Vec<FileMetadata>,
    file_offsets: Vec<u32>,
    haystack_words: Vec<u32>,
    rule_count: u32,
    queue_len: u32,
    hit_capacity: u32,
    haystack: GpuBufferHandle,
    offsets: GpuBufferHandle,
    metadata: GpuBufferHandle,
    /// Flat segment table: `segment_count * SEGMENT_WORDS` u32s, each row
    /// `[file_idx, scan_start, emit_start, emit_end]` (file-relative). The
    /// device claim decode reads `seg_idx = claim / rule_count` rows from here.
    segments: GpuBufferHandle,
    queue_state: GpuBufferHandle,
    hit_ring: GpuBufferHandle,
}

/// Telemetry for one in-place [`FileBatch`] refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileBatchRefreshReport {
    /// Host-to-device bytes written for refreshed logical input prefixes.
    pub bytes_uploaded: u64,
    /// New resident GPU allocations required because refreshed data exceeded
    /// existing allocation capacity.
    pub resident_allocations: u32,
    /// Resident buffers refreshed in place.
    pub reused_buffers: u32,
    /// Resident buffers replaced with new allocations.
    pub refreshed_buffers: u32,
}

impl std::fmt::Debug for FileBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileBatch")
            .field("file_count", &self.file_count())
            .field("queue_len", &self.queue_len())
            .field("haystack_bytes", &self.haystack.byte_len())
            .field("hit_capacity", &self.hit_capacity)
            .finish()
    }
}

impl FileBatch {
    /// Upload a new multi-file batch into persistent GPU buffers.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the batch exceeds the
    /// current `u32` table limits or the work queue would overflow.
    pub fn upload(
        device_queue: Arc<(wgpu::Device, wgpu::Queue)>,
        files: &[BatchFile],
        rule_count: u32,
        hit_capacity: u32,
    ) -> Result<Self, PipelineError> {
        validate_hit_capacity(hit_capacity)?;
        let (device, queue) = &*device_queue;
        validate_batch_shape(files, rule_count)?;
        let mut file_metadata = Vec::new();
        let mut file_offsets = Vec::new();
        let mut haystack_words = Vec::new();
        build_metadata_into(files, &mut file_metadata)?;
        build_offsets_into(files, &mut file_offsets)?;
        flatten_haystack_words_into(files, &mut haystack_words)?;
        let (seg_len, overlap) = default_segmentation(file_metadata.len());
        let segment_words = build_segment_table(&file_metadata, seg_len, overlap);
        let queue_len = segment_queue_len(&segment_words, rule_count)?;
        let queue_state_words = initial_queue_state(queue_len, hit_capacity, rule_count);

        let haystack = GpuBufferHandle::upload(
            device,
            queue,
            bytemuck::cast_slice(&haystack_words),
            persistent_storage_binding_usage(),
        )?;
        let offsets = GpuBufferHandle::upload(
            device,
            queue,
            bytemuck::cast_slice(&file_offsets),
            persistent_storage_binding_usage(),
        )?;
        let metadata = GpuBufferHandle::upload(
            device,
            queue,
            bytemuck::cast_slice(&file_metadata),
            persistent_storage_binding_usage(),
        )?;
        let segments = GpuBufferHandle::upload(
            device,
            queue,
            bytemuck::cast_slice(&segment_words),
            persistent_storage_binding_usage(),
        )?;
        let queue_state = GpuBufferHandle::upload(
            device,
            queue,
            bytemuck::cast_slice(&queue_state_words),
            persistent_storage_binding_usage(),
        )?;
        let hit_ring_bytes = hit_ring_byte_len(hit_capacity)?;
        let hit_ring =
            GpuBufferHandle::alloc(device, hit_ring_bytes, persistent_storage_binding_usage())?;

        Ok(Self {
            device_queue,
            file_metadata,
            file_offsets,
            haystack_words,
            rule_count,
            queue_len,
            hit_capacity,
            haystack,
            offsets,
            metadata,
            segments,
            queue_state,
            hit_ring,
        })
    }

    /// Refresh this batch in place, reusing host staging vectors and resident
    /// GPU buffers whenever the new batch fits existing allocations.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] before mutating the batch when the
    /// requested file/rule fanout cannot fit the megakernel batch protocol.
    pub fn refresh(
        &mut self,
        files: &[BatchFile],
        rule_count: u32,
        hit_capacity: u32,
    ) -> Result<(), PipelineError> {
        self.refresh_with_report(files, rule_count, hit_capacity)
            .map(|_| ())
    }

    /// Refresh this batch in place and return allocation/transfer telemetry.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] before mutating the batch when the
    /// requested file/rule fanout cannot fit the megakernel batch protocol.
    pub fn refresh_with_report(
        &mut self,
        files: &[BatchFile],
        rule_count: u32,
        hit_capacity: u32,
    ) -> Result<FileBatchRefreshReport, PipelineError> {
        validate_hit_capacity(hit_capacity)?;
        validate_batch_shape(files, rule_count)?;

        build_metadata_into(files, &mut self.file_metadata)?;
        build_offsets_into(files, &mut self.file_offsets)?;
        flatten_haystack_words_into(files, &mut self.haystack_words)?;

        let (seg_len, overlap) = default_segmentation(self.file_metadata.len());
        let segment_words = build_segment_table(&self.file_metadata, seg_len, overlap);
        let queue_len = segment_queue_len(&segment_words, rule_count)?;
        self.rule_count = rule_count;
        self.queue_len = queue_len;
        self.hit_capacity = hit_capacity;
        let queue_state_words = initial_queue_state(queue_len, hit_capacity, rule_count);
        let (device, queue) = &*self.device_queue;
        let mut report = FileBatchRefreshReport::default();
        accumulate_refresh(
            &mut report,
            &mut self.haystack,
            device,
            queue,
            bytemuck::cast_slice(&self.haystack_words),
            persistent_storage_binding_usage(),
        )?;
        accumulate_refresh(
            &mut report,
            &mut self.offsets,
            device,
            queue,
            bytemuck::cast_slice(&self.file_offsets),
            persistent_storage_binding_usage(),
        )?;
        accumulate_refresh(
            &mut report,
            &mut self.metadata,
            device,
            queue,
            bytemuck::cast_slice(&self.file_metadata),
            persistent_storage_binding_usage(),
        )?;
        accumulate_refresh(
            &mut report,
            &mut self.segments,
            device,
            queue,
            bytemuck::cast_slice(&segment_words),
            persistent_storage_binding_usage(),
        )?;
        accumulate_refresh(
            &mut report,
            &mut self.queue_state,
            device,
            queue,
            bytemuck::cast_slice(&queue_state_words),
            persistent_storage_binding_usage(),
        )?;
        let hit_ring_bytes = hit_ring_byte_len(hit_capacity)?;
        if self.hit_ring.allocation_len() < padded_write_len_u64(hit_ring_bytes)? {
            self.hit_ring =
                GpuBufferHandle::alloc(device, hit_ring_bytes, persistent_storage_binding_usage())?;
            report.resident_allocations += 1;
            report.refreshed_buffers += 1;
        } else {
            report.reused_buffers += 1;
        }
        Ok(report)
    }

    /// Reset the persistent queue indices before another dispatch.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] when the queue-state upload fails.
    pub fn reset_queue_state(&self) -> Result<(), PipelineError> {
        let (_, queue) = &*self.device_queue;
        let words = initial_queue_state(self.queue_len, self.hit_capacity, self.rule_count);
        queue.write_buffer(self.queue_state.buffer(), 0, bytemuck::cast_slice(&words));
        Ok(())
    }

    /// Re-tile this batch at a new window geometry: rebuild the `segments` table
    /// (each file split into `ceil(len / seg_len)` windows, files shorter than
    /// `seg_len` staying whole), grow/rewrite the resident segments buffer, and
    /// update `queue_len = segment_count * rule_count` + the queue-state header so
    /// the next dispatch claims the new work space. `seg_len = u32::MAX` restores
    /// one segment per file (the legacy whole-file scan).
    ///
    /// SOUNDNESS CONTRACT: `overlap` MUST be at least
    /// [`segmentation::catalog_sync_overlap`] over the rules that will scan this
    /// batch. With a shorter warm-up a window can reconstruct the wrong DFA state
    /// at `emit_start` and drop or fabricate matches. The geometry is the
    /// caller's decision (it owns the rule catalog); this method enforces only the
    /// structural invariant `seg_len > 0`.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when `seg_len == 0` or the new work
    /// queue overflows the device claim protocol, or [`PipelineError::Backend`]
    /// when the buffer upload fails.
    pub fn set_segmentation(&mut self, seg_len: u32, overlap: u32) -> Result<(), PipelineError> {
        if seg_len == 0 {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "segment owned-width (seg_len) must be > 0; pass u32::MAX for one segment per file",
            });
        }
        let segment_words = build_segment_table(&self.file_metadata, seg_len, overlap);
        let queue_len = segment_queue_len(&segment_words, self.rule_count)?;
        self.queue_len = queue_len;
        let queue_state_words = initial_queue_state(queue_len, self.hit_capacity, self.rule_count);
        let (device, queue) = &*self.device_queue;
        let mut report = FileBatchRefreshReport::default();
        accumulate_refresh(
            &mut report,
            &mut self.segments,
            device,
            queue,
            bytemuck::cast_slice(&segment_words),
            persistent_storage_binding_usage(),
        )?;
        queue.write_buffer(
            self.queue_state.buffer(),
            0,
            bytemuck::cast_slice(&queue_state_words),
        );
        Ok(())
    }

    /// Number of files in the batch.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.file_metadata.len()
    }

    /// Number of queued `(file, rule, layer)` items.
    #[must_use]
    pub const fn queue_len(&self) -> u32 {
        self.queue_len
    }

    /// Sparse-hit capacity.
    #[must_use]
    pub const fn hit_capacity(&self) -> u32 {
        self.hit_capacity
    }

    /// Device queue used for every buffer in this batch.
    #[must_use]
    pub fn device_queue(&self) -> Arc<(wgpu::Device, wgpu::Queue)> {
        Arc::clone(&self.device_queue)
    }

    /// Packed haystack buffer.
    #[must_use]
    pub const fn haystack(&self) -> &GpuBufferHandle {
        &self.haystack
    }

    /// Prefix-sum offset table. Length = `file_count + 1`.
    #[must_use]
    pub const fn offsets(&self) -> &GpuBufferHandle {
        &self.offsets
    }

    /// Per-file metadata table.
    #[must_use]
    pub const fn metadata(&self) -> &GpuBufferHandle {
        &self.metadata
    }

    /// Flat segment table (`segment_count * SEGMENT_WORDS` u32s). The device
    /// claim decode reads row `seg_idx = claim / rule_count` to derive the
    /// window `(file_idx, scan_start, emit_start, emit_end)`.
    #[must_use]
    pub const fn segments(&self) -> &GpuBufferHandle {
        &self.segments
    }

    /// Queue-state/control words.
    #[must_use]
    pub const fn queue_state(&self) -> &GpuBufferHandle {
        &self.queue_state
    }

    /// Sparse output ring.
    #[must_use]
    pub const fn hit_ring(&self) -> &GpuBufferHandle {
        &self.hit_ring
    }

    /// Host-side file metadata.
    #[must_use]
    pub fn host_metadata(&self) -> &[FileMetadata] {
        &self.file_metadata
    }

    /// Host-side prefix offsets.
    #[must_use]
    pub fn host_offsets(&self) -> &[u32] {
        &self.file_offsets
    }

    /// Host-side dense work queue.
    ///
    /// Dense batches derive work items on-device, so there are no host
    /// materialized triples to expose.
    #[must_use]
    pub fn host_work_items(&self) -> &[WorkTriple] {
        &[]
    }
}

/// Device-resident batch for the COMBINED Aho-Corasick segmented megakernel.
///
/// Where [`FileBatch`] uploads per-rule DFA tables and runs one work item per
/// `(segment, rule)`, `CombinedBatch` uploads ONE flattened combined automaton
/// (`transitions` / `output_offsets` / `output_records`) and runs one work item
/// per segment (`queue_len = segment_count`). It reuses every FileBatch host
/// primitive (metadata, offsets, haystack packing, segment tiling, queue-state
/// header, sparse hit ring) verbatim; only the automaton buffers and the work
/// dimension differ.
///
/// LAYERING: this type takes the automaton as RAW flattened `u32` arrays — it
/// holds NO pattern / Aho-Corasick knowledge. Building the automaton from
/// patterns (`vyre_libs::scan::classic_ac::classic_ac_compile`) lives in the
/// caller, because `vyre-libs` sits ABOVE this crate in the dependency graph.
///
/// SOUNDNESS: the segment warm-up `overlap` is fixed to `max_pattern_len` (a
/// match ending at a window's `emit_start` can begin up to `max_pattern_len-1`
/// bytes earlier, so the window must rescan that prefix to reconstruct the DFA
/// state). This is the overlap the `segmentation.rs` `combined_segmented_scan`
/// CPU oracle proves equal to a linear `classic_ac_scan`. It is NOT caller-
/// tunable: the automaton determines it.
#[derive(Clone)]
pub struct CombinedBatch {
    device_queue: Arc<(wgpu::Device, wgpu::Queue)>,
    file_metadata: Vec<FileMetadata>,
    queue_len: u32,
    hit_capacity: u32,
    max_pattern_len: u32,
    /// Byte-class count of the compressed transition table (`<= 256`). Baked
    /// into the kernel program (the automaton fixes it), so the pipeline is
    /// compiled once per resident catalog.
    num_classes: u32,
    /// Device packing of each transition target (`Bits32` default, or `Bits16`
    /// two-per-word). The dispatcher reads this to build a matching kernel.
    transition_width: TransitionWidth,
    haystack: GpuBufferHandle,
    offsets: GpuBufferHandle,
    metadata: GpuBufferHandle,
    /// Byte-class compressed transition table (`state_count * num_classes`),
    /// packed per [`CombinedBatch::transition_width`].
    transitions: GpuBufferHandle,
    output_offsets: GpuBufferHandle,
    output_records: GpuBufferHandle,
    /// 256-entry byte→class map.
    class_maps: GpuBufferHandle,
    segments: GpuBufferHandle,
    queue_state: GpuBufferHandle,
    hit_ring: GpuBufferHandle,
}

impl std::fmt::Debug for CombinedBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CombinedBatch")
            .field("file_count", &self.file_metadata.len())
            .field("queue_len", &self.queue_len)
            .field("hit_capacity", &self.hit_capacity)
            .field("max_pattern_len", &self.max_pattern_len)
            .finish()
    }
}

impl CombinedBatch {
    /// Upload a combined-AC batch into persistent GPU buffers.
    ///
    /// `transitions` is the dense `state_count * 256` byte→next-state table,
    /// `output_offsets` the `state_count + 1` CSR row pointers, and
    /// `output_records` the flat pattern-id payload. `max_pattern_len` is the
    /// longest pattern in the automaton (the warm-up overlap). `seg_len` is the
    /// per-segment owned width.
    ///
    /// # Throughput — `seg_len` is the device-saturation lever, choose it
    ///
    /// `seg_len = u32::MAX` is one segment per file: a single serial DFA walk
    /// with NO intra-file parallelism. It is correctness-equivalent to the
    /// pre-segmentation path but leaves the device almost entirely idle —
    /// ~0.01x Hyperscan on an 8 MiB input on the reference RTX 5090. It is a
    /// correctness floor, never a performance default; passing it (or any
    /// coarse `seg_len`) silently lands the caller on the slow path.
    ///
    /// Tiling the file into many overlapping windows is what saturates the
    /// device. Throughput climbs as `seg_len` shrinks (more resident windows)
    /// until the fixed `overlap = max_pattern_len` warm-up per window starts to
    /// dominate, then turns over. On the reference RTX 5090 the 8 MiB optimum is
    /// `seg_len ~= 128` (~15.6x Hyperscan for a 32-literal catalog, ~8.5x at
    /// 2048 literals), turning over by 64 — see the `megakernel_combined_scan`
    /// geometry sweep, which pins the conservation + throughput contract across
    /// `u32::MAX..=64`. The exact optimum shifts with device core count and
    /// catalog, so tune per host; but ANY fine window beats whole-file by orders
    /// of magnitude, so never ship `u32::MAX` as the production geometry.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the batch exceeds `u32` table
    /// limits or the work queue overflows, or [`PipelineError::Backend`] when
    /// the automaton arrays are internally inconsistent (a malformed automaton
    /// is rejected, never silently scanned).
    #[allow(clippy::too_many_arguments)]
    pub fn upload(
        device_queue: Arc<(wgpu::Device, wgpu::Queue)>,
        files: &[BatchFile],
        transitions: &[u32],
        output_offsets: &[u32],
        output_records: &[u32],
        state_count: u32,
        max_pattern_len: u32,
        seg_len: u32,
        hit_capacity: u32,
    ) -> Result<Self, PipelineError> {
        Self::upload_with_transition_width(
            device_queue,
            files,
            transitions,
            output_offsets,
            output_records,
            state_count,
            max_pattern_len,
            seg_len,
            hit_capacity,
            TransitionWidth::Bits32,
        )
    }

    /// Like [`CombinedBatch::upload`] but choosing the device transition-table
    /// packing. `TransitionWidth::Bits16` halves the table and bytes-per-
    /// transaction (the keyhog-scale L1 lever) and FAILS CLOSED if any target
    /// exceeds `u16::MAX` — never silently truncates a next-state (Law 10).
    ///
    /// # Errors
    ///
    /// As [`CombinedBatch::upload`], plus [`PipelineError::Backend`] when
    /// `Bits16` is requested but a transition target does not fit `u16`.
    #[allow(clippy::too_many_arguments)]
    pub fn upload_with_transition_width(
        device_queue: Arc<(wgpu::Device, wgpu::Queue)>,
        files: &[BatchFile],
        transitions: &[u32],
        output_offsets: &[u32],
        output_records: &[u32],
        state_count: u32,
        max_pattern_len: u32,
        seg_len: u32,
        hit_capacity: u32,
        transition_width: TransitionWidth,
    ) -> Result<Self, PipelineError> {
        validate_hit_capacity(hit_capacity)?;
        validate_combined_automaton(
            transitions,
            output_offsets,
            output_records,
            state_count,
        )?;
        if seg_len == 0 {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "segment owned-width (seg_len) must be > 0; pass u32::MAX for one segment per file",
            });
        }
        let (device, queue) = &*device_queue;
        let mut file_metadata = Vec::new();
        let mut file_offsets = Vec::new();
        let mut haystack_words = Vec::new();
        build_metadata_into(files, &mut file_metadata)?;
        build_offsets_into(files, &mut file_offsets)?;
        flatten_haystack_words_into(files, &mut haystack_words)?;
        let segment_words = build_segment_table(&file_metadata, seg_len, max_pattern_len);
        // One work item per segment: rule_count == 1 in the dense primitive.
        let queue_len = segment_queue_len(&segment_words, 1)?;
        let queue_state_words = initial_queue_state(queue_len, hit_capacity, 1);

        // LOSSLESS byte-class compression of the dense combined transition table,
        // reusing the SAME primitive the per-rule catalog packer uses (so the
        // "identical column ⇒ same class" contract has one owner). Shrinks the
        // table from `state_count * 256` to `state_count * num_classes`, which
        // is what keeps a thousand-state catalog resident in L2.
        let mut class_map = Vec::new();
        let num_classes = vyre_runtime::megakernel::rule_catalog::build_byte_class_map_for_table(
            transitions,
            state_count as usize,
            &mut class_map,
        );
        let mut compressed_transitions = Vec::new();
        vyre_runtime::megakernel::rule_catalog::compress_dense_transitions_into(
            transitions,
            state_count as usize,
            &class_map,
            num_classes,
            &mut compressed_transitions,
        );
        // Bits16 packs two targets per word (halving bytes/transaction) and FAILS
        // CLOSED on any target > u16::MAX — never silently truncates (Law 10). The
        // kernel built by the dispatcher unpacks to match `transition_width`.
        let transition_table = match transition_width {
            TransitionWidth::Bits32 => compressed_transitions,
            TransitionWidth::Bits16 => {
                let mut packed = Vec::new();
                vyre_runtime::megakernel::rule_catalog::try_pack_u16_transitions_into(
                    &compressed_transitions,
                    &mut packed,
                )?;
                packed
            }
        };

        let usage = persistent_storage_binding_usage();
        let haystack =
            GpuBufferHandle::upload(device, queue, bytemuck::cast_slice(&haystack_words), usage)?;
        let offsets =
            GpuBufferHandle::upload(device, queue, bytemuck::cast_slice(&file_offsets), usage)?;
        let metadata =
            GpuBufferHandle::upload(device, queue, bytemuck::cast_slice(&file_metadata), usage)?;
        let transitions = GpuBufferHandle::upload(
            device,
            queue,
            bytemuck::cast_slice(&transition_table),
            usage,
        )?;
        let output_offsets =
            GpuBufferHandle::upload(device, queue, bytemuck::cast_slice(output_offsets), usage)?;
        let output_records =
            GpuBufferHandle::upload(device, queue, bytemuck::cast_slice(output_records), usage)?;
        let class_maps =
            GpuBufferHandle::upload(device, queue, bytemuck::cast_slice(&class_map), usage)?;
        let segments =
            GpuBufferHandle::upload(device, queue, bytemuck::cast_slice(&segment_words), usage)?;
        let queue_state =
            GpuBufferHandle::upload(device, queue, bytemuck::cast_slice(&queue_state_words), usage)?;
        let hit_ring_bytes = hit_ring_byte_len(hit_capacity)?;
        let hit_ring = GpuBufferHandle::alloc(device, hit_ring_bytes, usage)?;

        Ok(Self {
            device_queue,
            file_metadata,
            queue_len,
            hit_capacity,
            max_pattern_len,
            num_classes,
            transition_width,
            haystack,
            offsets,
            metadata,
            transitions,
            output_offsets,
            output_records,
            class_maps,
            segments,
            queue_state,
            hit_ring,
        })
    }

    /// Re-tile this batch at a new segment owned-width, keeping the proven
    /// `overlap = max_pattern_len`. `seg_len = u32::MAX` restores one segment
    /// per file.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when `seg_len == 0` or the new work
    /// queue overflows, or [`PipelineError::Backend`] on upload failure.
    pub fn set_segmentation(&mut self, seg_len: u32) -> Result<(), PipelineError> {
        if seg_len == 0 {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "segment owned-width (seg_len) must be > 0; pass u32::MAX for one segment per file",
            });
        }
        let segment_words = build_segment_table(&self.file_metadata, seg_len, self.max_pattern_len);
        let queue_len = segment_queue_len(&segment_words, 1)?;
        self.queue_len = queue_len;
        let queue_state_words = initial_queue_state(queue_len, self.hit_capacity, 1);
        let (device, queue) = &*self.device_queue;
        let mut report = FileBatchRefreshReport::default();
        accumulate_refresh(
            &mut report,
            &mut self.segments,
            device,
            queue,
            bytemuck::cast_slice(&segment_words),
            persistent_storage_binding_usage(),
        )?;
        queue.write_buffer(
            self.queue_state.buffer(),
            0,
            bytemuck::cast_slice(&queue_state_words),
        );
        Ok(())
    }

    /// Reset the persistent queue indices before another dispatch.
    pub fn reset_queue_state(&self) {
        let (_, queue) = &*self.device_queue;
        let words = initial_queue_state(self.queue_len, self.hit_capacity, 1);
        queue.write_buffer(self.queue_state.buffer(), 0, bytemuck::cast_slice(&words));
    }

    /// Device work-queue length (== segment count).
    #[must_use]
    pub const fn queue_len(&self) -> u32 {
        self.queue_len
    }

    /// Sparse hit-ring capacity.
    #[must_use]
    pub const fn hit_capacity(&self) -> u32 {
        self.hit_capacity
    }

    /// Shared device + queue handle.
    #[must_use]
    pub fn device_queue(&self) -> Arc<(wgpu::Device, wgpu::Queue)> {
        Arc::clone(&self.device_queue)
    }

    /// Byte-class count of the compressed transition table (`<= 256`), baked
    /// into the kernel program.
    #[must_use]
    pub const fn num_classes(&self) -> u32 {
        self.num_classes
    }

    /// Device packing of each transition target. The dispatcher MUST build the
    /// kernel with this width so the unpack matches the uploaded table.
    #[must_use]
    pub const fn transition_width(&self) -> TransitionWidth {
        self.transition_width
    }

    /// Read-only input buffers in `combined_batch_program_buffers` declaration
    /// order (file_offsets, file_metadata, haystack, transitions,
    /// output_offsets, output_records, class_maps, segments). The persistent
    /// pipeline binds inputs positionally in this order.
    #[must_use]
    pub fn input_buffers(&self) -> [&GpuBufferHandle; 8] {
        [
            &self.offsets,
            &self.metadata,
            &self.haystack,
            &self.transitions,
            &self.output_offsets,
            &self.output_records,
            &self.class_maps,
            &self.segments,
        ]
    }

    /// Writable buffers in declaration order (queue_state, then the hit_ring
    /// output).
    #[must_use]
    pub const fn output_buffers(&self) -> [&GpuBufferHandle; 2] {
        [&self.queue_state, &self.hit_ring]
    }

    /// Persistent queue-state buffer (control words).
    #[must_use]
    pub const fn queue_state(&self) -> &GpuBufferHandle {
        &self.queue_state
    }

    /// Sparse hit-ring buffer.
    #[must_use]
    pub const fn hit_ring(&self) -> &GpuBufferHandle {
        &self.hit_ring
    }
}

/// Reject a structurally inconsistent combined automaton at the boundary rather
/// than uploading tables the kernel would index out of range (Law 10: fail
/// closed, never silently scan a malformed automaton).
fn validate_combined_automaton(
    transitions: &[u32],
    output_offsets: &[u32],
    output_records: &[u32],
    state_count: u32,
) -> Result<(), PipelineError> {
    if state_count == 0 {
        return Err(PipelineError::Backend(
            "combined automaton has state_count 0. Fix: compile at least one pattern before upload."
                .to_string(),
        ));
    }
    let state_count_usize = state_count as usize;
    let expected_transitions = state_count_usize.checked_mul(256).ok_or_else(|| {
        PipelineError::Backend(
            "combined transition table size (state_count * 256) overflowed usize. Fix: shard the pattern catalog."
                .to_string(),
        )
    })?;
    if transitions.len() != expected_transitions {
        return Err(PipelineError::Backend(format!(
            "combined transition table has {} words, expected state_count*256 = {expected_transitions}. Fix: pass the dense classic-AC transition table.",
            transitions.len()
        )));
    }
    if output_offsets.len() != state_count_usize + 1 {
        return Err(PipelineError::Backend(format!(
            "combined output_offsets has {} entries, expected state_count+1 = {}. Fix: pass the CSR row-pointer array.",
            output_offsets.len(),
            state_count_usize + 1
        )));
    }
    let last_offset = *output_offsets.last().expect("non-empty: checked above") as usize;
    if last_offset != output_records.len() {
        return Err(PipelineError::Backend(format!(
            "combined output_offsets terminator {last_offset} != output_records length {}. Fix: keep the CSR payload and row pointers consistent.",
            output_records.len()
        )));
    }
    Ok(())
}

fn accumulate_refresh(
    report: &mut FileBatchRefreshReport,
    handle: &mut GpuBufferHandle,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bytes: &[u8],
    usage: wgpu::BufferUsages,
) -> Result<(), PipelineError> {
    let refreshed = upload_or_refresh(handle, device, queue, bytes, usage)?;
    let padded_len = padded_write_len(bytes.len())?;
    report.bytes_uploaded = report.bytes_uploaded.checked_add(padded_len).ok_or_else(|| {
        PipelineError::Backend(
            "batch refresh uploaded-byte accounting overflowed u64. Fix: shard the file batch before GPU upload."
                .to_string(),
        )
    })?;
    if refreshed {
        report.resident_allocations += 1;
        report.refreshed_buffers += 1;
    } else {
        report.reused_buffers += 1;
    }
    Ok(())
}

fn upload_or_refresh(
    handle: &mut GpuBufferHandle,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bytes: &[u8],
    usage: wgpu::BufferUsages,
) -> Result<bool, PipelineError> {
    let required_len = padded_write_len(bytes.len())?;
    if handle.allocation_len() >= required_len
        && handle
            .usage()
            .contains(usage | wgpu::BufferUsages::COPY_DST)
    {
        write_padded_prefix(queue, handle.buffer(), bytes)?;
        Ok(false)
    } else {
        *handle = GpuBufferHandle::upload(device, queue, bytes, usage)?;
        Ok(true)
    }
}

fn padded_write_len(len: usize) -> Result<u64, PipelineError> {
    if len == 0 {
        return Ok(0);
    }
    let normalized = len.max(4);
    let remainder = normalized % 4;
    let padded = if remainder == 0 {
        normalized
    } else {
        normalized.checked_add(4 - remainder).ok_or_else(|| {
            PipelineError::Backend(
                "refreshed batch buffer length overflows usize while padding to WGPU alignment. Fix: split the batch before upload.".to_string(),
            )
        })?
    };
    u64::try_from(padded).map_err(|source| {
        PipelineError::Backend(format!(
            "refreshed batch buffer length cannot fit u64: {source}. Fix: split the batch before upload."
        ))
    })
}

fn padded_write_len_u64(len: u64) -> Result<u64, PipelineError> {
    if len == 0 {
        return Ok(0);
    }
    let normalized = len.max(4);
    let remainder = normalized % 4;
    if remainder == 0 {
        return Ok(normalized);
    }
    normalized.checked_add(4 - remainder).ok_or_else(|| {
        PipelineError::Backend(
            "refreshed batch buffer length overflows u64 while padding to WGPU alignment. Fix: split the batch before upload.".to_string(),
        )
    })
}

fn usize_to_u64(value: usize, label: &str) -> Result<u64, PipelineError> {
    u64::try_from(value).map_err(|source| {
        PipelineError::Backend(format!(
            "{label} cannot fit u64: {source}. Fix: split the batch before upload."
        ))
    })
}

fn hit_ring_byte_len(hit_capacity: u32) -> Result<u64, PipelineError> {
    u64::from(hit_capacity)
        .checked_mul(usize_to_u64(HIT_RECORD_WORDS, "hit record word count")?)
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| {
            PipelineError::Backend(
                "hit-ring allocation byte count overflowed u64. Fix: reduce hit_capacity or shard the batch."
                    .to_string(),
            )
        })
}

fn write_padded_prefix(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    bytes: &[u8],
) -> Result<(), PipelineError> {
    crate::padded_upload::write_padded_prefix(
        queue,
        buffer,
        bytes,
        "padded batch write tail offset",
    )
    .map_err(|source| PipelineError::Backend(source.to_string()))?;
    Ok(())
}

fn validate_batch_shape(files: &[BatchFile], rule_count: u32) -> Result<(), PipelineError> {
    if u32::try_from(files.len()).is_err() {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "file count exceeds u32::MAX; split the batch into smaller file shards",
        });
    }
    let mut total_bytes = 0u64;
    for file in files {
        if u32::try_from(file.bytes.len()).is_err() {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "file size exceeds u32::MAX; split the batch into smaller files before megakernel batching",
            });
        }
        let file_len = usize_to_u64(file.bytes.len(), "batched file byte length")?;
        total_bytes = total_bytes
            .checked_add(file_len)
            .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix: "batched haystack length overflowed u64; split the batch into smaller shards",
            })?;
    }
    if total_bytes > u64::from(u32::MAX) {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "batched haystack exceeds u32::MAX bytes; split the batch into smaller shards",
        });
    }
    validate_work_queue_shape(files.len(), rule_count)
}

fn dense_queue_len(file_count: usize, rule_count: u32) -> Result<u32, PipelineError> {
    let rule_count = usize::try_from(rule_count).map_err(|_| PipelineError::QueueFull {
        queue: "submission",
        fix: "rule count cannot fit host usize; shard the rule set before dense queue planning",
    })?;
    let capacity = file_count
        .checked_mul(rule_count)
        .ok_or(PipelineError::QueueFull {
        queue: "submission",
        fix: "file_count * rule_count overflowed usize; split the batch or reduce the rule fanout",
    })?;
    if u32::try_from(capacity).is_err() {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "work queue length exceeds u32::MAX; split the batch or reduce the rule fanout before allocation",
        });
    }
    if capacity > MAX_BATCH_WORK_ITEMS {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "work queue length exceeds the device claim protocol; split the file batch or reduce the rule fanout",
        });
    }
    u32::try_from(capacity).map_err(|_| PipelineError::QueueFull {
        queue: "submission",
        fix: "work queue length exceeds u32::MAX; split the batch or reduce the rule fanout before allocation",
    })
}

fn validate_work_queue_shape(file_count: usize, rule_count: u32) -> Result<(), PipelineError> {
    dense_queue_len(file_count, rule_count).map(|_| ())
}

fn build_metadata_into(
    files: &[BatchFile],
    metadata: &mut Vec<FileMetadata>,
) -> Result<(), PipelineError> {
    reserve_batch_vec_len(metadata, files.len(), "file metadata records")?;
    if metadata.len() == files.len() {
        for (slot, file) in metadata.iter_mut().zip(files) {
            *slot = FileMetadata::from_file(file)?;
        }
    } else {
        metadata.clear();
        for file in files {
            metadata.push(FileMetadata::from_file(file)?);
        }
    }
    Ok(())
}

#[cfg(test)]
fn build_offsets(files: &[BatchFile]) -> Result<Vec<u32>, PipelineError> {
    let mut offsets = Vec::new();
    build_offsets_into(files, &mut offsets)?;
    Ok(offsets)
}

fn build_offsets_into(files: &[BatchFile], offsets: &mut Vec<u32>) -> Result<(), PipelineError> {
    let required = files.len().checked_add(1).ok_or(PipelineError::QueueFull {
        queue: "submission",
        fix: "file count overflows offset table length; split the batch before upload",
    })?;
    reserve_batch_vec_len(offsets, required, "file offset table")?;
    let stable_len = offsets.len() == required;
    if stable_len {
        offsets[0] = 0;
    } else {
        offsets.clear();
        offsets.push(0);
    }
    let mut total = 0u64;
    for (index, file) in files.iter().enumerate() {
        let file_len = usize_to_u64(file.bytes.len(), "batched file byte length")?;
        total = total
            .checked_add(file_len)
            .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix: "batched haystack length overflowed u64; split the batch into smaller shards",
            })?;
        let offset = u32::try_from(total).map_err(|_| PipelineError::QueueFull {
            queue: "submission",
            fix: "batched haystack exceeds u32::MAX bytes; split the batch into smaller shards",
        })?;
        if stable_len {
            offsets[index + 1] = offset;
        } else {
            offsets.push(offset);
        }
    }
    Ok(())
}

#[cfg(test)]
fn flatten_haystack_words(files: &[BatchFile]) -> Result<Vec<u32>, PipelineError> {
    let mut words = Vec::new();
    flatten_haystack_words_into(files, &mut words)?;
    Ok(words)
}

fn flatten_haystack_words_into(
    files: &[BatchFile],
    words: &mut Vec<u32>,
) -> Result<(), PipelineError> {
    let total = files.iter().try_fold(0usize, |acc, file| {
        acc.checked_add(file.bytes.len())
            .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix:
                    "batched haystack length overflowed usize; split the batch into smaller shards",
            })
    })?;
    let target_words = total.div_ceil(4).max(1);
    reserve_batch_vec_len(words, target_words, "packed haystack words")?;
    let stable_len = words.len() == target_words;
    if stable_len {
        words.fill(0);
    } else {
        words.clear();
    }
    let mut word = 0u32;
    let mut shift = 0u32;
    let mut word_index = 0usize;
    for file in files {
        pack_bytes_into_words(
            &file.bytes,
            words,
            stable_len,
            &mut word_index,
            &mut word,
            &mut shift,
        );
    }
    if shift != 0 {
        write_packed_word(words, stable_len, &mut word_index, word);
    }
    if word_index == 0 {
        write_packed_word(words, stable_len, &mut word_index, 0);
    }
    Ok(())
}

fn pack_bytes_into_words(
    bytes: &[u8],
    words: &mut Vec<u32>,
    stable_len: bool,
    word_index: &mut usize,
    word: &mut u32,
    shift: &mut u32,
) {
    let mut cursor = bytes;
    if *shift != 0 {
        while *shift != 0 && !cursor.is_empty() {
            *word |= u32::from(cursor[0]) << *shift;
            *shift += 8;
            cursor = &cursor[1..];
            if *shift == 32 {
                write_packed_word(words, stable_len, word_index, *word);
                *word = 0;
                *shift = 0;
            }
        }
    }

    let mut chunks = cursor.chunks_exact(4);
    for chunk in &mut chunks {
        write_packed_word(
            words,
            stable_len,
            word_index,
            u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
        );
    }
    for byte in chunks.remainder() {
        *word |= u32::from(*byte) << *shift;
        *shift += 8;
    }
}

fn write_packed_word(words: &mut Vec<u32>, stable_len: bool, word_index: &mut usize, word: u32) {
    if stable_len {
        words[*word_index] = word;
    } else {
        words.push(word);
    }
    *word_index += 1;
}

#[cfg(test)]
fn derive_work_triple(
    file_metadata: &[FileMetadata],
    rule_count: u32,
    claim: u32,
) -> Result<WorkTriple, PipelineError> {
    let queue_len = dense_queue_len(file_metadata.len(), rule_count)?;
    if claim >= queue_len || rule_count == 0 {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "claim exceeds the dense device-derived queue; keep queue length and rule count synchronized",
        });
    }
    let file_idx = claim / rule_count;
    let rule_idx = claim % rule_count;
    let file_idx_usize = usize::try_from(file_idx).map_err(|_| PipelineError::QueueFull {
        queue: "submission",
        fix: "derived file index cannot fit host usize; shard the batch before decoding work triples",
    })?;
    let metadata = file_metadata
        .get(file_idx_usize)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "derived file index exceeds metadata length; keep queue length and metadata synchronized",
        })?;
    Ok(WorkTriple::new(
        file_idx,
        rule_idx,
        metadata.decoded_layer_index,
    ))
}

fn validate_hit_capacity(hit_capacity: u32) -> Result<(), PipelineError> {
    if hit_capacity > MAX_BATCH_HIT_CAPACITY {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "hit capacity exceeds the per-batch sparse ring cap; shard the batch or drain hits across multiple launches",
        });
    }
    Ok(())
}

fn initial_queue_state(
    queue_len: u32,
    hit_capacity: u32,
    rule_count: u32,
) -> [u32; QUEUE_STATE_WORDS] {
    // Word order MUST match `queue_state_word::*`:
    // [HEAD, QUEUE_LEN, HIT_HEAD, HIT_CAPACITY, DONE_COUNT, RULE_COUNT].
    [0, queue_len, 0, hit_capacity, 0, rule_count]
}

/// Default (non-tuned) window geometry: `seg_len = u32::MAX` and `overlap = 0`,
/// which yields exactly ONE segment per file. The resulting `segments` table is
/// `[file_idx, 0, 0, file_len]` per file, so the device decode scans each file
/// whole from state 0 with no warm-up — byte-for-byte the pre-segmentation
/// behavior. Tuning `seg_len` below the file length saturates the device, but is
/// only sound once `overlap >= the catalog's longest match span`; that geometry
/// is chosen by the caller, never defaulted here.
fn default_segmentation(_file_count: usize) -> (u32, u32) {
    (u32::MAX, 0)
}

/// Build the flat device segment table from the per-file metadata at the given
/// window geometry. The row order matches [`segmentation::plan_segments`], so the
/// device `seg_idx = claim / rule_count` indexes it directly.
fn build_segment_table(file_metadata: &[FileMetadata], seg_len: u32, overlap: u32) -> Vec<u32> {
    let file_lens: Vec<u32> = file_metadata.iter().map(|meta| meta.size_bytes).collect();
    segmentation::segment_table(&file_lens, seg_len, overlap)
}

/// Work-queue length for a segment table = `segment_count * rule_count`, reusing
/// the same overflow/limit guards as the dense path (`dense_queue_len` is the
/// `count * rule_count` primitive; here `count` is the segment count, not the
/// file count). A zero-length table (every file empty) yields a zero queue.
fn segment_queue_len(segment_words: &[u32], rule_count: u32) -> Result<u32, PipelineError> {
    let segment_count = segment_words.len() / segmentation::SEGMENT_WORDS;
    dense_queue_len(segment_count, rule_count)
}

fn reserve_batch_vec_len<T>(
    vec: &mut Vec<T>,
    target_len: usize,
    label: &'static str,
) -> Result<(), PipelineError> {
    reserve_vec_exact_for_len(
        vec,
        target_len,
        "megakernel FileBatch staging",
        label,
        "split the file batch or reduce rule fanout before upload",
    )
    .map_err(|error| PipelineError::Backend(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_are_prefix_sums() {
        let files = vec![
            BatchFile::new(1, 0, b"ab".to_vec()),
            BatchFile::new(2, 3, b"cdef".to_vec()),
        ];
        assert_eq!(build_offsets(&files).unwrap(), vec![0, 2, 6]);
    }

    #[test]
    fn haystack_flattening_preserves_cross_file_byte_order() {
        let files = vec![
            BatchFile::new(1, 0, vec![1, 2, 3]),
            BatchFile::new(2, 0, vec![4, 5, 6, 7, 8]),
            BatchFile::new(3, 0, vec![9]),
        ];

        let words = flatten_haystack_words(&files).unwrap();

        assert_eq!(
            words,
            vec![
                u32::from_le_bytes([1, 2, 3, 4]),
                u32::from_le_bytes([5, 6, 7, 8]),
                u32::from_le_bytes([9, 0, 0, 0]),
            ]
        );
    }

    #[test]
    fn device_schedule_derives_files_x_rules_without_materialized_queue() {
        let metadata = vec![
            FileMetadata {
                path_hash_lo: 1,
                path_hash_hi: 0,
                size_bytes: 2,
                decoded_layer_index: 4,
            },
            FileMetadata {
                path_hash_lo: 2,
                path_hash_hi: 0,
                size_bytes: 3,
                decoded_layer_index: 9,
            },
        ];
        let queue = (0..dense_queue_len(metadata.len(), 2).unwrap())
            .map(|claim| derive_work_triple(&metadata, 2, claim).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            queue,
            vec![
                WorkTriple::new(0, 0, 4),
                WorkTriple::new(0, 1, 4),
                WorkTriple::new(1, 0, 9),
                WorkTriple::new(1, 1, 9),
            ]
        );
    }

    #[test]
    fn work_queue_rejects_u32_overflow_before_allocation() {
        let metadata = vec![
            FileMetadata {
                path_hash_lo: 1,
                path_hash_hi: 0,
                size_bytes: 1,
                decoded_layer_index: 0,
            },
            FileMetadata {
                path_hash_lo: 2,
                path_hash_hi: 0,
                size_bytes: 1,
                decoded_layer_index: 0,
            },
        ];
        let err = dense_queue_len(metadata.len(), u32::MAX).expect_err(
            "Fix: queue fanout exceeding u32 protocol must be rejected before allocation",
        );
        assert!(matches!(err, PipelineError::QueueFull { .. }));
    }

    #[test]
    fn device_schedule_accepts_batches_above_legacy_host_queue_cap_without_allocating() {
        let metadata = vec![
            FileMetadata {
                path_hash_lo: 1,
                path_hash_hi: 0,
                size_bytes: 1,
                decoded_layer_index: 0,
            };
            2
        ];
        const LEGACY_HOST_WORK_QUEUE_CAP: usize = 16 * 1024 * 1024;
        let rule_count = u32::try_from(LEGACY_HOST_WORK_QUEUE_CAP / metadata.len() + 1).unwrap();
        let queue_len = dense_queue_len(metadata.len(), rule_count)
            .expect("Fix: device-derived scheduling must not retain the old host allocation cap");
        assert!(
            queue_len as usize > LEGACY_HOST_WORK_QUEUE_CAP,
            "dense scheduling must scale past the removed host Vec<WorkTriple> limit"
        );
    }

    #[test]
    fn hit_capacity_rejects_allocation_cap() {
        let err = validate_hit_capacity(MAX_BATCH_HIT_CAPACITY + 1)
            .expect_err("oversized hit ring must reject before GPU allocation");
        assert!(matches!(err, PipelineError::QueueFull { .. }));
    }

    #[test]
    fn refresh_reuses_host_and_gpu_storage_when_shape_fits() {
        let backend = crate::WgpuBackend::new().expect(
            "Fix: live WGPU backend required for FileBatch refresh reuse contract; missing GPU is a configuration bug.",
        );
        let first = vec![
            BatchFile::new(1, 0, b"abcdefgh".to_vec()),
            BatchFile::new(2, 1, b"ijklmnop".to_vec()),
        ];
        let second = vec![BatchFile::new(3, 2, b"xyz".to_vec())];
        let mut batch = FileBatch::upload(backend.device_queue(), &first, 4, 1024)
            .expect("Fix: initial FileBatch upload must succeed");
        let metadata_ptr = batch.file_metadata.as_ptr();
        let offsets_ptr = batch.file_offsets.as_ptr();
        let haystack_words_ptr = batch.haystack_words.as_ptr();
        let haystack_id = batch.haystack.allocation_identity();
        let offsets_id = batch.offsets.allocation_identity();
        let metadata_id = batch.metadata.allocation_identity();
        let queue_state_id = batch.queue_state.allocation_identity();
        let hit_ring_id = batch.hit_ring.allocation_identity();

        let refresh_report = batch
            .refresh_with_report(&second, 2, 512)
            .expect("Fix: smaller FileBatch refresh must succeed in place");

        assert_eq!(batch.file_metadata.as_ptr(), metadata_ptr);
        assert_eq!(batch.file_offsets.as_ptr(), offsets_ptr);
        assert_eq!(batch.haystack_words.as_ptr(), haystack_words_ptr);
        assert_eq!(batch.haystack.allocation_identity(), haystack_id);
        assert_eq!(batch.offsets.allocation_identity(), offsets_id);
        assert_eq!(batch.metadata.allocation_identity(), metadata_id);
        assert_eq!(batch.queue_state.allocation_identity(), queue_state_id);
        assert_eq!(batch.hit_ring.allocation_identity(), hit_ring_id);
        assert_eq!(batch.file_count(), 1);
        assert_eq!(batch.queue_len(), 2);
        assert!(
            batch.host_work_items().is_empty(),
            "dense megakernel batches must not retain file_count * rule_count host triples"
        );
        assert_eq!(batch.hit_capacity(), 512);
        assert_eq!(
            refresh_report.resident_allocations, 0,
            "smaller refresh must reuse every resident allocation"
        );
        assert_eq!(
            refresh_report.refreshed_buffers, 0,
            "smaller refresh must not replace resident input buffers"
        );
        assert_eq!(
            refresh_report.reused_buffers, 6,
            "refresh must account for five refreshed inputs (haystack, offsets, metadata, \
             segments, queue_state) plus hit ring reuse"
        );
        assert!(
            refresh_report.bytes_uploaded > 0,
            "refresh telemetry must report host-to-device logical prefix writes"
        );

        let config = crate::megakernel::BatchDispatchConfig {
            workgroup_size_x: 64,
            worker_groups: 4,
            hit_capacity: 512,
            timeout: std::time::Duration::from_secs(10),
            ..Default::default()
        };
        let mut dispatcher = crate::megakernel::BatchDispatcher::new(backend, config)
            .expect("Fix: live batch dispatcher must compile after FileBatch refresh");
        let rules = vec![
            vyre_runtime::megakernel::BatchRuleProgram::new(0, vec![0; 256], vec![1], 1)
                .expect("Fix: accepting rule 0 must be valid"),
            vyre_runtime::megakernel::BatchRuleProgram::new(1, vec![0; 256], vec![1], 1)
                .expect("Fix: accepting rule 1 must be valid"),
        ];
        let mut hits = Vec::new();
        let report = dispatcher
            .dispatch_into(&batch, &rules, &mut hits)
            .expect("Fix: refreshed FileBatch must dispatch");
        assert_eq!(
            report.hit_count, 6,
            "refreshed batch must scan only the 3 refreshed bytes across 2 rules, not stale tail bytes"
        );
    }

    #[test]
    fn set_segmentation_preserves_hits_vs_whole_file() {
        // GPU PARITY: tiling a file into overlapping windows (`set_segmentation`)
        // must produce the EXACT same hit set as the whole-file scan. Uses an
        // accept-every rule (one state, accept[0]=1) so every byte offset is a
        // hit — the hit set is the offset tiling itself, so any double-count,
        // gap, or warm-up leak shows up directly. overlap=8 exercises the
        // emit-guard: warm-up bytes (`byte_pos < emit_start`) must advance state
        // but NEVER emit. dfa_sync_distance of a 1-state DFA is 0, so overlap=8 is
        // amply sound here.
        let backend = crate::WgpuBackend::new().expect(
            "Fix: live WGPU backend required for the segmentation GPU-parity contract.",
        );
        let content: Vec<u8> = (0..600u32).map(|i| (i % 251) as u8).collect();
        let files = vec![BatchFile::new(7, 0, content)];
        let mut batch = FileBatch::upload(backend.device_queue(), &files, 1, 2048)
            .expect("Fix: FileBatch upload must succeed");

        let config = crate::megakernel::BatchDispatchConfig {
            workgroup_size_x: 64,
            worker_groups: 8,
            hit_capacity: 2048,
            timeout: std::time::Duration::from_secs(10),
            ..Default::default()
        };
        let mut dispatcher = crate::megakernel::BatchDispatcher::new(backend, config)
            .expect("Fix: live batch dispatcher must compile");
        let rules = vec![
            vyre_runtime::megakernel::BatchRuleProgram::new(0, vec![0; 256], vec![1], 1)
                .expect("Fix: accept-every rule must be valid"),
        ];

        // Whole-file scan (one segment per file): expect one hit per byte offset.
        assert_eq!(batch.queue_len(), 1, "default geometry is one work item (1 file × 1 rule)");
        let mut whole = Vec::new();
        dispatcher
            .dispatch_into(&batch, &rules, &mut whole)
            .expect("Fix: whole-file dispatch must succeed");
        let whole_set: std::collections::BTreeSet<(u32, u32)> =
            whole.iter().map(|h| (h.rule_idx, h.match_offset)).collect();
        assert_eq!(whole_set.len(), 600, "accept-every rule hits every one of 600 offsets");
        assert_eq!(*whole_set.iter().next().unwrap(), (0, 0));
        assert_eq!(*whole_set.iter().next_back().unwrap(), (0, 599));

        // Segment into 5 windows (ceil(600/128)) with 8 bytes of warm-up.
        batch
            .set_segmentation(128, 8)
            .expect("Fix: set_segmentation must succeed");
        assert_eq!(
            batch.queue_len(),
            5,
            "600 bytes at seg_len=128 ⇒ 5 segments × 1 rule = 5 work items"
        );
        let mut segmented = Vec::new();
        dispatcher
            .dispatch_into(&batch, &rules, &mut segmented)
            .expect("Fix: segmented dispatch must succeed");
        let segmented_set: std::collections::BTreeSet<(u32, u32)> =
            segmented.iter().map(|h| (h.rule_idx, h.match_offset)).collect();

        assert_eq!(
            segmented_set, whole_set,
            "segmented scan must produce the identical (rule, offset) hit set as the whole-file scan"
        );
        // No double counting from the overlapping warm-up regions.
        assert_eq!(
            segmented.len(),
            whole.len(),
            "segmented scan must not duplicate hits across window warm-up overlaps"
        );
    }

    #[test]
    fn refresh_reused_buffers_write_only_padded_logical_prefix() {
        let src = include_str!("batch.rs");
        let production = src
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("Fix: FileBatch production section should precede tests");
        let refresh_body = src
            .split("pub fn refresh(")
            .nth(1)
            .and_then(|tail| tail.split("pub fn reset_queue_state").next())
            .expect("Fix: FileBatch::refresh body must be discoverable");
        let reused_write_body = src
            .split("fn write_padded_prefix(")
            .nth(1)
            .and_then(|tail| tail.split("fn validate_batch_shape").next())
            .expect("Fix: write_padded_prefix body must be discoverable");

        assert!(
            refresh_body.contains("accumulate_refresh"),
            "FileBatch::refresh must route resident inputs through telemetry-aware reusable buffer refresh"
        );
        assert!(
            refresh_body.contains("refresh_with_report"),
            "FileBatch::refresh must preserve the telemetry-capable refresh path"
        );
        assert!(
            reused_write_body.contains("crate::padded_upload::write_padded_prefix"),
            "reused FileBatch buffers must use the shared padded-prefix writer"
        );
        assert!(
            !reused_write_body.contains("allocation_len"),
            "reused FileBatch buffers must not zero-fill the full old allocation on smaller refreshes"
        );
        assert!(
            !production.contains("Vec::with_capacity"),
            "Fix: FileBatch upload/refresh staging must not use infallible capacity constructors."
        );
        assert!(
            !production.contains(".reserve_exact("),
            "Fix: FileBatch upload/refresh staging must route reservations through the shared fallible helper."
        );
        assert!(
            production.contains("reserve_batch_vec_len"),
            "Fix: FileBatch staging should have one shared target-length reservation adapter."
        );
    }
}
