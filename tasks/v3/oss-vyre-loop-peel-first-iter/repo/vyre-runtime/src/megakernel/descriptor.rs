//! Typed host-side descriptors for publishing work into the megakernel ring.
//!
//! Wrappers such as VyreOffload should not have to hand-assemble
//! `(opcode, tenant_id, args)` tuples or know when to switch to the
//! packed-slot path. These descriptors provide an additive typed API
//! over the existing wire protocol.

use super::staging_reserve::reserve_vec_capacity as reserve_descriptor_vec;
use crate::PipelineError;

use smallvec::SmallVec;

const ARGS_PER_SLOT_USIZE: usize = 12;

use super::{protocol, Megakernel};

/// Built-in megakernel opcodes exposed as a typed host API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinOpcode {
    /// No-op heartbeat / probe.
    Nop,
    /// `control[arg1] = arg0`.
    StoreU32,
    /// `atomic_add(control[arg1], arg0)`.
    AtomicAdd,
    /// `control[OBSERVABLE_BASE + arg1] = control[arg0]`.
    LoadU32,
    /// Compare-and-swap on `control[arg0]`.
    CompareSwap,
    /// Copy `arg2` words from `control[arg0]` to `control[arg1]`.
    Memcpy,
    /// Single DFA transition step.
    DfaStep,
    /// Batch fence / epoch bump.
    BatchFence,
    /// Emit a debug log record.
    Printf,
    /// Set `SHUTDOWN=1`.
    Shutdown,
}

impl BuiltinOpcode {
    /// Underlying wire opcode.
    #[must_use]
    pub const fn into_wire(self) -> u32 {
        match self {
            Self::Nop => protocol::opcode::NOP,
            Self::StoreU32 => protocol::opcode::STORE_U32,
            Self::AtomicAdd => protocol::opcode::ATOMIC_ADD,
            Self::LoadU32 => protocol::opcode::LOAD_U32,
            Self::CompareSwap => protocol::opcode::COMPARE_SWAP,
            Self::Memcpy => protocol::opcode::MEMCPY,
            Self::DfaStep => protocol::opcode::DFA_STEP,
            Self::BatchFence => protocol::opcode::BATCH_FENCE,
            Self::Printf => protocol::opcode::PRINTF,
            Self::Shutdown => protocol::opcode::SHUTDOWN,
        }
    }
}

/// A slot opcode can target either a builtin wire opcode or a caller-defined
/// extension registered via an opcode handler (see `handlers` module).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotOpcode {
    /// One of the frozen builtins in [`protocol::opcode`].
    Builtin(BuiltinOpcode),
    /// A custom extension opcode.
    Custom(u32),
}

impl SlotOpcode {
    /// Underlying wire opcode.
    #[must_use]
    pub const fn into_wire(self) -> u32 {
        match self {
            Self::Builtin(op) => op.into_wire(),
            Self::Custom(op) => op,
        }
    }
}

/// One packed inner-op inside a `PACKED_SLOT`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedOpDescriptor {
    /// Inner opcode id. Must fit in `u8` due to the current wire format.
    pub opcode: u8,
    /// Positional `u32` arguments for the inner opcode.
    pub args: Vec<u32>,
}

impl PackedOpDescriptor {
    /// Convenience constructor.
    #[must_use]
    pub fn new(opcode: u8, args: Vec<u32>) -> Self {
        Self { opcode, args }
    }
}

/// One top-level slot publication request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotDescriptor {
    /// Publish one normal slot.
    Single {
        /// Tenant id used for the runtime's authorization mask.
        tenant_id: u32,
        /// Slot opcode.
        opcode: SlotOpcode,
        /// Positional `u32` arguments.
        args: Vec<u32>,
    },
    /// Publish one packed slot containing several inner ops.
    Packed {
        /// Tenant id used for the runtime's authorization mask.
        tenant_id: u32,
        /// Inner packed ops.
        ops: Vec<PackedOpDescriptor>,
    },
}

impl SlotDescriptor {
    /// Build a simple slot descriptor.
    #[must_use]
    pub fn single(tenant_id: u32, opcode: SlotOpcode, args: Vec<u32>) -> Self {
        Self::Single {
            tenant_id,
            opcode,
            args,
        }
    }

    /// Build a packed-slot descriptor.
    #[must_use]
    pub fn packed(tenant_id: u32, ops: Vec<PackedOpDescriptor>) -> Self {
        Self::Packed { tenant_id, ops }
    }

