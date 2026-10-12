use crate::megakernel::planner::MegakernelWorkItem;
use crate::megakernel::protocol::{self, slot, SLOT_WORDS};
use crate::megakernel::{scheduler, Megakernel, PackedOpDescriptor};
use crate::PipelineError;

const SLOT_WORDS_USIZE: usize = 16;
const STATUS_WORD_USIZE: usize = 0;
const OPCODE_WORD_USIZE: usize = 1;
const TENANT_WORD_USIZE: usize = 2;
const PRIORITY_WORD_USIZE: usize = 3;
const ARG0_WORD_USIZE: usize = 4;
const ARGS_PER_SLOT_USIZE: usize = 12;

#[derive(Debug, Clone, Copy)]
struct RingPublishView {
    slot_bytes: usize,
    slot_capacity: usize,
}

/// Explicit host-observable ring slot lifecycle transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingSlotTransition {
    /// Host publishes a fully written slot.
    Publish,
    /// Worker claims a published or scheduler-ready slot.
    Claim,
    /// Worker marks a claimed slot done.
    Done,
    /// Runtime marks an in-flight slot faulted.
    Fault,
    /// Host cancels an unclaimed in-flight slot.
    Cancel,
}

impl RingSlotTransition {
    fn label(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Claim => "claim",
            Self::Done => "done",
            Self::Fault => "fault",
            Self::Cancel => "cancel",
        }
    }

    fn target_status(self) -> u32 {
        match self {
            Self::Publish => slot::PUBLISHED,
            Self::Claim => slot::CLAIMED,
            Self::Done => slot::DONE,
            Self::Fault => slot::FAULT,
            Self::Cancel => slot::EMPTY,
        }
    }

    fn allows(self, current_status: u32) -> bool {
        match self {
            Self::Publish => matches!(current_status, slot::EMPTY | slot::DONE),
            Self::Claim => matches!(
                current_status,
                slot::PUBLISHED | slot::YIELD | slot::REQUEUE
            ),
            Self::Done => current_status == slot::CLAIMED,
            Self::Fault => matches!(
                current_status,
                slot::PUBLISHED | slot::CLAIMED | slot::WAIT_IO | slot::YIELD | slot::REQUEUE
            ),
            Self::Cancel => matches!(
                current_status,
                slot::PUBLISHED | slot::WAIT_IO | slot::YIELD | slot::REQUEUE
            ),
        }
    }
}

fn validate_ring_publish_view(ring_bytes: &[u8]) -> Result<RingPublishView, PipelineError> {
    let slot_bytes = SLOT_WORDS_USIZE
        .checked_mul(4)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "slot byte width overflowed usize; keep SLOT_WORDS within the u32 ABI",
        })?;
    if ring_bytes.len() % slot_bytes != 0 {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "ring buffer byte length is not an exact multiple of SLOT_WORDS * 4; rebuild it with Megakernel::encode_empty_ring",
        });
    }
    Ok(RingPublishView {
        slot_bytes,
        slot_capacity: ring_bytes.len() / slot_bytes,
    })
}

