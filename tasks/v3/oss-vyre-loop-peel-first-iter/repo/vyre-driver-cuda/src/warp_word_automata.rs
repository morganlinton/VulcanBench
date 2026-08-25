//! CUDA warp-word bit-parallel automata layout evidence.
//!
//! The scan compiler owns NFA/DFA construction and table semantics. This module
//! owns only CUDA-side layout metadata for deciding whether an eligible
//! bit-parallel automata program can be promoted to a warp-word kernel and how
//! that promotion is reported against a table-driven DFA baseline.

/// Schema version for CUDA warp-word automata layout evidence.
pub const CUDA_WARP_WORD_AUTOMATA_LAYOUT_SCHEMA_VERSION: u32 = 1;

const WARP_WORD_LAYOUT_FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const WARP_WORD_LAYOUT_FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Instruction class selected for a CUDA warp-word automata layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CudaWarpWordInstructionClass {
    /// One warp can hold the live NFA state set in lane-local words.
    SingleWarpBitParallel,
    /// Multiple warp-word groups are required for the live NFA state set.
    MultiWarpBitParallel,
}

impl CudaWarpWordInstructionClass {
    /// Stable evidence label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleWarpBitParallel => "single_warp_bit_parallel",
            Self::MultiWarpBitParallel => "multi_warp_bit_parallel",
        }
    }

    const fn tag(self) -> u64 {
        match self {
            Self::SingleWarpBitParallel => 1,
            Self::MultiWarpBitParallel => 2,
        }
    }
}

/// Request for CUDA warp-word automata layout evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CudaWarpWordAutomataLayoutRequest {
    /// NFA state count from the shared scan compiler.
    pub nfa_state_count: u32,
    /// Pattern count represented by the automata program.
    pub pattern_count: u32,
    /// Haystack chunk bytes processed by the candidate kernel.
    pub haystack_chunk_bytes: u64,
    /// CUDA warp size reported by the device capability probe.
    pub warp_size: u32,
    /// Bits stored in one lane-local automata word.
    pub word_bits: u32,
    /// Estimated registers per thread for the bit-parallel candidate.
    pub bit_parallel_registers_per_thread: u16,
    /// Maximum registers per thread allowed by the promotion policy.
    pub max_registers_per_thread: u16,
    /// Shared memory required by the candidate.
    pub shared_memory_bytes: u32,
    /// Shared memory budget allowed by the promotion policy.
    pub max_shared_memory_bytes: u32,
    /// Throughput measured or projected for the bit-parallel candidate.
    pub bit_parallel_throughput_bytes_per_second: u64,
    /// Throughput measured or projected for the table-driven DFA baseline.
    pub table_driven_throughput_bytes_per_second: u64,
    /// Match digest for the bit-parallel candidate.
    pub bit_parallel_match_digest: u64,
    /// Match digest for the table-driven DFA baseline.
    pub table_driven_match_digest: u64,
}

impl CudaWarpWordAutomataLayoutRequest {
    /// Construct a CUDA warp-word automata layout request.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        nfa_state_count: u32,
        pattern_count: u32,
        haystack_chunk_bytes: u64,
        warp_size: u32,
        word_bits: u32,
        bit_parallel_registers_per_thread: u16,
        max_registers_per_thread: u16,
        shared_memory_bytes: u32,
        max_shared_memory_bytes: u32,
        bit_parallel_throughput_bytes_per_second: u64,
        table_driven_throughput_bytes_per_second: u64,
        bit_parallel_match_digest: u64,
        table_driven_match_digest: u64,
    ) -> Self {
        Self {
            nfa_state_count,
            pattern_count,
            haystack_chunk_bytes,
            warp_size,
            word_bits,
            bit_parallel_registers_per_thread,
            max_registers_per_thread,
            shared_memory_bytes,
            max_shared_memory_bytes,
            bit_parallel_throughput_bytes_per_second,
            table_driven_throughput_bytes_per_second,
            bit_parallel_match_digest,
            table_driven_match_digest,
        }
    }
}

