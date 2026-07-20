//! Ring producer / consumer traits for the megakernel host protocol.
//!
//! T036 / T037 in `VyreOffload/RELEASE_PLAN.md`. Today the protocol
//! module ships byte-oriented `encode_*` / `decode_*` helpers and the
//! consumer (host) drives a `Vec<u8>` ring directly. To make the ring
//! source swappable  -  in-process host, out-of-process broker, or a
//! GPU-direct producer  -  we lift the two halves of that contract behind
//! traits and keep the existing path as the default in-process impl.
//!
//! The wire format is owned by [`super::protocol`]; this module sits
//! one level above it (publishing/observation surface, not bytes).
//!
//! ### Producer
//!
//! [`RingProducer::publish`] writes one encoded slot. The encoded bytes
//! come from a `protocol::encode_*` helper; the producer never inspects
//! them beyond their length. Producers are responsible for the
//! visibility/fence semantics the GPU expects (atomic store of the
//! status word last); the default in-process producer does this via the
//! protocol codec's byte ordering and memcpy.
//!
//! ### Consumer
//!
//! [`RingConsumer::read_slot`] is a read-only view of one slot's bytes.
//! Consumers may decode with `protocol::decode_*`. A consumer is
//! decoupled from where the bytes are stored (host RAM, GPU mirror,
//! shared-mem broker)  -  only the byte layout matters.
//!
//! ### Boundary
//!
//! Neither trait names a consumer-specific concept (no "expert", no
//! "MoE", no "shard"). The two traits are vyre-generic  -  see the
//! boundary rule in `AGENTS.md`.

use super::protocol::{self, ProtocolError};

const SLOT_WORDS_USIZE: usize = 16;
const STATUS_WORD_USIZE: usize = 0;
/// Bytes per slot in the megakernel ring buffer (= `SLOT_WORDS * 4`).
pub const SLOT_BYTES: usize = SLOT_WORDS_USIZE * 4;

/// Producer half of the megakernel ring contract.
///
/// Implementations write encoded slot bytes (from
/// [`protocol::encode_load_miss`] et al.) into a ring of `slot_count`
/// fixed-size slots. The mapping from logical slot index to physical
/// storage is the implementation's concern; consumers only see slot
/// indices and the byte layout the protocol module defines.
pub trait RingProducer {
    /// Publish `encoded` into `slot_idx`. `encoded` must be exactly
    /// [`SLOT_BYTES`] long; otherwise returns
    /// [`ProtocolError::MisalignedByteLength`].
    fn publish(&mut self, slot_idx: u32, encoded: &[u8]) -> Result<(), ProtocolError>;

    /// Number of slots in the underlying ring.
    fn slot_count(&self) -> u32;

    /// Stable identifier for telemetry (e.g. `"in-process-host"`,
    /// `"uring-cmd-nvme"`, `"gds"`).
    fn name(&self) -> &'static str;
}

/// Consumer half of the megakernel ring contract.
pub trait RingConsumer {
    /// Copy slot `slot_idx`'s bytes into `out`. `out` must be exactly
    /// [`SLOT_BYTES`] long; otherwise returns
    /// [`ProtocolError::MisalignedByteLength`].
    fn read_slot(&self, slot_idx: u32, out: &mut [u8]) -> Result<(), ProtocolError>;

    /// Fallibly count slots currently in `DONE` status.
    ///
    /// The default implementation walks the ring through [`Self::read_slot`].
    /// Specialized consumers backed by a device/control-buffer counter may
    /// override this method to avoid host reads. Unlike [`Self::done_count`],
    /// this surface reports malformed slot bytes and host arithmetic overflow
    /// as [`ProtocolError`] instead of panicking.
    fn try_done_count(&self) -> Result<u32, ProtocolError> {
        let mut acc = 0u32;
        let mut buf = [0u8; SLOT_BYTES];
        for slot in 0..self.slot_count() {
            self.read_slot(slot, &mut buf)?;
            if read_slot_status_word(&buf)? == protocol::slot::DONE {
                acc = acc
                    .checked_add(1)
                    .ok_or(ProtocolError::ByteLengthOverflow {
                        buffer: "ring done count",
                        fix: "shard the ring before host observation",
                    })?;
            }
        }
        Ok(acc)
    }

