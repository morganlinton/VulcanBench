/// Invalid performance proof input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityPerformanceProofError {
    /// clang wall time was zero, so no speedup ratio can be computed.
    ZeroClangWallTime,
    /// vyrec wall time was zero, so no finite measured speedup ratio can be computed.
    ZeroVyrecWallTime,
    /// Required speedup was zero, which would make the performance gate meaningless.
    ZeroRequiredSpeedup,
}

/// Measured speedup proof for one release target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParityPerformanceProof {
    /// clang oracle wall time in nanoseconds.
    pub clang_wall_ns: u64,
    /// vyrec wall time in nanoseconds.
    pub vyrec_wall_ns: u64,
    /// Required speedup multiplier scaled by 1000.
    ///
    /// `100_000` means `100.000x`.
    pub required_speedup_x1000: u64,
}

impl ParityPerformanceProof {
    /// Creates a validated performance proof.
    ///
    /// The constructor rejects zero values because zero-time inputs would turn
    /// the release gate into a benchmark artifact bug rather than a speedup
    /// proof.
    pub fn new(
        clang_wall_ns: u64,
        vyrec_wall_ns: u64,
        required_speedup_x1000: u64,
    ) -> Result<Self, ParityPerformanceProofError> {
        if clang_wall_ns == 0 {
            return Err(ParityPerformanceProofError::ZeroClangWallTime);
        }
        if vyrec_wall_ns == 0 {
            return Err(ParityPerformanceProofError::ZeroVyrecWallTime);
        }
        if required_speedup_x1000 == 0 {
            return Err(ParityPerformanceProofError::ZeroRequiredSpeedup);
        }
        Ok(Self {
            clang_wall_ns,
            vyrec_wall_ns,
            required_speedup_x1000,
        })
    }

    /// Returns measured speedup scaled by 1000.
    #[must_use]
    pub fn measured_speedup_x1000(self) -> u64 {
        ((self.clang_wall_ns as u128 * 1000) / self.vyrec_wall_ns as u128) as u64
    }

    /// Returns whether the measured speedup satisfies the required contract.
    #[must_use]
    pub fn passes_contract(self) -> bool {
        (self.clang_wall_ns as u128 * 1000)
            >= (self.vyrec_wall_ns as u128 * self.required_speedup_x1000 as u128)
    }
}