impl Megakernel {
    /// Apply one explicit lifecycle transition to a ring slot status word.
    ///
    /// This helper is for host-side tests, recovery, cancellation, and
    /// diagnostics. Normal publication should still use [`Megakernel::publish_slot`]
    /// or batch publishers so payload words are written before the PUBLISHED
    /// status barrier.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the slot is out of bounds,
    /// the ring is malformed, or the requested transition is illegal for the
    /// current status word.
    pub fn transition_slot_status(
        ring_bytes: &mut [u8],
        slot_idx: u32,
        transition: RingSlotTransition,
    ) -> Result<u32, PipelineError> {
        if transition == RingSlotTransition::Publish {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "publish transitions must use publish_slot or a batch publisher so payload words are written before PUBLISHED status",
            });
        }
        let view = validate_ring_publish_view(ring_bytes)?;
        let current_status = read_slot_status_word(ring_bytes, view, slot_idx)?;
        validate_slot_transition(current_status, transition)?;
        write_slot_status_word(ring_bytes, view, slot_idx, transition.target_status())?;
        Ok(current_status)
    }

    /// Publish one opcode into `ring_bytes[slot_idx]`.
    ///
    /// # Errors
    ///
    /// [`PipelineError::QueueFull`] when out of bounds, too many args,
    /// or the slot is still in flight.
    pub fn publish_slot(
        ring_bytes: &mut [u8],
        slot_idx: u32,
        tenant_id: u32,
        opcode: u32,
        args: &[u32],
    ) -> Result<(), PipelineError> {
        let view = validate_ring_publish_view(ring_bytes)?;
        Self::publish_slot_validated(ring_bytes, view, slot_idx, tenant_id, opcode, args)
    }

    /// Reset `ring_bytes` to an empty ring and publish a contiguous `MegakernelWorkItem`
    /// queue into slots `0..items.len()`.
    ///
    /// This is the hot-path publisher for one-shot megakernel launches. It
    /// validates the full batch before mutating `ring_bytes`, encodes an empty
    /// ring once, writes the fixed `MegakernelWorkItem` ABI directly, and stores
    /// [`slot::PUBLISHED`] last for each slot.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when `slot_count` cannot encode,
    /// the queue does not fit in the ring, the slot ABI cannot hold a
    /// `MegakernelWorkItem`, or an item opcode is not publishable.
    pub fn encode_work_items_ring_into(
        slot_count: u32,
        tenant_id: u32,
        items: &[MegakernelWorkItem],
        ring_bytes: &mut Vec<u8>,
    ) -> Result<(), PipelineError> {
        let item_count = u32::try_from(items.len()).map_err(|_| PipelineError::QueueFull {
            queue: "submission",
            fix: "work item count exceeds u32::MAX; shard the megakernel queue before publishing",
        })?;
        if item_count > slot_count {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "work item count exceeds ring slot count; enlarge the launch geometry before publishing",
            });
        }
        if ARGS_PER_SLOT_USIZE < 3 {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "MegakernelWorkItem publication requires three argument words; increase ARGS_PER_SLOT",
            });
        }
        for item in items {
            if let Err(fix) = protocol::opcode::validate_publish_opcode(item.op_handle) {
                return Err(PipelineError::QueueFull {
                    queue: "submission",
                    fix,
                });
            }
        }

        protocol::try_encode_empty_ring_into(slot_count, ring_bytes)
            .map_err(super::protocol_error)?;
        let view = validate_ring_publish_view(ring_bytes)?;
        debug_assert!(items.len() <= view.slot_capacity);

        for (slot_idx, item) in items.iter().enumerate() {
            let slot_idx = u32::try_from(slot_idx).map_err(|_| PipelineError::QueueFull {
                queue: "submission",
                fix: "work item publish slot index exceeds u32::MAX; split the publish batch",
            })?;
            write_work_item_unchecked(ring_bytes, view, slot_idx, tenant_id, item)?;
        }
        Ok(())
    }

    /// Publish a contiguous fixed-ABI work-item window into an existing ring
    /// without resetting unrelated slots.
    ///
    /// This is the resident hot path for repeated megakernel queue updates:
    /// validate the whole target window first, then write each slot once and
    /// store [`slot::PUBLISHED`] last. Unlike
    /// [`Megakernel::encode_work_items_ring_into`], this does not clear the
    /// full ring, so sparse updates scale with `items.len()` rather than
    /// `slot_count`.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the target window is outside
    /// the ring, any slot is still in flight, or an item opcode is not
    /// publishable.
    pub fn publish_work_items(
        ring_bytes: &mut [u8],
        start_slot: u32,
        tenant_id: u32,
        items: &[MegakernelWorkItem],
    ) -> Result<u32, PipelineError> {
        validate_work_items(items)?;
        let item_count = u32::try_from(items.len()).map_err(|_| PipelineError::QueueFull {
            queue: "submission",
            fix: "work item count exceeds u32::MAX; shard the megakernel queue before publishing",
        })?;
        let view = validate_ring_publish_view(ring_bytes)?;
        let end_slot = start_slot
            .checked_add(item_count)
            .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix: "work item publish slot index overflowed u32; split the publish batch",
            })?;
        if u32_to_usize(end_slot)? > view.slot_capacity {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix:
                    "work item publish exceeds ring slot count; enlarge the ring or split the batch",
            });
        }
        for slot_idx in start_slot..end_slot {
            validate_publishable_slot(ring_bytes, view, slot_idx)?;
        }
        for (offset, item) in items.iter().enumerate() {
            let slot_idx = start_slot
                .checked_add(u32::try_from(offset).map_err(|_| PipelineError::QueueFull {
                    queue: "submission",
                    fix: "work item publish offset exceeds u32::MAX; split the publish batch",
                })?)
                .ok_or(PipelineError::QueueFull {
                    queue: "submission",
                    fix: "work item publish slot index overflowed u32; split the publish batch",
                })?;
            write_work_item_unchecked(ring_bytes, view, slot_idx, tenant_id, item)?;
        }
        Ok(item_count)
    }

    /// Reset `ring_words` to an empty ring and publish a contiguous `MegakernelWorkItem`
    /// queue as native little-endian u32 words.
    ///
    /// This is equivalent to [`Megakernel::encode_work_items_ring_into`] but
    /// avoids thousands of tiny byte-slice stores on hot dispatch paths. Callers
    /// can pass the result to backends as bytes with `bytemuck::cast_slice`.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when `slot_count` cannot encode,
    /// the queue does not fit in the ring, the slot ABI cannot hold a
    /// `MegakernelWorkItem`, or an item opcode is not publishable.
    pub fn encode_work_items_ring_words_into(
        slot_count: u32,
        tenant_id: u32,
        items: &[MegakernelWorkItem],
        ring_words: &mut Vec<u32>,
    ) -> Result<(), PipelineError> {
        validate_work_item_batch(slot_count, items)?;
        let total_words = encoded_ring_word_count(slot_count)?;

        if ring_words.len() != total_words {
            ring_words.clear();
            ring_words.resize(total_words, 0);
        } else {
            let slot_count = u32_to_usize(slot_count)?;
            for slot_idx in items.len()..slot_count {
                ring_words[slot_idx * SLOT_WORDS_USIZE + STATUS_WORD_USIZE] = slot::EMPTY;
            }
        }

        for (slot_idx, item) in items.iter().enumerate() {
            let base = slot_idx * SLOT_WORDS_USIZE;
            ring_words[base + OPCODE_WORD_USIZE] = item.op_handle;
            ring_words[base + TENANT_WORD_USIZE] = tenant_id;
            ring_words[base + PRIORITY_WORD_USIZE] = scheduler::priority::NORMAL;
            ring_words[base + ARG0_WORD_USIZE] = item.input_handle;
            ring_words[base + ARG0_WORD_USIZE + 1] = item.output_handle;
            ring_words[base + ARG0_WORD_USIZE + 2] = item.param;
            ring_words[base + STATUS_WORD_USIZE] = slot::PUBLISHED;
        }
        Ok(())
    }

    fn publish_slot_validated(
        ring_bytes: &mut [u8],
        view: RingPublishView,
        slot_idx: u32,
        tenant_id: u32,
        opcode: u32,
        args: &[u32],
    ) -> Result<(), PipelineError> {
        if u32_to_usize(slot_idx)? >= view.slot_capacity {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "slot_idx exceeds ring slot count; enlarge the ring via encode_empty_ring",
            });
        }
        if args.len() > ARGS_PER_SLOT_USIZE {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "too many args for one slot; 12 u32 args max per slot",
            });
        }
        if let Err(fix) = protocol::opcode::validate_publish_opcode(opcode) {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix,
            });
        }

        let base = slot_base(slot_idx, view)?;
        let read_word = |buf: &[u8], word_idx: usize| -> Result<u32, PipelineError> {
            let off = base + word_idx * 4;
            let bytes = buf.get(off..off + 4).ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix: "slot word is outside the validated ring buffer; validate ring length before publishing",
            })?;
            let mut word = [0u8; 4];
            word.copy_from_slice(bytes);
            Ok(u32::from_le_bytes(word))
        };

        let current_status = read_word(ring_bytes, STATUS_WORD_USIZE)?;
        validate_slot_transition(current_status, RingSlotTransition::Publish)?;

        let write_word = |buf: &mut [u8], word_idx: usize, value: u32| {
            let off = base + word_idx * 4;
            buf[off..off + 4].copy_from_slice(&value.to_le_bytes());
        };

        write_word(ring_bytes, OPCODE_WORD_USIZE, opcode);
        write_word(ring_bytes, TENANT_WORD_USIZE, tenant_id);
        write_word(ring_bytes, PRIORITY_WORD_USIZE, scheduler::priority::NORMAL);
        let args_start = base + ARG0_WORD_USIZE * 4;
        let args_end = args_start + ARGS_PER_SLOT_USIZE * 4;
        ring_bytes[args_start..args_end].fill(0);
        for (i, arg) in args.iter().enumerate() {
            write_word(ring_bytes, ARG0_WORD_USIZE + i, *arg);
        }
        // Status last  -  PUBLISH is the publish barrier.
        write_word(ring_bytes, STATUS_WORD_USIZE, slot::PUBLISHED);

        Ok(())
    }

    /// Publish one packed slot containing multiple inner ops.
    ///
    /// The inner opcode id is stored as `u8`; args are packed into the slot's
    /// 12-word payload tail and addressed by per-op `arg_offset` values.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the packed payload exceeds
    /// the slot capacity or when the target slot is not publishable.
    pub fn publish_packed_slot<A>(
        ring_bytes: &mut [u8],
        slot_idx: u32,
        tenant_id: u32,
        ops: &[(u8, A)],
    ) -> Result<(), PipelineError>
    where
        A: AsRef<[u32]>,
    {
        Self::publish_packed_slot_from(ring_bytes, slot_idx, tenant_id, ops.len(), |index| {
            let (op_id, args) = &ops[index];
            (*op_id, args.as_ref())
        })
    }

    pub(crate) fn publish_packed_descriptors(
        ring_bytes: &mut [u8],
        slot_idx: u32,
        tenant_id: u32,
        ops: &[PackedOpDescriptor],
    ) -> Result<(), PipelineError> {
        Self::publish_packed_slot_from(ring_bytes, slot_idx, tenant_id, ops.len(), |index| {
            let op = &ops[index];
            (op.opcode, op.args.as_slice())
        })
    }

    fn publish_packed_slot_from<'a>(
        ring_bytes: &mut [u8],
        slot_idx: u32,
        tenant_id: u32,
        op_count: usize,
        mut op_at: impl FnMut(usize) -> (u8, &'a [u32]),
    ) -> Result<(), PipelineError> {
        let opcode_count = u8::try_from(op_count).map_err(|_| PipelineError::QueueFull {
            queue: "submission",
            fix: "packed slot supports at most 255 inner opcodes",
        })?;
        let metadata_bytes = op_count
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(2))
            .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix: "packed slot metadata length overflowed usize; reduce packed opcode count",
            })?;
        let metadata_words = metadata_bytes.div_ceil(4);
        if metadata_words > ARGS_PER_SLOT_USIZE {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "packed slot metadata exceeds the 12-word slot argument budget",
            });
        }

        let mut packed_args = [0u32; ARGS_PER_SLOT_USIZE];
        let mut packed_arg_words = 0usize;
        let mut args = [0u32; ARGS_PER_SLOT_USIZE];
        write_packed_metadata_byte(&mut args, 0, opcode_count);
        let metadata_payload_bytes =
            metadata_words
                .checked_mul(4)
                .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix:
                    "packed slot metadata byte length overflowed usize; reduce packed opcode count",
            })?;
        for index in 0..op_count {
            let arg_offset =
                u8::try_from(packed_arg_words).map_err(|_| PipelineError::QueueFull {
                    queue: "submission",
                    fix: "packed slot arg offsets must fit in one u8 word index",
                })?;
            let (op_id, op_args) = op_at(index);
            let end =
                packed_arg_words
                    .checked_add(op_args.len())
                    .ok_or(PipelineError::QueueFull {
                        queue: "submission",
                        fix: "packed slot arg word count overflowed usize; reduce packed args",
                    })?;
            let total_words = metadata_words
                .checked_add(end)
                .ok_or(PipelineError::QueueFull {
                    queue: "submission",
                    fix: "packed slot total word count overflowed usize; reduce packed args",
                })?;
            if total_words > ARGS_PER_SLOT_USIZE {
                return Err(PipelineError::QueueFull {
                    queue: "submission",
                    fix: "packed slot payload exceeds the 12-word slot argument budget",
                });
            }
            packed_args[packed_arg_words..end].copy_from_slice(op_args);
            packed_arg_words = end;

            let byte_index = 2 + index * 2;
            debug_assert!(byte_index + 1 < metadata_payload_bytes);
            write_packed_metadata_byte(&mut args, byte_index, op_id);
            write_packed_metadata_byte(&mut args, byte_index + 1, arg_offset);
        }

        // Byte 1: total packed arg word count, so the host-side
        // decoder can slice off the correct portion without relying
        // on trailing-zero heuristics (slot memory can legitimately
        // contain zero arg values, and rings aren't guaranteed zero
        // after wrap-around).
        let packed_arg_words_u8 =
            u8::try_from(packed_arg_words).map_err(|_| PipelineError::QueueFull {
                queue: "submission",
                fix: "packed slot total arg words must fit in one u8",
            })?;
        write_packed_metadata_byte(&mut args, 1, packed_arg_words_u8);
        let total_words = metadata_words + packed_arg_words;
        args[metadata_words..total_words].copy_from_slice(&packed_args[..packed_arg_words]);
        Self::publish_slot(
            ring_bytes,
            slot_idx,
            tenant_id,
            protocol::opcode::PACKED_SLOT,
            &args[..total_words],
        )
    }

    /// Publish multiple slots atomically  -  the final slot is a
    /// `BATCH_FENCE` that signals completion to the host. This is
    /// the high-throughput entry point for scanner pipelines: publish
    /// N work items + 1 fence in one call.
    ///
    /// # Errors
    ///
    /// [`PipelineError::QueueFull`] if any slot rejects.
    pub fn batch_publish<A>(
        ring_bytes: &mut [u8],
        start_slot: u32,
        tenant_id: u32,
        items: &[(u32, A)], // (opcode, args) pairs
        batch_tag: u32,
    ) -> Result<u32, PipelineError>
    where
        A: AsRef<[u32]>,
    {
        let item_count = u32::try_from(items.len()).map_err(|_| PipelineError::QueueFull {
            queue: "submission",
            fix: "batch item count exceeds u32::MAX; split the publish batch",
        })?;
        let view = validate_ring_publish_view(ring_bytes)?;
        let total_slots = item_count.checked_add(1).ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "batch publish slot count overflowed u32; split the publish batch",
        })?;
        let end_slot = start_slot
            .checked_add(total_slots)
            .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix: "batch publish slot index overflowed u32; split the publish batch",
            })?;
        if u32_to_usize(end_slot)? > view.slot_capacity {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "batch publish exceeds ring slot count; enlarge the ring or split the batch",
            });
        }
        for (opcode, args) in items {
            validate_publish_payload(*opcode, args.as_ref())?;
        }
        validate_publish_payload(protocol::opcode::BATCH_FENCE, &[item_count, batch_tag])?;
        for slot_idx in start_slot..end_slot {
            validate_publishable_slot(ring_bytes, view, slot_idx)?;
        }

        for (offset, (opcode, args)) in items.iter().enumerate() {
            let slot_idx = start_slot
                .checked_add(u32::try_from(offset).map_err(|_| PipelineError::QueueFull {
                    queue: "submission",
                    fix: "batch publish offset exceeds u32::MAX; split the publish batch",
                })?)
                .ok_or(PipelineError::QueueFull {
                    queue: "submission",
                    fix: "batch publish slot index overflowed u32; split the publish batch",
                })?;
            write_slot_unchecked(
                ring_bytes,
                view,
                slot_idx,
                tenant_id,
                *opcode,
                args.as_ref(),
            )?;
        }
        let fence_slot = start_slot
            .checked_add(item_count)
            .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix: "batch publish fence slot overflowed u32; split the publish batch",
            })?;
        write_slot_unchecked(
            ring_bytes,
            view,
            fence_slot,
            tenant_id,
            protocol::opcode::BATCH_FENCE,
            &[item_count, batch_tag],
        )?;
        fence_slot
            .checked_add(1)
            .and_then(|end| end.checked_sub(start_slot))
            .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix: "batch publish consumed-slot count overflowed u32; split the publish batch",
            })
    }
}

