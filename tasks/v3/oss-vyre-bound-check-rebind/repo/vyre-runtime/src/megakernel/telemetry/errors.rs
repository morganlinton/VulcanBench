use crate::PipelineError;

pub(super) fn slot_word_offset_overflow() -> PipelineError {
    PipelineError::Backend(
        "megakernel telemetry slot word byte offset overflowed usize. Fix: keep slot word indices within host address space."
            .to_string(),
    )
}

pub(super) fn slot_word_end_overflow() -> PipelineError {
    PipelineError::Backend(
        "megakernel telemetry slot word byte end overflowed usize. Fix: keep slot word indices within host address space."
            .to_string(),
    )
}

pub(super) fn missing_slot_word(word_idx: usize, byte_len: usize) -> PipelineError {
    PipelineError::Backend(format!(
        "megakernel telemetry slot chunk is missing word {word_idx} in {byte_len} bytes. Fix: capture complete ring slots before telemetry decode."
    ))
}

pub(super) fn done_counter_backwards(previous: u32, current: u32) -> PipelineError {
    PipelineError::Backend(format!(
        "megakernel done counter moved backwards from {previous} to {current}. Fix: treat counter reset/wrap as a new telemetry epoch."
    ))
}

pub(super) fn launch_telemetry_failed(source: PipelineError) -> vyre_driver::BackendError {
    vyre_driver::BackendError::new(format!(
        "megakernel launch telemetry aggregation failed: {source}"
    ))
}

pub(super) fn hot_opcode_count_overflow(source: impl core::fmt::Display) -> vyre_driver::BackendError {
    vyre_driver::BackendError::new(format!(
        "megakernel hot opcode count cannot fit u32: {source}. Fix: cap metrics slots at the protocol boundary."
    ))
}

pub(super) fn route_window_demand_overflow() -> vyre_driver::BackendError {
    vyre_driver::BackendError::new(
        "megakernel route-window slot demand overflowed u32. Fix: shard route windows before telemetry aggregation."
            .to_string(),
    )
}

pub(super) fn hot_window_count_overflow() -> vyre_driver::BackendError {
    vyre_driver::BackendError::new(
        "megakernel hot window count overflowed usize. Fix: shard telemetry windows before launch recommendation."
            .to_string(),
    )
}

pub(super) fn hot_window_count_too_wide(source: impl core::fmt::Display) -> vyre_driver::BackendError {
    vyre_driver::BackendError::new(format!(
        "megakernel hot window count cannot fit u32: {source}. Fix: shard telemetry windows before launch recommendation."
    ))
}

pub(super) fn requeue_count_overflow() -> vyre_driver::BackendError {
    vyre_driver::BackendError::new(
        "megakernel requeue count overflowed u64. Fix: shard telemetry windows before launch recommendation."
            .to_string(),
    )
}

pub(super) fn missing_control_word(word_idx: usize) -> PipelineError {
    PipelineError::Backend(format!(
        "megakernel control snapshot is missing required word {word_idx}. Fix: capture the full control buffer before telemetry decode."
    ))
}

pub(super) fn density_bps_overflow(source: impl core::fmt::Display) -> PipelineError {
    PipelineError::Backend(format!(
        "megakernel density bps cannot fit u16 after clamp: {source}. Fix: repair density accounting."
    ))
}

pub(super) fn ring_slot_alignment(byte_len: usize, slot_bytes: usize) -> PipelineError {
    PipelineError::Backend(format!(
        "megakernel ring snapshot has {byte_len} bytes, not a multiple of slot size {slot_bytes}. Fix: capture whole ring slots."
    ))
}

pub(super) fn ring_slot_count_too_wide(slot_count: usize) -> PipelineError {
    PipelineError::Backend(format!(
        "megakernel ring snapshot has {slot_count} slots, above the u32 telemetry ABI. Fix: shard ring snapshots before host decode."
    ))
}

pub(super) fn control_length_overflow() -> PipelineError {
    PipelineError::Backend(
        "megakernel control length overflowed usize. Fix: keep protocol constants bounded."
            .to_string(),
    )
}

pub(super) fn bad_control_snapshot(byte_len: usize, min_control: usize) -> PipelineError {
    PipelineError::Backend(format!(
        "megakernel control snapshot has {byte_len} bytes, expected at least {min_control} and 4-byte alignment. Fix: capture the full control buffer."
    ))
}

pub(super) fn slot_byte_width_overflow() -> PipelineError {
    PipelineError::Backend(
        "megakernel telemetry slot byte width overflowed usize. Fix: keep SLOT_WORDS within host address space."
            .to_string(),
    )
}

pub(super) fn telemetry_u32_to_usize(
    value: u32,
    label: &'static str,
    source: impl core::fmt::Display,
) -> PipelineError {
    PipelineError::Backend(format!(
        "megakernel telemetry {label} value {value} cannot fit usize: {source}. Fix: shard telemetry buffers before host decode."
    ))
}

pub(super) fn control_word_index(word: u32, source: impl core::fmt::Display) -> PipelineError {
    PipelineError::Backend(format!(
        "megakernel control word index {word} cannot fit usize: {source}. Fix: keep control ABI words within host address space."
    ))
}

pub(super) fn control_word_offset_overflow() -> PipelineError {
    PipelineError::Backend(
        "megakernel control word offset overflowed u32. Fix: shard telemetry arrays before host decode."
            .to_string(),
    )
}

pub(super) fn counter_sum_overflow(label: &'static str, fix: &'static str) -> PipelineError {
    PipelineError::Backend(format!("megakernel {label} overflowed u64. Fix: {fix}."))
}

pub(super) fn fairness_skew_invalid(max: u32, min_nonzero: u32) -> PipelineError {
    PipelineError::Backend(format!(
        "megakernel fairness skew saw max {max} below min_nonzero {min_nonzero}. Fix: reject malformed fairness counters before telemetry aggregation."
    ))
}
