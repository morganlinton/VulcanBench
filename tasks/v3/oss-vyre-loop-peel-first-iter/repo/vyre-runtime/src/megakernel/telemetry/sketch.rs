use super::RingTelemetry;
use crate::PipelineError;

/// Fixed-depth Count-Min sketch for compact megakernel telemetry.
///
/// The layout is intentionally plain `Vec<u64>` plus `(depth, width)` so the
/// same shape can be mirrored by GPU control buffers later. Hashing is
/// deterministic and seed-indexed; no host randomness is involved, which keeps
/// replay and regression tests stable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountMinSketch {
    depth: usize,
    width: usize,
    counters: Vec<u64>,
}

impl CountMinSketch {
    /// Create a zeroed sketch with the requested dimensions.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when either dimension is zero or the counter
    /// table size overflows host address space.
    pub fn new(depth: usize, width: usize) -> Result<Self, PipelineError> {
        if depth == 0 || width == 0 {
            return Err(PipelineError::QueueFull {
                queue: "telemetry",
                fix: "Count-Min sketch depth and width must be non-zero",
            });
        }
        let len = depth.checked_mul(width).ok_or(PipelineError::QueueFull {
            queue: "telemetry",
            fix: "Count-Min sketch dimensions overflowed host address space; reduce depth or width",
        })?;
        let mut counters = Vec::new();
        reserve_counter_capacity(&mut counters, len)?;
        counters.resize(len, 0);
        Ok(Self {
            depth,
            width,
            counters,
        })
    }

    /// Number of independent hash rows.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }

    /// Number of counters per hash row.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Raw row-major counters. Intended for readback, replay, and tests.
    #[must_use]
    pub fn counters(&self) -> &[u64] {
        &self.counters
    }

    /// Reset all counters to zero while retaining allocation.
    pub fn clear(&mut self) {
        self.counters.fill(0);
    }

    /// Resize this sketch shape and clear counters.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when either dimension is zero or the counter
    /// table size overflows host address space.
    pub fn reset_shape(&mut self, depth: usize, width: usize) -> Result<(), PipelineError> {
        if depth == 0 || width == 0 {
            return Err(PipelineError::QueueFull {
                queue: "telemetry",
                fix: "Count-Min sketch depth and width must be non-zero",
            });
        }
        let len = depth.checked_mul(width).ok_or(PipelineError::QueueFull {
            queue: "telemetry",
            fix: "Count-Min sketch dimensions overflowed host address space; reduce depth or width",
        })?;
        if self.depth == depth && self.width == width && self.counters.len() == len {
            self.counters.fill(0);
            return Ok(());
        }
        self.depth = depth;
        self.width = width;
        self.counters.clear();
        reserve_counter_capacity(&mut self.counters, len)?;
        self.counters.resize(len, 0);
        Ok(())
    }

    /// Add `amount` to every row bucket selected for `key`.
    pub fn add(&mut self, key: u32, amount: u64) {
        if amount == 0 {
            return;
        }
        for row in 0..self.depth {
            let idx = self.bucket(row, key);
            self.counters[idx] = self.counters[idx].saturating_add(amount);
        }
    }

    /// Checked add of `amount` to every row bucket selected for `key`.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when a counter would overflow u64.
    pub fn try_add(&mut self, key: u32, amount: u64) -> Result<(), PipelineError> {
        if amount == 0 {
            return Ok(());
        }
        for row in 0..self.depth {
            let idx = self.bucket(row, key);
            self.counters[idx] = self.counters[idx].checked_add(amount).ok_or_else(|| {
                PipelineError::Backend(format!(
                    "Count-Min sketch counter overflowed for row {row}, key {key}. Fix: snapshot and clear telemetry before counters reach u64::MAX."
                ))
            })?;
        }
        Ok(())
    }

    /// Conservative point estimate for `key`.
    #[must_use]
    pub fn estimate(&self, key: u32) -> u64 {
        (0..self.depth)
            .map(|row| self.counters[self.bucket(row, key)])
            .min()
            .unwrap_or(0)
    }

    /// Merge another sketch with identical dimensions into this sketch.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] if the sketches have different shapes.
    pub fn merge(&mut self, other: &Self) -> Result<(), PipelineError> {
        if self.depth != other.depth || self.width != other.width {
            return Err(PipelineError::Backend(format!(
                "cannot merge Count-Min sketches with shapes {}x{} and {}x{}. Fix: construct telemetry sketches with the same dimensions.",
                self.depth, self.width, other.depth, other.width
            )));
        }
        for (left, right) in self.counters.iter_mut().zip(&other.counters) {
            *left = left.checked_add(*right).ok_or_else(|| {
                PipelineError::Backend(
                    "Count-Min sketch merge overflowed u64. Fix: merge and clear telemetry more frequently."
                        .to_string(),
                )
            })?;
        }
        Ok(())
    }

    fn bucket(&self, row: usize, key: u32) -> usize {
        let row_u64 = row as u64;
        let hash = splitmix64(u64::from(key) ^ row_u64.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let bucket = (hash % self.width as u64) as usize;
        row * self.width + bucket
    }
}

fn reserve_counter_capacity(counters: &mut Vec<u64>, len: usize) -> Result<(), PipelineError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(counters, len).map_err(|source| {
        PipelineError::Backend(format!(
            "Count-Min sketch could not reserve {len} counters: {source}. Fix: reduce telemetry sketch depth or width."
        ))
    })
}