fn validate_publish_payload(opcode: u32, args: &[u32]) -> Result<(), PipelineError> {
    if args.len() > ARGS_PER_SLOT_USIZE {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "too many args for one slot; 12 u32 args max per slot",
        });
    }
    if let Err(fix) = protocol::opcode::validate_publish_opcode(opcode) {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix,
        });
    }
    Ok(())
}

fn u32_to_usize(value: u32) -> Result<usize, PipelineError> {
    usize::try_from(value).map_err(|_| PipelineError::QueueFull {
        queue: "submission",
        fix: "u32 slot index cannot fit host usize; shard the megakernel ring for this target",
    })
}

fn slot_base(slot_idx: u32, view: RingPublishView) -> Result<usize, PipelineError> {
    u32_to_usize(slot_idx)?
        .checked_mul(view.slot_bytes)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "slot byte offset overflowed usize; shard the ring before publishing",
        })
}

fn validate_publishable_slot(
    ring_bytes: &[u8],
    view: RingPublishView,
    slot_idx: u32,
) -> Result<(), PipelineError> {
    let current_status = read_slot_status_word(ring_bytes, view, slot_idx)?;
    validate_slot_transition(current_status, RingSlotTransition::Publish)
}

fn validate_slot_transition(
    current_status: u32,
    transition: RingSlotTransition,
) -> Result<(), PipelineError> {
    if transition.allows(current_status) {
        return Ok(());
    }
    Err(PipelineError::QueueFull {
        queue: "submission",
        fix: illegal_transition_fix(transition, current_status),
    })
}

