//! Pipeline orchestration: lex -> parse -> resolve -> typeck -> borrow -> lower.

use vyre_libs::parsing::rust::lex::lexer::core::Token;
use vyre_libs::parsing::rust::lex::lexer::plan::RustLexerPlan;
use vyre_libs::parsing::rust::parse::Module;

use crate::RustFrontendError;

pub mod borrow_stage;
pub mod lexer_dispatch;
pub mod lower_stage;
pub mod parse_stage;
pub mod resolve_stage;
pub mod typeck_stage;

/// Configuration for the Rust frontend pipeline.
#[derive(Debug, Clone)]
pub struct RustPipelineConfig {
    /// Whether to dispatch the Rust lexer IR on a GPU backend. Off by default
    /// so library users can run the host parser without requiring a GPU.
    pub gpu_lex: bool,
    /// Whether to run borrow checking (E0596/E0597/E0499/E0502). Off by
    /// default; when enabled it runs the full nano-subset borrow check (CFG NLL
    /// dataflow), rustc-differential-verified by `rust_sema_borrow_oracle`.
    pub borrow_check: bool,
    /// Whether to lower to Vyre IR. Off by default. When enabled it lowers the
    /// nano-subset entry function (the last function) to an executable Vyre
    /// `Program`; unsupported constructs fail loudly rather than miscompiling.
    pub lower: bool,
    /// When lowering is enabled, compile the scalar entry function as a
    /// data-parallel map over this many lanes. `None` keeps scalar ABI
    /// compatibility (`pN[0] -> out[0]`); `Some(n)` lowers to `pN[gid.x] ->
    /// out[gid.x]` with an out-of-range guard for rounded workgroups.
    pub lower_lane_count: Option<u32>,
}

impl Default for RustPipelineConfig {
    fn default() -> Self {
        // The default keeps host parser users off the GPU dispatch path.
        // GPU lexing, borrow checking, and lowering are opt-in wired stages:
        // requested work either runs or fails loudly with stage context.
        Self {
            gpu_lex: false,
            borrow_check: false,
            lower: false,
            lower_lane_count: None,
        }
    }
}

/// Pipeline state holder. Construct once, compile many files.
pub struct RustPipeline {
    config: RustPipelineConfig,
    lex_plan: RustLexerPlan,
}

impl RustPipeline {
    /// Create a new pipeline.
    pub fn new(config: RustPipelineConfig) -> Self {
        Self {
            config,
            lex_plan: RustLexerPlan::new(),
        }
    }

    /// Run the pipeline on a single source buffer.
    ///
    /// CPU lex, parse, name resolution, and type checking always run. Borrow
    /// checking and lowering are gated on the config. Borrow checking runs the
    /// full nano-subset rules (E0596/E0597/E0499/E0502 via CFG NLL dataflow),
    /// and lowering produces an executable Vyre `Program` (unsupported
    /// constructs fail loudly), so a caller never receives a success that
    /// skipped or miscompiled a requested stage.
    pub fn compile_unit(&self, source: &[u8]) -> Result<CompilationUnit, RustFrontendError> {
        let tokens = self::lexer_dispatch::lex(source, &self.config, &self.lex_plan)?;
        self.compile_unit_from_tokens(source, &tokens)
    }

    /// Run the pipeline on many source buffers, sharing the GPU lexer dispatch
    /// when `gpu_lex` is enabled.
    ///
    /// The returned vector keeps source order. Each element is independent, so
    /// a syntax, type, borrow, or lex error in one source does not discard
    /// successful compilation results for neighboring sources.
    pub fn compile_units(&self, sources: &[&[u8]]) -> Result<CompilationBatch, RustFrontendError> {
        let token_results = self::lexer_dispatch::lex_batch(sources, &self.config, &self.lex_plan)?;
        let mut units = Vec::with_capacity(sources.len());
        for (source, tokens) in sources.iter().zip(token_results) {
            units.push(tokens.and_then(|tokens| self.compile_unit_from_tokens(*source, &tokens)));
        }
        Ok(CompilationBatch {
            units,
            gpu_lex: self.config.gpu_lex,
        })
    }

    fn compile_unit_from_tokens(
        &self,
        source: &[u8],
        tokens: &[Token],
    ) -> Result<CompilationUnit, RustFrontendError> {
        let module: Module = self::parse_stage::parse(source, &tokens)?;
        let resolution = self::resolve_stage::resolve(&module, source)?;
        self::typeck_stage::typeck(&module, source, &resolution)?;
        if self.config.borrow_check {
            self::borrow_stage::borrow_check(&module, &resolution)?;
        }
        let program = if self.config.lower {
            Some(self::lower_stage::lower(
                &module,
                &resolution,
                self.config.lower_lane_count,
            )?)
        } else {
            None
        };

        Ok(CompilationUnit {
            token_count: tokens.len(),
            module,
            program,
        })
    }
}

/// Result of compiling one translation unit.
#[derive(Debug, Clone)]
pub struct CompilationUnit {
    /// Number of tokens lexed.
    pub token_count: usize,
    /// Parsed module.
    pub module: Module,
    /// Lowered Vyre program (if lowering was enabled).
    pub program: Option<vyre::ir::Program>,
}

/// Result of compiling a batch of translation units.
#[derive(Debug, Clone)]
pub struct CompilationBatch {
    /// Per-source compilation results in the same order as the input slice.
    pub units: Vec<Result<CompilationUnit, RustFrontendError>>,
    /// Whether the batch used the GPU lexer path.
    pub gpu_lex: bool,
}