/// Evidence for one CUDA warp-word automata layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CudaWarpWordAutomataLayoutEvidence {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Selected instruction class.
    pub instruction_class: CudaWarpWordInstructionClass,
    /// NFA state count from the shared scan compiler.
    pub nfa_state_count: u32,
    /// Pattern count represented by the automata program.
    pub pattern_count: u32,
    /// Haystack chunk bytes processed by the candidate kernel.
    pub haystack_chunk_bytes: u64,
    /// CUDA warp size reported by the device capability probe.
    pub warp_size: u32,
    /// Bits stored in one lane-local automata word.
    pub word_bits: u32,
    /// Words required to represent one NFA state set.
    pub state_set_words: u32,
    /// Warp-word groups required by the layout.
    pub warp_word_groups: u32,
    /// Active lanes in the final warp-word group.
    pub active_lanes_in_tail_group: u32,
    /// Lane utilization in basis points for the final warp-word group.
    pub lane_utilization_bps: u16,
    /// Estimated registers per thread for the bit-parallel candidate.
    pub bit_parallel_registers_per_thread: u16,
    /// Maximum registers per thread allowed by the promotion policy.
    pub max_registers_per_thread: u16,
    /// Shared memory required by the candidate.
    pub shared_memory_bytes: u32,
    /// Shared memory budget allowed by the promotion policy.
    pub max_shared_memory_bytes: u32,
    /// Register and lane utilization occupancy proxy in basis points.
    pub occupancy_proxy_bps: u16,
    /// Throughput measured or projected for the bit-parallel candidate.
    pub bit_parallel_throughput_bytes_per_second: u64,
    /// Throughput measured or projected for the table-driven DFA baseline.
    pub table_driven_throughput_bytes_per_second: u64,
    /// Bit-parallel throughput divided by table-driven throughput in basis points.
    pub throughput_speedup_bps: u64,
    /// Stable match digest when both engines agree.
    pub match_digest: u64,
    /// True when bit-parallel and table-driven outputs match.
    pub match_parity: bool,
    /// Deterministic digest of the layout evidence.
    pub layout_digest: u64,
}

impl CudaWarpWordAutomataLayoutEvidence {
    /// Return true when this evidence is complete enough for benchmark claims.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.schema_version == CUDA_WARP_WORD_AUTOMATA_LAYOUT_SCHEMA_VERSION
            && self.nfa_state_count != 0
            && self.pattern_count != 0
            && self.haystack_chunk_bytes != 0
            && self.warp_size != 0
            && self.word_bits != 0
            && self.state_set_words != 0
            && self.warp_word_groups != 0
            && self.active_lanes_in_tail_group != 0
            && self.bit_parallel_throughput_bytes_per_second != 0
            && self.table_driven_throughput_bytes_per_second != 0
            && self.match_digest != 0
            && self.match_parity
            && self.layout_digest != 0
    }
}

/// CUDA warp-word automata layout planning error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CudaWarpWordAutomataLayoutError {
    /// The NFA contains no states.
    ZeroNfaStates,
    /// The pattern set is empty.
    ZeroPatterns,
    /// The haystack chunk size is zero.
    ZeroHaystackChunk,
    /// The CUDA warp size is unsupported.
    InvalidWarpSize {
        /// Warp size supplied by the caller.
        warp_size: u32,
    },
    /// The automata word size is unsupported.
    InvalidWordBits {
        /// Word size supplied by the caller.
        word_bits: u32,
    },
    /// State-set layout arithmetic overflowed.
    LayoutOverflow,
    /// Register use exceeds the configured promotion budget.
    RegisterBudgetExceeded {
        /// Candidate register estimate.
        bit_parallel_registers_per_thread: u16,
        /// Configured promotion ceiling.
        max_registers_per_thread: u16,
    },
    /// Shared memory exceeds the configured promotion budget.
    SharedMemoryBudgetExceeded {
        /// Candidate shared memory usage.
        shared_memory_bytes: u32,
        /// Configured promotion ceiling.
        max_shared_memory_bytes: u32,
    },
    /// Throughput evidence is missing.
    ZeroThroughput {
        /// Throughput field that was zero.
        field: &'static str,
    },
    /// Match digest evidence is missing.
    ZeroMatchDigest {
        /// Match digest field that was zero.
        field: &'static str,
    },
    /// Candidate output diverged from the table-driven baseline.
    MatchDigestMismatch {
        /// Bit-parallel candidate digest.
        bit_parallel_match_digest: u64,
        /// Table-driven DFA baseline digest.
        table_driven_match_digest: u64,
    },
}