fn illegal_transition_fix(transition: RingSlotTransition, _current_status: u32) -> &'static str {
    match transition {
        RingSlotTransition::Publish => {
            "slot is not publishable; only EMPTY and DONE slots may be written by the host"
        }
        RingSlotTransition::Claim => {
            "illegal ring slot transition: claim requires PUBLISHED, YIELD, or REQUEUE status"
        }
        RingSlotTransition::Done => "illegal ring slot transition: done requires CLAIMED status",
        RingSlotTransition::Fault => {
            "illegal ring slot transition: fault requires an in-flight slot status"
        }
        RingSlotTransition::Cancel => {
            "illegal ring slot transition: cancel requires an unclaimed in-flight slot status"
        }
    }
}

fn slot_status_offset(slot_idx: u32, view: RingPublishView) -> Result<usize, PipelineError> {
    let base = slot_base(slot_idx, view)?;
    base.checked_add(
        STATUS_WORD_USIZE
            .checked_mul(4)
            .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix:
                "slot status word byte offset overflowed usize; keep SLOT_WORDS within the u32 ABI",
        })?,
    )
    .ok_or(PipelineError::QueueFull {
        queue: "submission",
        fix: "slot status byte offset overflowed usize; shard the ring before publishing",
    })
}

fn read_slot_status_word(
    ring_bytes: &[u8],
    view: RingPublishView,
    slot_idx: u32,
) -> Result<u32, PipelineError> {
    let status_offset = slot_status_offset(slot_idx, view)?;
    let status_end = status_offset
        .checked_add(4)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "slot status byte end overflowed usize; shard the ring before publishing",
        })?;
    let status_bytes = ring_bytes
        .get(status_offset..status_end)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "slot status is outside the validated ring buffer; validate ring length before publishing",
        })?;
    Ok(u32::from_le_bytes([
        status_bytes[0],
        status_bytes[1],
        status_bytes[2],
        status_bytes[3],
    ]))
}

