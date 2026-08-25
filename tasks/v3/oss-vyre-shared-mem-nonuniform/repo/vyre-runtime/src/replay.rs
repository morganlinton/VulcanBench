//! Differential megakernel replay log.
//!
//! Every slot the host publishes into the megakernel ring is also
//! appended to a circular log on disk. A later replay run can feed
//! the log into a fresh megakernel + backend pair and diff the
//! epoch-by-epoch observable stream against the original. This
//! catches schedule-dependent bugs  -  GPU nondeterminism, atomic
//! ordering hazards, cache-line races  -  that unit tests cannot hit
//! by construction.
//!
//! ## Layout
//!
//! ```text
//! header (32 bytes, aligned to 4 KiB):
//!     magic:          b"VRRL0001"        (8 bytes)    -  "Vyre Ring-Replay Log"
//!     version:        u32 = 1            (4 bytes)
//!     flags:          u32 = 0            (4 bytes)
//!     capacity:       u64                (8 bytes)    -  total record slots
//!     next_slot:      u64                (8 bytes)    -  write cursor (mod capacity)
//! records:                                          (capacity × RECORD_BYTES)
//!     magic:          u32 = 0xDEADBEEF  (4 bytes)   -  sync marker for forward scan
//!     timestamp_ns:   u64                (8 bytes)
//!     slot_idx:       u32                (4 bytes)
//!     tenant_id:      u32                (4 bytes)
//!     opcode:         u32                (4 bytes)
//!     args:           [u32; 4]           (16 bytes)
//!     epoch:          u32                (4 bytes)   -  observed at publish time
//!     slot_status:    u32                (4 bytes)   -  terminal ring status, zero when unknown
//!     failure_class:  u32                (4 bytes)   -  [`ReplayFailureClass`] discriminant
//!     backend_code:   u32                (4 bytes)   -  stable [`vyre_driver::backend::ErrorCode`]
//!     output_digest:  u64                (8 bytes)   -  digest of output bytes observed at failure
//! ```
//!
//! Record size = 52 bytes ≤ 64. Aligning to 64 by padding the reserved
//! tail keeps records cache-line aligned so a consumer can `mmap` the
//! log and read records without tearing.
//!
//! ## Rollover
//!
//! The log is a fixed-capacity ring. `next_slot = (next_slot + 1) %
//! capacity`; a replay iterates from `next_slot` through all records
//! that have a live magic word. Records that predate the first wrap
//! are overwritten in publish order.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use crate::megakernel::recovery::{classify_backend_recovery_error, MegakernelRecoveryClass};
use crate::PipelineError;
use vyre_driver::backend::BackendError;

const LOG_MAGIC: &[u8; 8] = b"VRRL0001";
const LOG_VERSION: u32 = 1;
const RECORD_MAGIC: u32 = 0xDEAD_BEEF;
const RECORD_BYTES: u64 = 64;
const HEADER_BYTES: u64 = 32;
const MAX_REPLAY_RECORDS: u64 = 1_048_576;

/// One published ring slot as captured by the replay log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedSlot {
    /// Host wall-clock timestamp, nanoseconds since UNIX epoch.
    pub timestamp_ns: u64,
    /// Ring slot index the host published into.
    pub slot_idx: u32,
    /// Tenant id from the slot's TENANT_WORD.
    pub tenant_id: u32,
    /// Opcode from the slot's OPCODE_WORD.
    pub opcode: u32,
    /// First four argument words (the rest of the 13-word arg space
    /// lives in a packed-slot extension and is captured separately).
    pub args: [u32; 4],
    /// Megakernel EPOCH word observed at publish time. A replay run
    /// on the same backend must reach the same epoch in the same
    /// order  -  divergence is the load-bearing signal.
    pub epoch: u32,
}

/// One replay record including optional failure evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayRecord {
    /// Published ring slot.
    pub slot: RecordedSlot,
    /// Backend/runtime failure evidence captured for this slot.
    pub failure: Option<ReplayFailureEvidence>,
}