    /// Compatibility-only lossy count of slots currently in `DONE` status.
    ///
    /// Runtime paths must call [`Self::try_done_count`] so malformed snapshots
    /// and host arithmetic overflow remain observable as [`ProtocolError`].
    #[deprecated(
        note = "use RingConsumer::try_done_count so malformed ring snapshots do not collapse to zero"
    )]
    fn done_count(&self) -> u32 {
        self.try_done_count().unwrap_or(0)
    }

    /// Number of slots in the underlying ring.
    fn slot_count(&self) -> u32;
}

/// Default in-process ring backed by a `Vec<u8>`. Both [`RingProducer`]
/// and [`RingConsumer`] are implemented on a single `&mut` /`&` borrow
/// so the producer-consumer parity test can drive both halves with the
/// same buffer.
pub struct HostRing {
    bytes: Vec<u8>,
    slot_count: u32,
}

impl HostRing {
    /// Allocate a new ring of `slot_count` empty slots.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::ByteLengthOverflow`] if `slot_count`
    /// exceeds [`protocol::MAX_ENCODED_RING_SLOTS`].
    pub fn new(slot_count: u32) -> Result<Self, ProtocolError> {
        let bytes = protocol::try_encode_empty_ring(slot_count)?;
        Ok(Self { bytes, slot_count })
    }

    /// Borrow the underlying ring bytes (for the dispatch path that
    /// still consumes `&[u8]` directly).
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Mutably borrow the underlying ring bytes.
    #[must_use]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

fn ring_slot_base(slot_idx: u32) -> Result<usize, ProtocolError> {
    usize::try_from(slot_idx)
        .map_err(|_| ProtocolError::MissingWord {
            buffer: "ring slot",
            word_idx: usize::MAX,
            byte_len: 0,
            fix: "slot_idx cannot fit host usize; shard the megakernel ring before host access",
        })?
        .checked_mul(SLOT_BYTES)
        .ok_or(ProtocolError::MissingWord {
            buffer: "ring slot",
            word_idx: usize::MAX,
            byte_len: 0,
            fix: "slot byte offset overflowed usize; shard the megakernel ring before host access",
        })
}

fn ring_slot_word_index(slot_idx: u32) -> Result<usize, ProtocolError> {
    usize::try_from(slot_idx)
        .map_err(|_| ProtocolError::MissingWord {
            buffer: "ring slot",
            word_idx: usize::MAX,
            byte_len: 0,
            fix: "slot_idx cannot fit host usize; shard the megakernel ring before host access",
        })?
        .checked_mul(SLOT_WORDS_USIZE)
        .ok_or(ProtocolError::MissingWord {
            buffer: "ring slot",
            word_idx: usize::MAX,
            byte_len: 0,
            fix: "slot word offset overflowed usize; shard the megakernel ring before host access",
        })
}

fn read_slot_status_word(slot_bytes: &[u8]) -> Result<u32, ProtocolError> {
    let status_offset =
        STATUS_WORD_USIZE
            .checked_mul(4)
            .ok_or(ProtocolError::ByteLengthOverflow {
                buffer: "ring slot status",
                fix: "keep ring status word indices within host address space",
            })?;
    let status_end = status_offset
        .checked_add(4)
        .ok_or(ProtocolError::ByteLengthOverflow {
            buffer: "ring slot status",
            fix: "keep ring status word indices within host address space",
        })?;
    let bytes = slot_bytes
        .get(status_offset..status_end)
        .ok_or(ProtocolError::MissingWord {
            buffer: "ring slot",
            word_idx: STATUS_WORD_USIZE,
            byte_len: slot_bytes.len(),
            fix: "read a complete SLOT_BYTES slot before counting DONE status",
        })?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

impl RingProducer for HostRing {
    fn publish(&mut self, slot_idx: u32, encoded: &[u8]) -> Result<(), ProtocolError> {
        if encoded.len() != SLOT_BYTES {
            return Err(ProtocolError::MisalignedByteLength {
                buffer: "ring slot",
                byte_len: encoded.len(),
                fix: "encoded slot must be exactly SLOT_BYTES (64) long",
            });
        }
        if slot_idx >= self.slot_count {
            return Err(ProtocolError::MissingWord {
                buffer: "ring slot",
                word_idx: ring_slot_word_index(slot_idx)?,
                byte_len: self.bytes.len(),
                fix: "slot_idx must be < slot_count",
            });
        }
        let base = ring_slot_base(slot_idx)?;
        self.bytes[base..base + SLOT_BYTES].copy_from_slice(encoded);
        Ok(())
    }

    fn slot_count(&self) -> u32 {
        self.slot_count
    }

    fn name(&self) -> &'static str {
        "in-process-host"
    }
}

impl RingConsumer for HostRing {
    fn read_slot(&self, slot_idx: u32, out: &mut [u8]) -> Result<(), ProtocolError> {
        if out.len() != SLOT_BYTES {
            return Err(ProtocolError::MisalignedByteLength {
                buffer: "ring slot",
                byte_len: out.len(),
                fix: "out slice must be exactly SLOT_BYTES (64) long",
            });
        }
        if slot_idx >= self.slot_count {
            return Err(ProtocolError::MissingWord {
                buffer: "ring slot",
                word_idx: ring_slot_word_index(slot_idx)?,
                byte_len: self.bytes.len(),
                fix: "slot_idx must be < slot_count",
            });
        }
        let base = ring_slot_base(slot_idx)?;
        out.copy_from_slice(&self.bytes[base..base + SLOT_BYTES]);
        Ok(())
    }