fn write_slot_status_word(
    ring_bytes: &mut [u8],
    view: RingPublishView,
    slot_idx: u32,
    value: u32,
) -> Result<(), PipelineError> {
    let status_offset = slot_status_offset(slot_idx, view)?;
    let status_end = status_offset
        .checked_add(4)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "slot status byte end overflowed usize; shard the ring before publishing",
        })?;
    let status_bytes = ring_bytes
        .get_mut(status_offset..status_end)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "slot status is outside the validated ring buffer; validate ring length before publishing",
        })?;
    status_bytes.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_slot_unchecked(
    ring_bytes: &mut [u8],
    view: RingPublishView,
    slot_idx: u32,
    tenant_id: u32,
    opcode: u32,
    args: &[u32],
) -> Result<(), PipelineError> {
    let base = slot_base(slot_idx, view)?;
    write_slot_word(ring_bytes, base, OPCODE_WORD_USIZE, opcode);
    write_slot_word(ring_bytes, base, TENANT_WORD_USIZE, tenant_id);
    write_slot_word(
        ring_bytes,
        base,
        PRIORITY_WORD_USIZE,
        scheduler::priority::NORMAL,
    );
    let args_start = base + ARG0_WORD_USIZE * 4;
    let args_end = args_start + ARGS_PER_SLOT_USIZE * 4;
    ring_bytes[args_start..args_end].fill(0);
    for (index, arg) in args.iter().enumerate() {
        write_slot_word(ring_bytes, base, ARG0_WORD_USIZE + index, *arg);
    }
    write_slot_word(ring_bytes, base, STATUS_WORD_USIZE, slot::PUBLISHED);
    Ok(())
}