    /// Publish this slot into the ring at `slot_idx`.
    ///
    /// # Errors
    ///
    /// Propagates any wire-level publication error from the underlying ring
    /// protocol helpers.
    pub fn publish_into(&self, ring_bytes: &mut [u8], slot_idx: u32) -> Result<(), PipelineError> {
        match self {
            Self::Single {
                tenant_id,
                opcode,
                args,
            } => {
                Megakernel::publish_slot(ring_bytes, slot_idx, *tenant_id, opcode.into_wire(), args)
            }
            Self::Packed { tenant_id, ops } => {
                Megakernel::publish_packed_descriptors(ring_bytes, slot_idx, *tenant_id, ops)
            }
        }
    }
}

/// A typed batch publication request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchDescriptor {
    /// Slot index where the first item should be written.
    pub start_slot: u32,
    /// Items to publish in order.
    pub items: Vec<SlotDescriptor>,
}

impl BatchDescriptor {
    /// Convenience constructor.
    #[must_use]
    pub fn new(start_slot: u32, items: Vec<SlotDescriptor>) -> Self {
        Self { start_slot, items }
    }

    /// Publish all items into the ring. Returns the number of slots consumed.
    ///
    /// # Errors
    ///
    /// Propagates any slot publication error.
    pub fn publish_into(&self, ring_bytes: &mut [u8]) -> Result<u32, PipelineError> {
        let item_count = u32::try_from(self.items.len()).map_err(|_| PipelineError::QueueFull {
            queue: "submission",
            fix: "batch size exceeds u32::MAX slots",
        })?;
        if item_count > 0 {
            self.start_slot
                .checked_add(item_count - 1)
                .ok_or(PipelineError::QueueFull {
                    queue: "submission",
                    fix: "batch start plus item count overflows u32; split the descriptor batch before publishing",
                })?;
        }
        for (slot_offset, item) in (0..item_count).zip(self.items.iter()) {
            let slot_idx = self
                .start_slot
                .checked_add(slot_offset)
                .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix:
                    "batch slot index overflowed u32; split the descriptor batch before publishing",
            })?;
            item.publish_into(ring_bytes, slot_idx)?;
        }
        Ok(item_count)
    }
}

/// Classification for items published inside a window descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowClass {
    /// Required work that must converge for the window to be usable.
    Required,
    /// Lookahead work that improves the next step but is not on the immediate critical path.
    Lookahead,
}

impl WindowClass {
    /// Stable on-the-wire encoding  -  `Required` = 0, `Lookahead` = 1.
    #[must_use]
    pub const fn into_wire(self) -> u32 {
        match self {
            Self::Required => 0,
            Self::Lookahead => 1,
        }
    }
}

/// A ticketed window of related slot publications.
///
/// Each emitted slot receives a stable prefix of `[window_ticket, class_tag]`
/// followed by the caller-supplied payload, so wrappers can submit required and
/// lookahead work as one structured batch without hand-assembling the prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowDescriptor {
    /// Slot index where the first window item should be written.
    pub start_slot: u32,
    /// Tenant id used for all emitted slots.
    pub tenant_id: u32,
    /// Slot opcode shared by all emitted slots.
    pub opcode: SlotOpcode,
    /// Stable ticket id correlating every slot in this window.
    pub ticket: u32,
    /// Required entries for the window.
    pub required: Vec<Vec<u32>>,
    /// Lookahead entries for the window.
    pub lookahead: Vec<Vec<u32>>,
}

impl WindowDescriptor {
    /// Convenience constructor.
    #[must_use]
    pub fn new(
        start_slot: u32,
        tenant_id: u32,
        opcode: SlotOpcode,
        ticket: u32,
        required: Vec<Vec<u32>>,
        lookahead: Vec<Vec<u32>>,
    ) -> Self {
        Self {
            start_slot,
            tenant_id,
            opcode,
            ticket,
            required,
            lookahead,
        }
    }

    /// Convert the window into a typed batch publication.
    ///
    /// # Panics
    ///
    /// Panics when `try_into_batch` fails (oversized payload, OOM, or slot
    /// overflow). The panic message names the failing condition and directs the
    /// caller to `try_into_batch`. Panicking here is intentional: silently
    /// returning an empty `BatchDescriptor` would publish zero ring slots and
    /// lose all window work with no operator signal (Law 10). In production
    /// code, call `try_into_batch` directly so errors can be propagated.
    #[must_use]
    pub fn into_batch(&self) -> BatchDescriptor {
        self.try_into_batch().unwrap_or_else(|e| {
            panic!(
                "WindowDescriptor::into_batch failed: {e}. Fix: call try_into_batch() and propagate the error instead of using the infallible wrapper."
            )
        })
    }