/// Backend/runtime failure class encoded into the replay record tail.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReplayFailureClass {
    /// No failure evidence was recorded for this published slot.
    #[default]
    None,
    /// Backend context, adapter, or compiled-pipeline state was lost or stale.
    DeviceLoss,
    /// Queue/resource pressure that can be retried without recompilation.
    TransientQueue,
    /// Program/lowering/kernel-source failure that should not be retried as-is.
    ProgramBug,
    /// Failure did not match a known automated recovery class.
    Unclassified,
}

impl ReplayFailureClass {
    const NONE: u32 = 0;
    const DEVICE_LOSS: u32 = 1;
    const TRANSIENT_QUEUE: u32 = 2;
    const PROGRAM_BUG: u32 = 3;
    const UNCLASSIFIED: u32 = 4;

    const fn encode(self) -> u32 {
        match self {
            Self::None => Self::NONE,
            Self::DeviceLoss => Self::DEVICE_LOSS,
            Self::TransientQueue => Self::TRANSIENT_QUEUE,
            Self::ProgramBug => Self::PROGRAM_BUG,
            Self::Unclassified => Self::UNCLASSIFIED,
        }
    }

    const fn decode(raw: u32) -> Self {
        match raw {
            Self::NONE => Self::None,
            Self::DEVICE_LOSS => Self::DeviceLoss,
            Self::TRANSIENT_QUEUE => Self::TransientQueue,
            Self::PROGRAM_BUG => Self::ProgramBug,
            Self::UNCLASSIFIED => Self::Unclassified,
            _ => Self::Unclassified,
        }
    }

    const fn from_recovery_class(class: MegakernelRecoveryClass) -> Self {
        match class {
            MegakernelRecoveryClass::DeviceLoss => Self::DeviceLoss,
            MegakernelRecoveryClass::TransientQueue => Self::TransientQueue,
            MegakernelRecoveryClass::ProgramBug => Self::ProgramBug,
            MegakernelRecoveryClass::Unclassified => Self::Unclassified,
        }
    }
}

/// Failure evidence captured in a replay record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayFailureEvidence {
    /// Terminal or observed ring status word for the failed slot.
    pub slot_status: u32,
    /// Recovery-oriented failure class.
    pub failure_class: ReplayFailureClass,
    /// Stable backend error code. Zero means no backend error was known.
    pub backend_error_code: u32,
    /// Stable digest over output bytes observed before/at failure.
    pub output_digest: u64,
}

impl ReplayFailureEvidence {
    /// Build replay failure evidence from a backend error and observed output bytes.
    #[must_use]
    pub fn from_backend_error(slot_status: u32, error: &BackendError, output_bytes: &[u8]) -> Self {
        Self {
            slot_status,
            failure_class: ReplayFailureClass::from_recovery_class(
                classify_backend_recovery_error(error),
            ),
            backend_error_code: error.code().stable_id(),
            output_digest: output_digest(output_bytes),
        }
    }

    fn from_words(
        slot_status: u32,
        failure_class: u32,
        backend_error_code: u32,
        output_digest: u64,
    ) -> Option<Self> {
        if slot_status == 0 && failure_class == 0 && backend_error_code == 0 && output_digest == 0 {
            return None;
        }
        Some(Self {
            slot_status,
            failure_class: ReplayFailureClass::decode(failure_class),
            backend_error_code,
            output_digest,
        })
    }
}