fn write_work_item_unchecked(
    ring_bytes: &mut [u8],
    view: RingPublishView,
    slot_idx: u32,
    tenant_id: u32,
    item: &MegakernelWorkItem,
) -> Result<(), PipelineError> {
    let base = slot_base(slot_idx, view)?;
    write_slot_word(ring_bytes, base, OPCODE_WORD_USIZE, item.op_handle);
    write_slot_word(ring_bytes, base, TENANT_WORD_USIZE, tenant_id);
    write_slot_word(
        ring_bytes,
        base,
        PRIORITY_WORD_USIZE,
        scheduler::priority::NORMAL,
    );
    let args_start = base + ARG0_WORD_USIZE * 4;
    let args_end = args_start + ARGS_PER_SLOT_USIZE * 4;
    ring_bytes[args_start..args_end].fill(0);
    write_slot_word(ring_bytes, base, ARG0_WORD_USIZE, item.input_handle);
    write_slot_word(ring_bytes, base, ARG0_WORD_USIZE + 1, item.output_handle);
    write_slot_word(ring_bytes, base, ARG0_WORD_USIZE + 2, item.param);
    write_slot_word(ring_bytes, base, STATUS_WORD_USIZE, slot::PUBLISHED);
    Ok(())
}

fn validate_work_item_batch(
    slot_count: u32,
    items: &[MegakernelWorkItem],
) -> Result<(), PipelineError> {
    let item_count = u32::try_from(items.len()).map_err(|_| PipelineError::QueueFull {
        queue: "submission",
        fix: "work item count exceeds u32::MAX; shard the megakernel queue before publishing",
    })?;
    if item_count > slot_count {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "work item count exceeds ring slot count; enlarge the launch geometry before publishing",
        });
    }
    validate_work_items(items)
}