impl std::fmt::Display for CudaWarpWordAutomataLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroNfaStates => formatter.write_str(
                "CUDA warp-word automata layout has zero NFA states. Fix: pass the shared scan compiler state count before promotion.",
            ),
            Self::ZeroPatterns => formatter.write_str(
                "CUDA warp-word automata layout has zero patterns. Fix: pass a non-empty pattern set before promotion.",
            ),
            Self::ZeroHaystackChunk => formatter.write_str(
                "CUDA warp-word automata layout has zero haystack chunk bytes. Fix: bind a concrete scan chunk before benchmark evidence.",
            ),
            Self::InvalidWarpSize { warp_size } => write!(
                formatter,
                "CUDA warp-word automata layout received warp_size={warp_size}. Fix: use a probed CUDA warp size in 1..=64 before promotion."
            ),
            Self::InvalidWordBits { word_bits } => write!(
                formatter,
                "CUDA warp-word automata layout received word_bits={word_bits}. Fix: use 32 or 64 bit lane-local automata words."
            ),
            Self::LayoutOverflow => formatter.write_str(
                "CUDA warp-word automata layout arithmetic overflowed. Fix: shard the automata state set before promotion.",
            ),
            Self::RegisterBudgetExceeded {
                bit_parallel_registers_per_thread,
                max_registers_per_thread,
            } => write!(
                formatter,
                "CUDA warp-word automata register estimate {bit_parallel_registers_per_thread} exceeds budget {max_registers_per_thread}. Fix: keep the table-driven DFA path or split the bit-parallel kernel."
            ),
            Self::SharedMemoryBudgetExceeded {
                shared_memory_bytes,
                max_shared_memory_bytes,
            } => write!(
                formatter,
                "CUDA warp-word automata shared memory {shared_memory_bytes} exceeds budget {max_shared_memory_bytes}. Fix: keep the table-driven DFA path or shard resident tables."
            ),
            Self::ZeroThroughput { field } => write!(
                formatter,
                "CUDA warp-word automata throughput field {field} is zero. Fix: record candidate and table-driven baseline throughput before claiming promotion."
            ),
            Self::ZeroMatchDigest { field } => write!(
                formatter,
                "CUDA warp-word automata match digest field {field} is zero. Fix: compute candidate and table-driven baseline digests before claiming parity."
            ),
            Self::MatchDigestMismatch {
                bit_parallel_match_digest,
                table_driven_match_digest,
            } => write!(
                formatter,
                "CUDA warp-word automata match digest mismatch bit_parallel={bit_parallel_match_digest:#x} table_driven={table_driven_match_digest:#x}. Fix: reject the bit-parallel PTX candidate until output parity is restored."
            ),
        }
    }
}

impl std::error::Error for CudaWarpWordAutomataLayoutError {}