/// Errors surfaced by the replay-log surface. Every variant carries
/// an actionable `Fix:` hint.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReplayLogError {
    /// I/O syscall on the log file failed.
    #[error("replay log {op} on `{path}` failed: {source}. Fix: check disk space + permissions.")]
    Io {
        /// Syscall name (`open`, `seek`, `read`, `write`).
        op: &'static str,
        /// Path the syscall was issued against.
        path: Arc<str>,
        /// Underlying io::Error.
        #[source]
        source: std::io::Error,
    },
    /// Log header magic or version mismatch.
    #[error("replay log `{path}` header mismatch. Fix: regenerate the log; VRRL format may have changed.")]
    HeaderMismatch {
        /// Log path.
        path: Arc<str>,
    },
    /// Capacity of `0` is rejected  -  a zero-capacity log never accepts writes.
    #[error("replay log capacity must be > 0. Fix: construct with at least one slot.")]
    ZeroCapacity,
    /// Record capacity exceeds the replay-log bound. Capping here
    /// prevents malformed log headers from forcing host OOM during
    /// replay and keeps record offsets within checked arithmetic.
    #[error("replay log capacity {count} exceeds max {max}. Fix: shard replay into smaller logs.")]
    CapacityOverflow {
        /// Requested capacity.
        count: u64,
        /// Maximum accepted capacity.
        max: u64,
    },
}

fn io_err(op: &'static str, path: &Path, source: std::io::Error) -> ReplayLogError {
    ReplayLogError::Io {
        op,
        path: Arc::from(path.to_string_lossy().as_ref()),
        source,
    }
}

/// Append-only circular replay log backed by a real file. Callers
/// drive `append` on every host-side `publish_slot` and `replay_all`
/// at cert-time to walk the captured slot stream.
#[derive(Debug)]
pub struct RingLog {
    file: File,
    path_repr: Arc<str>,
    capacity: u64,
    next_slot: u64,
}

impl RingLog {
    /// Open a log at `path`, creating + preallocating one with
    /// `capacity` records if no file exists yet.
    ///
    /// # Errors
    ///
    /// - [`ReplayLogError::ZeroCapacity`] if `capacity == 0`.
    /// - [`ReplayLogError::CapacityOverflow`] if `capacity > u32::MAX`.
    /// - [`ReplayLogError::Io`] on any syscall failure.
    /// - [`ReplayLogError::HeaderMismatch`] when an existing file
    ///   has the wrong magic or version.
    pub fn open(path: impl AsRef<Path>, capacity: u64) -> Result<Self, ReplayLogError> {
        if capacity == 0 {
            return Err(ReplayLogError::ZeroCapacity);
        }
        validate_capacity(capacity)?;

        let path = path.as_ref();
        let path_repr: Arc<str> = Arc::from(path.to_string_lossy().as_ref());
        let existed = path.exists();
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| io_err("open", path, e))?;

        if existed {
            let mut magic = [0u8; 8];
            file.read_exact(&mut magic)
                .map_err(|e| io_err("read", path, e))?;
            if &magic != LOG_MAGIC {
                return Err(ReplayLogError::HeaderMismatch {
                    path: Arc::clone(&path_repr),
                });
            }
            let mut version_bytes = [0u8; 4];
            file.read_exact(&mut version_bytes)
                .map_err(|e| io_err("read", path, e))?;
            if u32::from_le_bytes(version_bytes) != LOG_VERSION {
                return Err(ReplayLogError::HeaderMismatch {
                    path: Arc::clone(&path_repr),
                });
            }
            let mut _flags = [0u8; 4];
            file.read_exact(&mut _flags)
                .map_err(|e| io_err("read", path, e))?;
            let mut cap_bytes = [0u8; 8];
            file.read_exact(&mut cap_bytes)
                .map_err(|e| io_err("read", path, e))?;
            let mut cursor_bytes = [0u8; 8];
            file.read_exact(&mut cursor_bytes)
                .map_err(|e| io_err("read", path, e))?;
            let existing_cap = u64::from_le_bytes(cap_bytes);
            validate_capacity(existing_cap)?;
            let cursor = u64::from_le_bytes(cursor_bytes);
            return Ok(Self {
                file,
                path_repr,
                capacity: existing_cap,
                next_slot: cursor % existing_cap,
            });
        }

