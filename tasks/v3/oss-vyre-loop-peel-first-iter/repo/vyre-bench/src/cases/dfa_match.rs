use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// DFA Match benchmark  -  full grid-strided word-aligned literal scan.
///
/// Scans a 256KB text buffer for every 4-byte-aligned occurrence of `b"vyre"`.
/// The grid is auto-inferred from the text buffer's element count (65536 u32
/// words), so every word is visited exactly once by a distinct thread.
///
/// The CPU baseline uses `memchr::memmem::find_iter` at byte granularity,
/// but the GPU version aligns to u32 word boundaries. To make the parity
/// check valid, we plant the needle only at word-aligned offsets.
pub struct DfaMatch;

const WORD_COUNT: u32 = 65_536; // 256K bytes / 4 bytes per u32

impl BenchCase for DfaMatch {
    fn id(&self) -> BenchId {
        BenchId("foundation.dfa_match.256k".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "DFA Match 256K".to_string(),
            description:
                "Full-coverage word-aligned literal scan over 256K bytes with atomic match counting"
                    .to_string(),
            tags: vec![
                "compute".to_string(),
                "branching".to_string(),
                "atomic".to_string(),
            ],
            layer: BenchLayer::Foundation,
            workload: WorkloadClass::Micro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
        }
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        None
    }

    fn bytes_touched(&self, _prepared: &PreparedCase) -> (u64, u64) {
        (WORD_COUNT as u64 * 4, 4)
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        // The text buffer has 65536 elements. With workgroup [256,1,1], the
        // driver infers ceil(65536/256) = 256 workgroups → 65536 total threads.
        // Each thread checks exactly one word via gid_x().
        let prog = Program::wrapped(
            vec![
                BufferDecl::output("out_matches", 0, DataType::U32).with_count(1),
                BufferDecl::storage("text", 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(WORD_COUNT),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("idx", Expr::gid_x()),
                Node::if_then(
                    Expr::and(
                        Expr::lt(Expr::var("idx"), Expr::u32(WORD_COUNT)),
                        Expr::eq(
                            Expr::load("text", Expr::var("idx")),
                            Expr::u32(u32::from_le_bytes(*b"vyre")),
                        ),
                    ),
                    vec![Node::let_bind(
                        "_old",
                        Expr::atomic_add("out_matches", Expr::u32(0), Expr::u32(1)),
                    )],
                ),
            ],
        );
        Ok(Box::new(prog))
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prog = crate::api::case::prepared_program(prepared)?;

        let size: usize = WORD_COUNT as usize * 4; // 256KB
        let mut text_bytes = vec![0u8; size];
        // Fill with deterministic non-matching pattern
        for (index, byte) in text_bytes.iter_mut().enumerate() {
            *byte = b'a' + (index % 26) as u8;
        }
        // Plant "vyre" at word-aligned offsets every 4096 bytes.
        // This ensures both the GPU (word-aligned scan) and the CPU
        // (memchr byte-scan) find the same number of matches.
        for offset in (0..size.saturating_sub(4)).step_by(4096) {
            text_bytes[offset..offset + 4].copy_from_slice(b"vyre");
        }

        let inputs = vec![text_bytes];
        let timed = ctx
            .dispatch_timed(prog, &inputs, &ctx.dispatch_config)
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
        let elapsed = timed.wall_ns;
        let dispatch_ns = timed.device_ns;
        let outputs = timed.outputs;

        // CPU baseline  -  byte-granularity scan via memchr
        let start_ref = std::time::Instant::now();
        let baseline_outputs = vec![crate::cases::cpu_baselines::dfa_vyre_match_count_bytes(
            &inputs[0],
        )];
        let elapsed_ref = start_ref.elapsed().as_nanos() as u64;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(elapsed),
                dispatch_ns,
                input_bytes: Some(inputs.iter().map(Vec::len).sum::<usize>() as u64),
                output_bytes: Some(outputs.iter().map(Vec::len).sum::<usize>() as u64),
                bytes_read: Some(WORD_COUNT as u64 * 4),
                bytes_written: Some(4),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(elapsed_ref),
                input_bytes: Some(inputs[0].len() as u64),
                output_bytes: Some(baseline_outputs.iter().map(Vec::len).sum::<usize>() as u64),
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(baseline_outputs),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

inventory::submit! {
    &DfaMatch as &'static dyn BenchCase
}