/// Plan CUDA warp-word automata layout evidence.
///
/// # Errors
///
/// Returns [`CudaWarpWordAutomataLayoutError`] when layout arithmetic,
/// register/shared-memory budgets, throughput evidence, or match parity fails.
pub fn plan_cuda_warp_word_automata_layout(
    request: CudaWarpWordAutomataLayoutRequest,
) -> Result<CudaWarpWordAutomataLayoutEvidence, CudaWarpWordAutomataLayoutError> {
    if request.nfa_state_count == 0 {
        return Err(CudaWarpWordAutomataLayoutError::ZeroNfaStates);
    }
    if request.pattern_count == 0 {
        return Err(CudaWarpWordAutomataLayoutError::ZeroPatterns);
    }
    if request.haystack_chunk_bytes == 0 {
        return Err(CudaWarpWordAutomataLayoutError::ZeroHaystackChunk);
    }
    if request.warp_size == 0 || request.warp_size > 64 {
        return Err(CudaWarpWordAutomataLayoutError::InvalidWarpSize {
            warp_size: request.warp_size,
        });
    }
    if !matches!(request.word_bits, 32 | 64) {
        return Err(CudaWarpWordAutomataLayoutError::InvalidWordBits {
            word_bits: request.word_bits,
        });
    }
    if request.bit_parallel_registers_per_thread > request.max_registers_per_thread {
        return Err(CudaWarpWordAutomataLayoutError::RegisterBudgetExceeded {
            bit_parallel_registers_per_thread: request.bit_parallel_registers_per_thread,
            max_registers_per_thread: request.max_registers_per_thread,
        });
    }
    if request.shared_memory_bytes > request.max_shared_memory_bytes {
        return Err(CudaWarpWordAutomataLayoutError::SharedMemoryBudgetExceeded {
            shared_memory_bytes: request.shared_memory_bytes,
            max_shared_memory_bytes: request.max_shared_memory_bytes,
        });
    }
    if request.bit_parallel_throughput_bytes_per_second == 0 {
        return Err(CudaWarpWordAutomataLayoutError::ZeroThroughput {
            field: "bit_parallel_throughput_bytes_per_second",
        });
    }
    if request.table_driven_throughput_bytes_per_second == 0 {
        return Err(CudaWarpWordAutomataLayoutError::ZeroThroughput {
            field: "table_driven_throughput_bytes_per_second",
        });
    }
    if request.bit_parallel_match_digest == 0 {
        return Err(CudaWarpWordAutomataLayoutError::ZeroMatchDigest {
            field: "bit_parallel_match_digest",
        });
    }
    if request.table_driven_match_digest == 0 {
        return Err(CudaWarpWordAutomataLayoutError::ZeroMatchDigest {
            field: "table_driven_match_digest",
        });
    }
    if request.bit_parallel_match_digest != request.table_driven_match_digest {
        return Err(CudaWarpWordAutomataLayoutError::MatchDigestMismatch {
            bit_parallel_match_digest: request.bit_parallel_match_digest,
            table_driven_match_digest: request.table_driven_match_digest,
        });
    }

    let state_set_words = ceil_div_u32(request.nfa_state_count, request.word_bits)?;
    let warp_word_groups = ceil_div_u32(state_set_words, request.warp_size)?;
    let tail_remainder = state_set_words % request.warp_size;
    let active_lanes_in_tail_group = if tail_remainder == 0 {
        request.warp_size
    } else {
        tail_remainder
    };
    let instruction_class = if warp_word_groups == 1 {
        CudaWarpWordInstructionClass::SingleWarpBitParallel
    } else {
        CudaWarpWordInstructionClass::MultiWarpBitParallel
    };
    let lane_utilization_bps =
        ((u64::from(active_lanes_in_tail_group) * 10_000) / u64::from(request.warp_size)) as u16;
    let register_headroom_bps = register_headroom_bps(
        request.bit_parallel_registers_per_thread,
        request.max_registers_per_thread,
    );
    let occupancy_proxy_bps =
        ((u64::from(lane_utilization_bps) * u64::from(register_headroom_bps)) / 10_000) as u16;
    let speedup_u128 = (request.bit_parallel_throughput_bytes_per_second as u128)
        .checked_mul(10_000)
        .ok_or(CudaWarpWordAutomataLayoutError::LayoutOverflow)?
        / u128::from(request.table_driven_throughput_bytes_per_second);
    let throughput_speedup_bps =
        u64::try_from(speedup_u128).map_err(|_| CudaWarpWordAutomataLayoutError::LayoutOverflow)?;

    let mut evidence = CudaWarpWordAutomataLayoutEvidence {
        schema_version: CUDA_WARP_WORD_AUTOMATA_LAYOUT_SCHEMA_VERSION,
        instruction_class,
        nfa_state_count: request.nfa_state_count,
        pattern_count: request.pattern_count,
        haystack_chunk_bytes: request.haystack_chunk_bytes,
        warp_size: request.warp_size,
        word_bits: request.word_bits,
        state_set_words,
        warp_word_groups,
        active_lanes_in_tail_group,
        lane_utilization_bps,
        bit_parallel_registers_per_thread: request.bit_parallel_registers_per_thread,
        max_registers_per_thread: request.max_registers_per_thread,
        shared_memory_bytes: request.shared_memory_bytes,
        max_shared_memory_bytes: request.max_shared_memory_bytes,
        occupancy_proxy_bps,
        bit_parallel_throughput_bytes_per_second: request
            .bit_parallel_throughput_bytes_per_second,
        table_driven_throughput_bytes_per_second: request
            .table_driven_throughput_bytes_per_second,
        throughput_speedup_bps,
        match_digest: request.bit_parallel_match_digest,
        match_parity: true,
        layout_digest: 0,
    };
    evidence.layout_digest = cuda_warp_word_automata_layout_digest(evidence);
    Ok(evidence)
}

fn ceil_div_u32(numerator: u32, denominator: u32) -> Result<u32, CudaWarpWordAutomataLayoutError> {
    numerator
        .checked_add(denominator - 1)
        .ok_or(CudaWarpWordAutomataLayoutError::LayoutOverflow)
        .map(|value| value / denominator)
}

fn register_headroom_bps(used: u16, max: u16) -> u16 {
    if max == 0 {
        return 0;
    }
    (((u64::from(max - used) + 1) * 10_000) / u64::from(max)) as u16
}