    fn try_done_count(&self) -> Result<u32, ProtocolError> {
        let status_word_offset = STATUS_WORD_USIZE * 4;
        let mut done = 0u32;
        let slot_count =
            usize::try_from(self.slot_count).map_err(|_| ProtocolError::ByteLengthOverflow {
                buffer: "ring slot count",
                fix: "shard the ring before host observation",
            })?;
        for slot in 0..slot_count {
            let base = slot
                .checked_mul(SLOT_BYTES)
                .and_then(|offset| offset.checked_add(status_word_offset))
                .ok_or(ProtocolError::ByteLengthOverflow {
                    buffer: "ring status offset",
                    fix: "shard the ring before host observation",
                })?;
            let end = base
                .checked_add(4)
                .ok_or(ProtocolError::ByteLengthOverflow {
                    buffer: "ring status offset",
                    fix: "shard the ring before host observation",
                })?;
            let word = read_slot_status_word(self.bytes.get(base..end).ok_or(
                ProtocolError::MissingWord {
                    buffer: "ring slot",
                    word_idx: slot
                        .checked_mul(SLOT_WORDS_USIZE)
                        .and_then(|word| word.checked_add(STATUS_WORD_USIZE))
                        .unwrap_or(usize::MAX),
                    byte_len: self.bytes.len(),
                    fix: "slot_count and ring byte length disagree; rebuild HostRing through HostRing::new",
                },
            )?)?;
            if word == protocol::slot::DONE {
                done = done
                    .checked_add(1)
                    .ok_or(ProtocolError::ByteLengthOverflow {
                        buffer: "ring done count",
                        fix: "shard the ring before host observation",
                    })?;
            }
        }
        Ok(done)
    }