fn validate_work_items(items: &[MegakernelWorkItem]) -> Result<(), PipelineError> {
    if ARGS_PER_SLOT_USIZE < 3 {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "MegakernelWorkItem publication requires three argument words; increase ARGS_PER_SLOT",
        });
    }
    for item in items {
        if let Err(fix) = protocol::opcode::validate_publish_opcode(item.op_handle) {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix,
            });
        }
    }
    Ok(())
}

fn encoded_ring_word_count(slot_count: u32) -> Result<usize, PipelineError> {
    if slot_count > protocol::MAX_ENCODED_RING_SLOTS {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "split the dispatch into smaller ring shards before encoding; slot_count exceeds the megakernel allocation cap or host address space",
        });
    }
    let words = slot_count
        .checked_mul(SLOT_WORDS)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "split the dispatch into smaller ring shards before encoding; slot_count exceeds the megakernel protocol cap or host address space",
        })?;
    usize::try_from(words).map_err(|_| PipelineError::QueueFull {
        queue: "submission",
        fix: "split the dispatch into smaller ring shards before encoding; ring word count does not fit usize",
    })
}

fn write_slot_word(ring_bytes: &mut [u8], slot_base: usize, word_idx: usize, value: u32) {
    let off = slot_base + word_idx * 4;
    ring_bytes[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_packed_metadata_byte(args: &mut [u32; ARGS_PER_SLOT_USIZE], byte_index: usize, value: u8) {
    let word_index = byte_index / 4;
    let shift = ((byte_index % 4) * 8) as u32;
    args[word_index] |= u32::from(value) << shift;
}