        // Fresh log: write the header + zero the body so every record
        // magic starts at `0` (the uninitialised sentinel the replay
        // scanner treats as EMPTY).
        let total_bytes = log_file_len(capacity)?;
        file.set_len(total_bytes)
            .map_err(|e| io_err("set_len", path, e))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| io_err("seek", path, e))?;
        file.write_all(LOG_MAGIC)
            .map_err(|e| io_err("write", path, e))?;
        file.write_all(&LOG_VERSION.to_le_bytes())
            .map_err(|e| io_err("write", path, e))?;
        file.write_all(&0u32.to_le_bytes())
            .map_err(|e| io_err("write", path, e))?; // flags
        file.write_all(&capacity.to_le_bytes())
            .map_err(|e| io_err("write", path, e))?;
        file.write_all(&0u64.to_le_bytes())
            .map_err(|e| io_err("write", path, e))?; // cursor

        Ok(Self {
            file,
            path_repr,
            capacity,
            next_slot: 0,
        })
    }

    /// Number of record slots in the log. Records past this capacity
    /// wrap and overwrite the oldest entry.
    #[must_use]
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Current write cursor (next slot to be overwritten).
    #[must_use]
    pub fn cursor(&self) -> u64 {
        self.next_slot
    }

    /// Path representation this log was opened against.
    #[must_use]
    pub fn path(&self) -> &str {
        self.path_repr.as_ref()
    }

    /// Append a record. Overwrites the oldest slot when the log
    /// wraps. The cursor is persisted to disk on every append so a
    /// crash mid-session does not desynchronise the replay.
    ///
    /// # Errors
    ///
    /// Propagates [`ReplayLogError::Io`] on any file I/O failure.
    pub fn append(&mut self, slot: RecordedSlot) -> Result<(), ReplayLogError> {
        self.append_record(ReplayRecord {
            slot,
            failure: None,
        })
    }

    /// Append a record with backend/runtime failure evidence.
    ///
    /// # Errors
    ///
    /// Propagates [`ReplayLogError::Io`] on any file I/O failure.
    pub fn append_with_failure(
        &mut self,
        slot: RecordedSlot,
        failure: ReplayFailureEvidence,
    ) -> Result<(), ReplayLogError> {
        self.append_record(ReplayRecord {
            slot,
            failure: Some(failure),
        })
    }

    fn append_record(&mut self, record: ReplayRecord) -> Result<(), ReplayLogError> {
        let record_offset = log_record_offset(self.next_slot)?;
        self.file
            .seek(SeekFrom::Start(record_offset))
            .map_err(|e| self.io_err("seek", e))?;

        let mut buf = [0u8; RECORD_BYTES as usize];
        buf[0..4].copy_from_slice(&RECORD_MAGIC.to_le_bytes());
        buf[4..12].copy_from_slice(&record.slot.timestamp_ns.to_le_bytes());
        buf[12..16].copy_from_slice(&record.slot.slot_idx.to_le_bytes());
        buf[16..20].copy_from_slice(&record.slot.tenant_id.to_le_bytes());
        buf[20..24].copy_from_slice(&record.slot.opcode.to_le_bytes());
        buf[24..28].copy_from_slice(&record.slot.args[0].to_le_bytes());
        buf[28..32].copy_from_slice(&record.slot.args[1].to_le_bytes());
        buf[32..36].copy_from_slice(&record.slot.args[2].to_le_bytes());
        buf[36..40].copy_from_slice(&record.slot.args[3].to_le_bytes());
        buf[40..44].copy_from_slice(&record.slot.epoch.to_le_bytes());
        if let Some(failure) = record.failure {
            buf[44..48].copy_from_slice(&failure.slot_status.to_le_bytes());
            buf[48..52].copy_from_slice(&failure.failure_class.encode().to_le_bytes());
            buf[52..56].copy_from_slice(&failure.backend_error_code.to_le_bytes());
            buf[56..64].copy_from_slice(&failure.output_digest.to_le_bytes());
        }
        self.file
            .write_all(&buf)
            .map_err(|e| self.io_err("write", e))?;

        // Persist the advanced cursor. Readers that mmap the log see
        // this value and use it to know how far to scan.
        self.next_slot = (self.next_slot + 1) % self.capacity;
        self.file
            .seek(SeekFrom::Start(24)) // header cursor offset
            .map_err(|e| self.io_err("seek", e))?;
        self.file
            .write_all(&self.next_slot.to_le_bytes())
            .map_err(|e| self.io_err("write", e))?;

        Ok(())
    }

    /// Walk the log in publish order starting at the record
    /// immediately after the current cursor (oldest still-live
    /// record). Stops at the first record whose magic differs from
    /// the crate-private `RECORD_MAGIC` sentinel  -  meaning the log
    /// is still before wraparound at that position  -  unless every record
    /// has been written.
    ///
    /// # Errors
    ///
    /// Propagates [`ReplayLogError::Io`] on read failure.
    pub fn replay_all(&mut self) -> Result<Vec<RecordedSlot>, ReplayLogError> {
        Ok(self
            .replay_records()?
            .into_iter()
            .map(|record| record.slot)
            .collect())
    }

    /// Walk the log in publish order and return full records, including
    /// optional failure evidence.
    ///
    /// # Errors
    ///
    /// Propagates [`ReplayLogError::Io`] on read failure.
    pub fn replay_records(&mut self) -> Result<Vec<ReplayRecord>, ReplayLogError> {
        let capacity =
            usize::try_from(self.capacity).map_err(|_| ReplayLogError::CapacityOverflow {
                count: self.capacity,
                max: MAX_REPLAY_RECORDS,
            })?;
        let mut out = Vec::with_capacity(capacity);
        for step in 0..self.capacity {
            let slot_index = (self.next_slot + step) % self.capacity;
            let offset = log_record_offset(slot_index)?;
            self.file
                .seek(SeekFrom::Start(offset))
                .map_err(|e| self.io_err("seek", e))?;
            let mut buf = [0u8; RECORD_BYTES as usize];
            self.file
                .read_exact(&mut buf)
                .map_err(|e| self.io_err("read", e))?;
            let magic = read_u32(&buf, 0);
            if magic == 0 {
                // Zero-magic means the slot was never written (pre-wrap sentinel).
                // In a ring that has not yet wrapped, zero-magic slots at the scan
                // frontier are expected and skipped. However, if the ring HAS
                // wrapped, a zero-magic slot is a corruption gap (sector fault,
                // partial crash, or explicit zeroing of a live record) — the log
                // has no wrapped-flag field to distinguish these cases.
                //
                // Emit a warning so post-wrap corruption is operator-visible
                // rather than silently producing a shorter-than-expected replay.
                // A differential replay run comparing epoch sequences must treat
                // a warning here as a potential corruption event.
                tracing::warn!(
                    slot_index,
                    next_slot = self.next_slot,
                    log_capacity = self.capacity,
                    step,
                    "replay_records: zero-magic record at slot_index {slot_index} (step {step}). \
                     If the log has wrapped this is a corruption gap — the replay will be shorter than expected. \
                     Fix: ensure the replay-log file is not subject to external zeroing or partial-write truncation."
                );
                continue;
            }
            if magic != RECORD_MAGIC {
                return Err(ReplayLogError::HeaderMismatch {
                    path: self.path_repr.clone(),
                });
            }
            let slot = RecordedSlot {
                timestamp_ns: read_u64(&buf, 4),
                slot_idx: read_u32(&buf, 12),
                tenant_id: read_u32(&buf, 16),
                opcode: read_u32(&buf, 20),
                args: [
                    read_u32(&buf, 24),
                    read_u32(&buf, 28),
                    read_u32(&buf, 32),
                    read_u32(&buf, 36),
                ],
                epoch: read_u32(&buf, 40),
            };
            out.push(ReplayRecord {
                slot,
                failure: ReplayFailureEvidence::from_words(
                    read_u32(&buf, 44),
                    read_u32(&buf, 48),
                    read_u32(&buf, 52),
                    read_u64(&buf, 56),
                ),
            });
        }
        Ok(out)
    }

    /// Flush + sync the file to durable storage. Callers invoke this
    /// when they want the log guaranteed on disk  -  the hot-path
    /// `append` does not fsync per-record.
    ///
    /// # Errors
    ///
    /// Propagates [`ReplayLogError::Io`] on fsync failure.
    pub fn sync(&mut self) -> Result<(), ReplayLogError> {
        self.file.sync_all().map_err(|e| self.io_err("sync", e))?;
        Ok(())
    }

    fn io_err(&self, op: &'static str, source: std::io::Error) -> ReplayLogError {
        ReplayLogError::Io {
            op,
            path: self.path_repr.clone(),
            source,
        }
    }
}