fn cuda_warp_word_automata_layout_digest(evidence: CudaWarpWordAutomataLayoutEvidence) -> u64 {
    let mut digest = WARP_WORD_LAYOUT_FNV_OFFSET;
    digest = mix_warp_word_layout_digest(digest, evidence.instruction_class.tag());
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.nfa_state_count));
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.pattern_count));
    digest = mix_warp_word_layout_digest(digest, evidence.haystack_chunk_bytes);
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.warp_size));
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.word_bits));
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.state_set_words));
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.warp_word_groups));
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.active_lanes_in_tail_group));
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.lane_utilization_bps));
    digest = mix_warp_word_layout_digest(
        digest,
        u64::from(evidence.bit_parallel_registers_per_thread),
    );
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.max_registers_per_thread));
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.shared_memory_bytes));
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.max_shared_memory_bytes));
    digest = mix_warp_word_layout_digest(digest, u64::from(evidence.occupancy_proxy_bps));
    digest = mix_warp_word_layout_digest(
        digest,
        evidence.bit_parallel_throughput_bytes_per_second,
    );
    digest = mix_warp_word_layout_digest(
        digest,
        evidence.table_driven_throughput_bytes_per_second,
    );
    digest = mix_warp_word_layout_digest(digest, evidence.throughput_speedup_bps);
    mix_warp_word_layout_digest(digest, evidence.match_digest)
}

fn mix_warp_word_layout_digest(mut digest: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(WARP_WORD_LAYOUT_FNV_PRIME);
    }
    digest
}

#[cfg(test)]
mod tests {
    use super::{
        plan_cuda_warp_word_automata_layout, CudaWarpWordAutomataLayoutError,
        CudaWarpWordAutomataLayoutRequest, CudaWarpWordInstructionClass,
        CUDA_WARP_WORD_AUTOMATA_LAYOUT_SCHEMA_VERSION,
    };

    fn request() -> CudaWarpWordAutomataLayoutRequest {
        CudaWarpWordAutomataLayoutRequest::new(
            96,
            12,
            1 << 20,
            32,
            32,
            48,
            128,
            2048,
            49_152,
            12_000_000_000,
            8_000_000_000,
            0xfeed,
            0xfeed,
        )
    }

    #[test]
    fn warp_word_layout_reports_instruction_register_occupancy_throughput_and_parity() {
        let evidence = plan_cuda_warp_word_automata_layout(request())
            .expect("Fix: valid CUDA warp-word automata layout should be accepted");

        assert_eq!(
            evidence.schema_version,
            CUDA_WARP_WORD_AUTOMATA_LAYOUT_SCHEMA_VERSION
        );
        assert_eq!(
            evidence.instruction_class,
            CudaWarpWordInstructionClass::SingleWarpBitParallel
        );
        assert_eq!(evidence.state_set_words, 3);
        assert_eq!(evidence.warp_word_groups, 1);
        assert_eq!(evidence.active_lanes_in_tail_group, 3);
        assert_eq!(evidence.lane_utilization_bps, 937);
        assert_eq!(evidence.bit_parallel_registers_per_thread, 48);
        assert!(evidence.occupancy_proxy_bps > 0);
        assert_eq!(evidence.throughput_speedup_bps, 15_000);
        assert_eq!(evidence.match_digest, 0xfeed);
        assert!(evidence.match_parity);
        assert!(evidence.is_complete());
    }

    #[test]
    fn warp_word_layout_rejects_register_budget_overflow() {
        let mut request = request();
        request.bit_parallel_registers_per_thread = 129;
        request.max_registers_per_thread = 128;

        assert!(matches!(
            plan_cuda_warp_word_automata_layout(request),
            Err(CudaWarpWordAutomataLayoutError::RegisterBudgetExceeded {
                bit_parallel_registers_per_thread: 129,
                max_registers_per_thread: 128
            })
        ));
    }

    #[test]
    fn warp_word_layout_rejects_digest_drift_against_table_driven_baseline() {
        let mut request = request();
        request.table_driven_match_digest = 0xbeef;

        assert!(matches!(
            plan_cuda_warp_word_automata_layout(request),
            Err(CudaWarpWordAutomataLayoutError::MatchDigestMismatch {
                bit_parallel_match_digest: 0xfeed,
                table_driven_match_digest: 0xbeef
            })
        ));
    }
}