    fn slot_count(&self) -> u32 {
        self.slot_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parity: a slot published via the trait must round-trip through
    /// the consumer trait and decode identically via the existing
    /// `protocol::decode_load_miss` helper.
    #[test]
    fn host_ring_publishes_and_round_trips_a_load_miss() {
        let mut ring = HostRing::new(4).expect("Fix: ring constructs");
        let encoded = protocol::encode_load_miss(123, true);

        RingProducer::publish(&mut ring, 1, &encoded).expect("Fix: publish");

        let mut slot_bytes = [0u8; SLOT_BYTES];
        RingConsumer::read_slot(&ring, 1, &mut slot_bytes).expect("Fix: read_slot");
        assert_eq!(slot_bytes.as_slice(), encoded.as_slice());

        // And, importantly, the existing decoder must read it back from
        // the ring bytes at slot 1.
        let decoded = protocol::decode_load_miss(ring.as_bytes(), 1);
        assert_eq!(decoded, Some((123, true)));
    }

    #[test]
    fn host_ring_rejects_out_of_range_slot() {
        let mut ring = HostRing::new(2).unwrap();
        let encoded = protocol::encode_load_miss(0, false);
        let err_hi = RingProducer::publish(&mut ring, 2, &encoded).expect_err("slot 2 OOB");
        assert!(
            matches!(err_hi, ProtocolError::MissingWord { .. }),
            "OOB publish error: {err_hi}"
        );
        let err_max =
            RingProducer::publish(&mut ring, u32::MAX, &encoded).expect_err("slot MAX OOB");
        assert!(
            matches!(err_max, ProtocolError::MissingWord { .. }),
            "MAX slot publish error: {err_max}"
        );

        let mut buf = [0u8; SLOT_BYTES];
        let read_err = RingConsumer::read_slot(&ring, 2, &mut buf).expect_err("read OOB");
        assert!(
            matches!(read_err, ProtocolError::MissingWord { .. }),
            "OOB read error: {read_err}"
        );
    }

    #[test]
    fn host_ring_rejects_mis_sized_encoded() {
        let mut ring = HostRing::new(2).unwrap();
        let short = [0u8; SLOT_BYTES - 1];
        let short_pub = RingProducer::publish(&mut ring, 0, &short).expect_err("short publish");
        assert!(
            matches!(short_pub, ProtocolError::MisalignedByteLength { .. }),
            "short publish error: {short_pub}"
        );
        let long = [0u8; SLOT_BYTES + 1];
        let long_pub = RingProducer::publish(&mut ring, 0, &long).expect_err("long publish");
        assert!(
            matches!(long_pub, ProtocolError::MisalignedByteLength { .. }),
            "long publish error: {long_pub}"
        );

        let mut short_out = [0u8; SLOT_BYTES - 1];
        let short_read =
            RingConsumer::read_slot(&ring, 0, &mut short_out).expect_err("short read buffer");
        assert!(
            matches!(short_read, ProtocolError::MisalignedByteLength { .. }),
            "short read error: {short_read}"
        );
    }

    /// Default try_done_count walks the ring; if we stamp DONE into a slot's
    /// status word manually it must show up in the count.
    #[test]
    fn default_try_done_count_walks_the_ring() {
        let mut ring = HostRing::new(4).unwrap();
        // Empty ring: done count is 0.
        assert_eq!(RingConsumer::try_done_count(&ring).unwrap(), 0);

        // Stamp DONE into slot 0's status word.
        let bytes = ring.as_bytes_mut();
        let status_offset = STATUS_WORD_USIZE * 4;
        bytes[status_offset..status_offset + 4]
            .copy_from_slice(&protocol::slot::DONE.to_le_bytes());

        // And into slot 2's status word.
        let status_offset_2 = 2 * SLOT_BYTES + STATUS_WORD_USIZE * 4;
        bytes[status_offset_2..status_offset_2 + 4]
            .copy_from_slice(&protocol::slot::DONE.to_le_bytes());

        assert_eq!(RingConsumer::try_done_count(&ring).unwrap(), 2);
    }

    #[test]
    fn try_done_count_rejects_inconsistent_host_ring_bytes() {
        let ring = HostRing {
            bytes: vec![0u8; SLOT_BYTES],
            slot_count: 2,
        };

        let error = RingConsumer::try_done_count(&ring)
            .expect_err("Fix: malformed ring snapshots must not panic in fallible DONE count");
        assert!(
            matches!(error, ProtocolError::MissingWord { .. }),
            "Fix: malformed ring error must explain the slot-count/byte mismatch: {error}"
        );
    }
}