/// Compact sketch summary derived from a megakernel telemetry snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SketchTelemetry {
    /// Ring slots by opcode, regardless of terminal status.
    pub ring_opcode: CountMinSketch,
    /// Active ring slots by opcode.
    pub active_opcode: CountMinSketch,
    /// Ring slots by tenant id.
    pub tenant: CountMinSketch,
    /// Ring slots by raw status discriminant.
    pub status: CountMinSketch,
    /// Control-buffer dispatch metrics by opcode metric index.
    pub dispatch_metrics: CountMinSketch,
    /// Total decoded ring slots.
    pub total_slots: u64,
    /// Active decoded ring slots.
    pub active_slots: u64,
}

/// Caller-owned scratch for repeated compact telemetry sketches.
#[derive(Debug)]
pub struct SketchTelemetryScratch {
    /// Ring slots by opcode, regardless of terminal status.
    pub ring_opcode: CountMinSketch,
    /// Active ring slots by opcode.
    pub active_opcode: CountMinSketch,
    /// Ring slots by tenant id.
    pub tenant: CountMinSketch,
    /// Ring slots by raw status discriminant.
    pub status: CountMinSketch,
    /// Control-buffer dispatch metrics by opcode metric index.
    pub dispatch_metrics: CountMinSketch,
    pub(super) total_slots: u64,
    pub(super) active_slots: u64,
}

impl SketchTelemetryScratch {
    /// Create reusable sketch scratch with the requested dimensions.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when dimensions are invalid.
    pub fn new(depth: usize, width: usize) -> Result<Self, PipelineError> {
        Ok(Self {
            ring_opcode: CountMinSketch::new(depth, width)?,
            active_opcode: CountMinSketch::new(depth, width)?,
            tenant: CountMinSketch::new(depth, width)?,
            status: CountMinSketch::new(depth, width)?,
            dispatch_metrics: CountMinSketch::new(depth, width)?,
            total_slots: 0,
            active_slots: 0,
        })
    }

    fn reset(&mut self, depth: usize, width: usize) -> Result<(), PipelineError> {
        self.ring_opcode.reset_shape(depth, width)?;
        self.active_opcode.reset_shape(depth, width)?;
        self.tenant.reset_shape(depth, width)?;
        self.status.reset_shape(depth, width)?;
        self.dispatch_metrics.reset_shape(depth, width)?;
        self.total_slots = 0;
        self.active_slots = 0;
        Ok(())
    }

    /// Convert this reusable scratch into an owned snapshot.
    #[must_use]
    pub fn to_snapshot(&self) -> SketchTelemetry {
        SketchTelemetry {
            ring_opcode: self.ring_opcode.clone(),
            active_opcode: self.active_opcode.clone(),
            tenant: self.tenant.clone(),
            status: self.status.clone(),
            dispatch_metrics: self.dispatch_metrics.clone(),
            total_slots: self.total_slots,
            active_slots: self.active_slots,
        }
    }

    /// Move this scratch into an owned snapshot without cloning counter
    /// arrays. Use this for one-shot sketches; keep [`Self::to_snapshot`] for
    /// long-lived scratch that must be reused after sampling.
    #[must_use]
    pub fn into_snapshot(self) -> SketchTelemetry {
        SketchTelemetry {
            ring_opcode: self.ring_opcode,
            active_opcode: self.active_opcode,
            tenant: self.tenant,
            status: self.status,
            dispatch_metrics: self.dispatch_metrics,
            total_slots: self.total_slots,
            active_slots: self.active_slots,
        }
    }
}

impl RingTelemetry {
    /// Build compact sketches from the decoded telemetry snapshot.
    ///
    /// This is the host mirror of the telemetry shape a GPU-resident
    /// scheduler/fuzzer can maintain in control memory: hot opcodes, active
    /// work, tenant pressure, status pressure, and dispatch metrics all become
    /// bounded-size counters with deterministic replay semantics.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when sketch dimensions are invalid.
    pub fn sketch(&self, depth: usize, width: usize) -> Result<SketchTelemetry, PipelineError> {
        let mut scratch = SketchTelemetryScratch::new(depth, width)?;
        self.sketch_into(depth, width, &mut scratch)?;
        Ok(scratch.into_snapshot())
    }

    /// Build compact sketches into caller-owned scratch.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when sketch dimensions are invalid.
    pub fn sketch_into(
        &self,
        depth: usize,
        width: usize,
        scratch: &mut SketchTelemetryScratch,
    ) -> Result<(), PipelineError> {
        scratch.reset(depth, width)?;

        for slot in &self.slots {
            scratch.ring_opcode.try_add(slot.opcode, 1)?;
            scratch.tenant.try_add(slot.tenant_id, 1)?;
            scratch.status.try_add(slot.status.raw(), 1)?;
            if slot.status.is_active() {
                scratch.active_slots = scratch.active_slots.checked_add(1).ok_or_else(|| {
                    PipelineError::Backend(
                        "active megakernel telemetry slot count overflowed u64. Fix: snapshot telemetry before counters reach u64::MAX."
                            .to_string(),
                    )
                })?;
                scratch.active_opcode.try_add(slot.opcode, 1)?;
            }
        }

        for (opcode_idx, count) in &self.control.metrics {
            scratch
                .dispatch_metrics
                .try_add(*opcode_idx, u64::from(*count))?;
        }
        scratch.total_slots = u64::try_from(self.slots.len()).map_err(|error| {
            PipelineError::Backend(format!(
                "megakernel telemetry slot count cannot fit u64: {error}. Fix: shard telemetry snapshots before sketching."
            ))
        })?;
        Ok(())
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}