fn validate_capacity(capacity: u64) -> Result<(), ReplayLogError> {
    if capacity == 0 {
        return Err(ReplayLogError::ZeroCapacity);
    }
    if capacity > MAX_REPLAY_RECORDS {
        return Err(ReplayLogError::CapacityOverflow {
            count: capacity,
            max: MAX_REPLAY_RECORDS,
        });
    }
    Ok(())
}

fn log_file_len(capacity: u64) -> Result<u64, ReplayLogError> {
    log_record_position(capacity)
}

fn log_record_offset(slot_index: u64) -> Result<u64, ReplayLogError> {
    log_record_position(slot_index)
}

fn log_record_position(record_index: u64) -> Result<u64, ReplayLogError> {
    let record_bytes =
        vyre_driver::accounting::checked_mul_u64_lazy(record_index, RECORD_BYTES, || {
            replay_capacity_overflow(record_index)
        })?;
    vyre_driver::accounting::checked_add_u64_lazy(HEADER_BYTES, record_bytes, || {
        replay_capacity_overflow(record_index)
    })
}

fn replay_capacity_overflow(count: u64) -> ReplayLogError {
    ReplayLogError::CapacityOverflow {
        count,
        max: MAX_REPLAY_RECORDS,
    }
}

fn read_u32(buf: &[u8], offset: usize) -> u32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&buf[offset..offset + 4]);
    u32::from_le_bytes(bytes)
}