    /// Convert the window into a typed batch publication with explicit staging
    /// and ABI-bound errors.
    pub fn try_into_batch(&self) -> Result<BatchDescriptor, PipelineError> {
        let item_count = self
            .required
            .len()
            .checked_add(self.lookahead.len())
            .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix:
                "window item count overflowed usize; split the window before materializing a batch",
        })?;
        let mut items = Vec::new();
        reserve_descriptor_vec(&mut items, item_count, "window batch item")?;
        for payload in &self.required {
            let mut args = window_payload_args(self.ticket, WindowClass::Required, payload)?;
            args.push(self.ticket);
            args.push(WindowClass::Required.into_wire());
            args.extend(payload.iter().copied());
            items.push(SlotDescriptor::single(self.tenant_id, self.opcode, args));
        }
        for payload in &self.lookahead {
            let mut args = window_payload_args(self.ticket, WindowClass::Lookahead, payload)?;
            args.push(self.ticket);
            args.push(WindowClass::Lookahead.into_wire());
            args.extend(payload.iter().copied());
            items.push(SlotDescriptor::single(self.tenant_id, self.opcode, args));
        }
        Ok(BatchDescriptor::new(self.start_slot, items))
    }

    /// Publish the full window into the ring and return the number of emitted slots.
    pub fn publish_into(&self, ring_bytes: &mut [u8]) -> Result<u32, PipelineError> {
        let consumed = self
            .required
            .len()
            .checked_add(self.lookahead.len())
            .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix: "window item count overflowed usize; split the window before publishing",
            })?;
        let consumed_u32 = u32::try_from(consumed).map_err(|_| PipelineError::QueueFull {
            queue: "submission",
            fix: "window size exceeds u32::MAX slots; split the window before publishing",
        })?;
        if consumed_u32 == 0 {
            return Ok(0);
        }
        self.start_slot
            .checked_add(consumed_u32 - 1)
            .ok_or(PipelineError::QueueFull {
                queue: "submission",
                fix: "window start plus item count overflows u32; split the window before publishing",
            })?;

        let mut slot_offset = 0u32;
        let mut args = SmallVec::<[u32; ARGS_PER_SLOT_USIZE]>::new();
        for payload in &self.required {
            publish_window_payload(
                ring_bytes,
                self.start_slot,
                &mut slot_offset,
                self.tenant_id,
                self.opcode,
                self.ticket,
                WindowClass::Required,
                payload,
                &mut args,
            )?;
        }
        for payload in &self.lookahead {
            publish_window_payload(
                ring_bytes,
                self.start_slot,
                &mut slot_offset,
                self.tenant_id,
                self.opcode,
                self.ticket,
                WindowClass::Lookahead,
                payload,
                &mut args,
            )?;
        }
        Ok(slot_offset)
    }
}

fn window_payload_args(
    _ticket: u32,
    _class: WindowClass,
    payload: &[u32],
) -> Result<Vec<u32>, PipelineError> {
    let required_args = payload
        .len()
        .checked_add(2)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "window payload argument count overflowed usize; split the payload before materializing a batch",
        })?;
    if required_args > ARGS_PER_SLOT_USIZE {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "too many args for one window payload; ticket plus class plus payload must fit in 12 u32 args",
        });
    }
    let mut args = Vec::new();
    reserve_descriptor_vec(&mut args, required_args, "window payload arg")?;
    Ok(args)
}

fn publish_window_payload(
    ring_bytes: &mut [u8],
    start_slot: u32,
    slot_offset: &mut u32,
    tenant_id: u32,
    opcode: SlotOpcode,
    ticket: u32,
    class: WindowClass,
    payload: &[u32],
    args: &mut SmallVec<[u32; ARGS_PER_SLOT_USIZE]>,
) -> Result<(), PipelineError> {
    let slot_idx = start_slot
        .checked_add(*slot_offset)
        .ok_or(PipelineError::QueueFull {
            queue: "submission",
            fix: "window slot index overflowed u32; split the window before publishing",
        })?;
    args.clear();
    let required_args = payload
        .len()
        .checked_add(2)
        .ok_or(PipelineError::QueueFull {
        queue: "submission",
        fix: "window payload argument count overflowed usize; split the payload before publishing",
    })?;
    if required_args > ARGS_PER_SLOT_USIZE {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "too many args for one window payload; ticket plus class plus payload must fit in 12 u32 args",
        });
    }
    args.push(ticket);
    args.push(class.into_wire());
    args.extend_from_slice(payload);
    Megakernel::publish_slot(ring_bytes, slot_idx, tenant_id, opcode.into_wire(), args)?;
    *slot_offset = slot_offset.checked_add(1).ok_or(PipelineError::QueueFull {
        queue: "submission",
        fix: "window slot count overflowed u32; split the window before publishing",
    })?;
    Ok(())
}

#[cfg(test)]
mod tests;