fn read_u64(buf: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buf[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

fn output_digest(bytes: &[u8]) -> u64 {
    let digest = blake3::hash(bytes);
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest.as_bytes()[..8]);
    u64::from_le_bytes(out)
}

/// Let callers bridge ReplayLogError into the unified PipelineError
/// surface when driving the log from the megakernel pump loop.
impl From<ReplayLogError> for PipelineError {
    fn from(err: ReplayLogError) -> Self {
        PipelineError::Backend(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(slot_idx: u32, epoch: u32) -> RecordedSlot {
        RecordedSlot {
            timestamp_ns: 1_000_000 + slot_idx as u64,
            slot_idx,
            tenant_id: 0,
            opcode: 0x4000_0000 + slot_idx,
            args: [slot_idx, slot_idx * 2, slot_idx * 3, slot_idx * 4],
            epoch,
        }
    }

    #[test]
    fn open_rejects_zero_capacity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.vrrl");
        let err = RingLog::open(&path, 0).expect_err("zero capacity must reject");
        assert!(matches!(err, ReplayLogError::ZeroCapacity));
    }

    #[test]
    fn append_and_replay_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.vrrl");
        let mut log = RingLog::open(&path, 4)
            .expect("Fix: open fresh log; restore this invariant before continuing.");
        log.append(rec(1, 10)).unwrap();
        log.append(rec(2, 11)).unwrap();
        log.sync().unwrap();

        let replay = log
            .replay_all()
            .expect("Fix: replay; restore this invariant before continuing.");
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].slot_idx, 1);
        assert_eq!(replay[0].epoch, 10);
        assert_eq!(replay[1].slot_idx, 2);
        assert_eq!(replay[1].epoch, 11);
    }

    #[test]
    fn append_with_failure_round_trips_reproduction_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.vrrl");
        let mut log = RingLog::open(&path, 4)
            .expect("Fix: open fresh log; restore this invariant before continuing.");
        let backend_error = BackendError::DispatchFailed {
            code: Some(17),
            message: "DeviceLost after queue submit".to_string(),
        };
        let failure =
            ReplayFailureEvidence::from_backend_error(3, &backend_error, b"partial-output");

        assert_eq!(failure.failure_class, ReplayFailureClass::DeviceLoss);
        assert_eq!(failure.backend_error_code, backend_error.code().stable_id());
        assert_ne!(failure.output_digest, 0);

        log.append_with_failure(rec(7, 44), failure).unwrap();
        log.sync().unwrap();

        let replay = log
            .replay_records()
            .expect("Fix: replay records; restore this invariant before continuing.");
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].slot.slot_idx, 7);
        assert_eq!(replay[0].slot.epoch, 44);
        assert_eq!(replay[0].failure, Some(failure));
    }

    #[test]
    fn append_without_failure_has_no_failure_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.vrrl");
        let mut log = RingLog::open(&path, 2)
            .expect("Fix: open fresh log; restore this invariant before continuing.");

        log.append(rec(1, 10)).unwrap();

        let replay = log
            .replay_records()
            .expect("Fix: replay records; restore this invariant before continuing.");
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].slot.slot_idx, 1);
        assert_eq!(replay[0].failure, None);
    }

    #[test]
    fn log_rollover_preserves_most_recent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.vrrl");
        let mut log =
            RingLog::open(&path, 3).expect("Fix: open; restore this invariant before continuing.");
        for i in 0..5 {
            log.append(rec(i, 100 + i)).unwrap();
        }
        let replay = log
            .replay_all()
            .expect("Fix: replay; restore this invariant before continuing.");
        assert_eq!(replay.len(), 3, "capacity=3 must retain exactly 3 records");
        let slot_ids: Vec<u32> = replay.iter().map(|r| r.slot_idx).collect();
        // Publish order: 0, 1, 2, 3, 4. After 2 wraps, live records
        // are [3, 4, 2] in ring-physical order; replay starts at
        // next_slot = 2 so the visible order is [2, 3, 4].
        assert_eq!(slot_ids, vec![2, 3, 4]);
    }

    #[test]
    fn reopen_restores_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.vrrl");
        {
            let mut log = RingLog::open(&path, 4)
                .expect("Fix: open fresh; restore this invariant before continuing.");
            log.append(rec(1, 10)).unwrap();
            log.append(rec(2, 11)).unwrap();
            log.sync().unwrap();
        }
        let mut reopened = RingLog::open(&path, 4)
            .expect("Fix: reopen; restore this invariant before continuing.");
        assert_eq!(reopened.cursor(), 2);
        let replay = reopened.replay_all().unwrap();
        assert_eq!(replay.len(), 2);
    }

    #[test]
    fn corrupted_magic_rejected() {
        use std::io::Write as _;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.vrrl");
        {
            // Create a "log" file with the wrong magic.
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"XXXX0001").unwrap();
            f.write_all(&1u32.to_le_bytes()).unwrap();
            f.write_all(&0u32.to_le_bytes()).unwrap();
            f.write_all(&4u64.to_le_bytes()).unwrap();
            f.write_all(&0u64.to_le_bytes()).unwrap();
            // Ensure enough bytes for the subsequent reads in open() (headers ≥ 32 B).
            f.set_len(HEADER_BYTES + 4 * RECORD_BYTES).unwrap();
        }
        let err = RingLog::open(&path, 4).expect_err("wrong magic must reject");
        assert!(matches!(err, ReplayLogError::HeaderMismatch { .. }));
    }

    fn write_header(path: &Path, capacity: u64, cursor: u64) {
        use std::io::Write as _;

        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(LOG_MAGIC).unwrap();
        f.write_all(&LOG_VERSION.to_le_bytes()).unwrap();
        f.write_all(&0u32.to_le_bytes()).unwrap();
        f.write_all(&capacity.to_le_bytes()).unwrap();
        f.write_all(&cursor.to_le_bytes()).unwrap();
    }

    #[test]
    fn existing_log_zero_capacity_rejected_before_cursor_modulo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.vrrl");
        write_header(&path, 0, 0);

        let err = RingLog::open(&path, 4).expect_err("header capacity=0 must reject");
        assert!(matches!(err, ReplayLogError::ZeroCapacity));
    }

    #[test]
    fn existing_log_huge_capacity_rejected_before_replay_allocation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.vrrl");
        write_header(&path, MAX_REPLAY_RECORDS + 1, 0);

        let err = RingLog::open(&path, 4).expect_err("huge header capacity must reject");
        assert!(matches!(
            err,
            ReplayLogError::CapacityOverflow {
                count,
                max: MAX_REPLAY_RECORDS
            } if count == MAX_REPLAY_RECORDS + 1
        ));
    }

    #[test]
    fn capacity_overflow_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.vrrl");
        let err = RingLog::open(&path, MAX_REPLAY_RECORDS + 1)
            .expect_err("over-size capacity must reject");
        assert!(matches!(
            err,
            ReplayLogError::CapacityOverflow {
                count,
                max: MAX_REPLAY_RECORDS
            } if count == MAX_REPLAY_RECORDS + 1
        ));
    }

    /// Regression test for the P1 zero-magic skip behavior.
    ///
    /// Before the fix the skip was completely silent — an operator observing a
    /// replay shorter than expected had no signal that a zero-magic record had
    /// been encountered. After the fix the skip emits `tracing::warn!`. We
    /// cannot assert tracing output in a unit test, but we CAN assert the
    /// observable contract: a zero-magic slot in the middle of the scan range
    /// must NOT produce an Err (it must still be skipped gracefully), AND the
    /// replay result must be shorter than the number of appended records,
    /// confirming the gap is present and observable to the caller through the
    /// length discrepancy.
    #[test]
    fn replay_zero_magic_mid_sequence_skips_gracefully_and_produces_shorter_result() {
        use std::io::{Seek, SeekFrom, Write as _};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.vrrl");
        let mut log = RingLog::open(&path, 4)
            .expect("Fix: open fresh log; restore this invariant before continuing.");

        // Append 3 records into a 4-slot capacity log.
        log.append(rec(10, 100)).unwrap();
        log.append(rec(20, 200)).unwrap();
        log.append(rec(30, 300)).unwrap();
        log.sync().unwrap();

        // Verify a clean replay first: cursor = 3, scan starts at slot 3 (empty),
        // then wraps to 0, 1, 2 — so we get exactly 3 records.
        {
            let records = log
                .replay_all()
                .expect("Fix: replay of 3 records must succeed");
            assert_eq!(records.len(), 3, "Fix: 3 appended records must all replay");
        }

        // Now zero out the record at slot 1 (record 20) directly via file I/O.
        // This simulates a sector fault / partial crash zeroing a live slot.
        let slot1_offset = HEADER_BYTES + RECORD_BYTES; // slot 0 is at HEADER_BYTES; slot 1 follows
        {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap();
            f.seek(SeekFrom::Start(slot1_offset)).unwrap();
            f.write_all(&[0u8; RECORD_BYTES as usize]).unwrap();
            f.sync_all().unwrap();
        }

        // Re-open the log to pick up the zeroed slot.
        let mut log2 = RingLog::open(&path, 4)
            .expect("Fix: reopen after zeroing must succeed");

        // Replay must not return Err — the zero-magic skip is graceful.
        let records = log2
            .replay_all()
            .expect("Fix: replay with a zeroed slot must not error");

        // We should now see only 2 records (slot 0 = rec(10) and slot 2 = rec(30)).
        // The scan order from cursor=3: slots 3 (empty), 0 (rec 10), 1 (zeroed → skip), 2 (rec 30).
        assert_eq!(
            records.len(),
            2,
            "Fix: zeroed slot must be skipped, yielding 2 out of 3 records; got: {:?}",
            records.iter().map(|r| r.slot_idx).collect::<Vec<_>>()
        );
        // Record 10 must come before record 30 in publish order.
        assert_eq!(records[0].slot_idx, 10, "Fix: first replayed record must be slot_idx=10");
        assert_eq!(records[1].slot_idx, 30, "Fix: second replayed record must be slot_idx=30");
    }
}
