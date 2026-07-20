use std::hint::black_box;
use std::time::Instant;

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::suite::SuiteKind;
use vyre_foundation::ir::Program;
use vyre_libs::parsing::c::lex::lexer::c11_lexer_regular_sparse_u8_haystack_with_flags;

const SCHEMA_VERSION: u32 = 1;
const SMALL_STATE_COUNT: u32 = 6;
const BYTE_CLASS_COUNT: u32 = 7;
const SMALL_STATE_TRANSITION_BYTES: u64 =
    (SMALL_STATE_COUNT as u64) * (BYTE_CLASS_COUNT as u64) * 2 + 256;
const SMALL_STATE_BRANCH_PROXY_PER_BYTE: u32 = 1;
const SUITES: &[SuiteKind] = &[
    SuiteKind::Smoke,
    SuiteKind::Release,
    SuiteKind::Deep,
    SuiteKind::Honest,
];

const C_TRANSITION_CORPUS: &[u8] = br#"
#define MAX_COUNT 17
static inline int add_one(int value) {
    const char *name = "vyre\nlexer";
    return value + MAX_COUNT;
}
"#;

const CLASS_SPACE: u8 = 0;
const CLASS_IDENT: u8 = 1;
const CLASS_DIGIT: u8 = 2;
const CLASS_QUOTE: u8 = 3;
const CLASS_OPERATOR: u8 = 4;
const CLASS_PUNCT: u8 = 5;
const CLASS_OTHER: u8 = 6;
const CLASS_END: u8 = 7;

const TOKEN_IDENT: u32 = 1;
const TOKEN_NUMBER: u32 = 2;
const TOKEN_STRING: u32 = 3;
const TOKEN_OPERATOR: u32 = 4;
const TOKEN_PUNCT: u32 = 5;
const TOKEN_OTHER: u32 = 6;

pub struct LexerSmallStateTransition;

struct LexerTransitionPrepared {
    source: Vec<u8>,
    baseline: LexerTransitionBaseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LexerTransitionBaseline {
    pub schema_version: u32,
    pub source_bytes: u32,
    pub token_count: u32,
    pub small_state_state_count: u32,
    pub small_state_equivalence_classes: u32,
    pub small_state_table_bytes: u64,
    pub small_state_branch_proxy_per_byte: u32,
    pub sparse_gpu_static_storage_bytes: u64,
    pub sparse_gpu_table_proxy_bytes: u64,
    pub sparse_gpu_state_proxy: u32,
    pub sparse_gpu_branch_proxy: u64,
    pub sparse_gpu_buffer_count: u32,
    pub sparse_gpu_workgroup_lanes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LexerToken {
    kind: u32,
    start: u32,
    len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmallState {
    Ground,
    Ident,
    Number,
    String,
    StringEscape,
    Operator,
}

impl BenchCase for LexerSmallStateTransition {
    fn id(&self) -> BenchId {
        BenchId("parser.c_lexer.small_state_transition.4k".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "C Lexer Small-State Transition Baseline".to_string(),
            description: "Compact byte-class transition scanner compared against the sparse C lexer reference surface and current sparse GPU lexer IR metrics".to_string(),
            tags: vec![
                "parser".to_string(),
                "lexer".to_string(),
                "c".to_string(),
                "small-state".to_string(),
                "shuffle".to_string(),
                "sparse".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Micro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: false,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: Some(C_TRANSITION_CORPUS.len() as u64),
            feature_set: vec![
                "c-lexer".to_string(),
                "small-state-transition".to_string(),
                "sparse-gpu-lexer-ir".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        None
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<LexerTransitionPrepared>()
            .map(|prepared| {
                (
                    prepared.source.len() as u64,
                    u64::from(prepared.baseline.token_count) * 12 + 4,
                )
            })
            .unwrap_or((0, 0))
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let source = C_TRANSITION_CORPUS.to_vec();
        let sparse_program = sparse_c_lexer_program(source.len())?;
        let sparse_tokens = sparse_reference_tokens(&source);
        let baseline = lexer_transition_baseline(&source, &sparse_tokens, &sparse_program)?;
        validate_lexer_transition_baseline(&baseline)?;
        Ok(Box::new(LexerTransitionPrepared { source, baseline }))
    }

    fn run(
        &self,
        _ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<LexerTransitionPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "prepared lexer transition payload had the wrong type".to_string(),
                )
            })?;

        let baseline_start = Instant::now();
        let baseline_tokens = sparse_reference_tokens(&prepared.source);
        black_box(baseline_tokens.len());
        let baseline_wall_ns = baseline_start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

        let started = Instant::now();
        let transition_tokens = small_state_transition_tokens(&prepared.source);
        black_box(transition_tokens.len());
        let wall_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

        let outputs = vec![encode_tokens(&transition_tokens)];
        let baseline_outputs = vec![encode_tokens(&baseline_tokens)];
        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let token_parity = u64::from(transition_tokens == baseline_tokens);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall_ns.max(1)),
                input_bytes: Some(prepared.source.len() as u64),
                output_bytes: Some(output_bytes),
                bytes_read: Some(prepared.source.len() as u64),
                bytes_written: Some(output_bytes),
                custom: lexer_transition_metric_points(
                    prepared.baseline,
                    wall_ns.max(1),
                    baseline_wall_ns.max(1),
                    token_parity,
                ),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(baseline_wall_ns.max(1)),
                input_bytes: Some(prepared.source.len() as u64),
                output_bytes: Some(baseline_outputs.iter().map(Vec::len).sum::<usize>() as u64),
                custom: vec![metric(
                    "lexer_transition_sparse_reference_tokens",
                    baseline_tokens.len() as u64,
                )],
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

fn sparse_c_lexer_program(source_len: usize) -> Result<Program, BenchError> {
    let source_len = u32::try_from(source_len).map_err(|_| {
        BenchError::EnvironmentInvalid(
            "lexer transition source exceeds u32 sparse lexer length. Fix: split the corpus into smaller benchmark shards."
                .to_string(),
        )
    })?;
    Ok(c11_lexer_regular_sparse_u8_haystack_with_flags(
        "haystack",
        "tok_types",
        "tok_starts",
        "tok_lens",
        "tok_counts",
        source_len,
    ))
}

fn lexer_transition_baseline(
    source: &[u8],
    sparse_tokens: &[LexerToken],
    sparse_program: &Program,
) -> Result<LexerTransitionBaseline, BenchError> {
    let stats = sparse_program.stats();
    let source_bytes = u32::try_from(source.len()).map_err(|_| {
        BenchError::EnvironmentInvalid(
            "lexer transition source byte length exceeds u32. Fix: split the corpus."
                .to_string(),
        )
    })?;
    let token_count = u32::try_from(sparse_tokens.len()).map_err(|_| {
        BenchError::EnvironmentInvalid(
            "lexer transition token count exceeds u32. Fix: split the corpus.".to_string(),
        )
    })?;
    let workgroup = sparse_program.workgroup_size();
    let sparse_gpu_buffer_count = u32::try_from(sparse_program.buffers().len()).map_err(|_| {
        BenchError::EnvironmentInvalid(
            "sparse lexer program buffer count exceeds u32. Fix: reduce generated buffers."
                .to_string(),
        )
    })?;
    let sparse_gpu_table_proxy_bytes = stats
        .static_storage_bytes
        .saturating_add((stats.node_count as u64).saturating_mul(8))
        .saturating_add(stats.control_flow_count.saturating_mul(4))
        .saturating_add(u64::from(sparse_gpu_buffer_count).saturating_mul(32));

    Ok(LexerTransitionBaseline {
        schema_version: SCHEMA_VERSION,
        source_bytes,
        token_count,
        small_state_state_count: SMALL_STATE_COUNT,
        small_state_equivalence_classes: BYTE_CLASS_COUNT,
        small_state_table_bytes: SMALL_STATE_TRANSITION_BYTES,
        small_state_branch_proxy_per_byte: SMALL_STATE_BRANCH_PROXY_PER_BYTE,
        sparse_gpu_static_storage_bytes: stats.static_storage_bytes,
        sparse_gpu_table_proxy_bytes,
        sparse_gpu_state_proxy: stats.register_pressure_estimate,
        sparse_gpu_branch_proxy: stats.control_flow_count,
        sparse_gpu_buffer_count,
        sparse_gpu_workgroup_lanes: workgroup[0],
    })
}

fn validate_lexer_transition_baseline(
    baseline: &LexerTransitionBaseline,
) -> Result<(), BenchError> {
    if baseline.schema_version != SCHEMA_VERSION {
        return Err(BenchError::EnvironmentInvalid(format!(
            "lexer transition baseline schema version {} did not match expected {}. Fix: regenerate the transition baseline with the current schema.",
            baseline.schema_version, SCHEMA_VERSION
        )));
    }
    if baseline.source_bytes == 0 || baseline.token_count == 0 {
        return Err(BenchError::EnvironmentInvalid(format!(
            "lexer transition baseline recorded source_bytes={} token_count={}. Fix: use a non-empty C lexer corpus with visible tokens.",
            baseline.source_bytes, baseline.token_count
        )));
    }
    if baseline.small_state_state_count == 0
        || baseline.small_state_equivalence_classes == 0
        || baseline.small_state_table_bytes == 0
    {
        return Err(BenchError::EnvironmentInvalid(
            "lexer transition baseline omitted small-state table bytes, state count, or byte classes. Fix: rebuild the compact transition table metadata."
                .to_string(),
        ));
    }
    if baseline.sparse_gpu_table_proxy_bytes == 0
        || baseline.sparse_gpu_buffer_count == 0
        || baseline.sparse_gpu_workgroup_lanes == 0
    {
        return Err(BenchError::EnvironmentInvalid(format!(
            "lexer transition baseline recorded sparse table proxy={} buffers={} workgroup_lanes={}. Fix: build the current sparse C lexer IR before reporting baseline metrics.",
            baseline.sparse_gpu_table_proxy_bytes,
            baseline.sparse_gpu_buffer_count,
            baseline.sparse_gpu_workgroup_lanes
        )));
    }
    Ok(())
}

fn sparse_reference_tokens(source: &[u8]) -> Vec<LexerToken> {
    let mut tokens = Vec::new();
    let mut index = 0_usize;
    while index < source.len() {
        match byte_class(source[index]) {
            CLASS_SPACE => index += 1,
            CLASS_IDENT => {
                let start = index;
                index += 1;
                while index < source.len() && is_ident_continue_class(byte_class(source[index])) {
                    index += 1;
                }
                push_token(&mut tokens, TOKEN_IDENT, start, index);
            }
            CLASS_DIGIT => {
                let start = index;
                index += 1;
                while index < source.len() && byte_class(source[index]) == CLASS_DIGIT {
                    index += 1;
                }
                push_token(&mut tokens, TOKEN_NUMBER, start, index);
            }
            CLASS_QUOTE => {
                let start = index;
                index = scan_string(source, index);
                push_token(&mut tokens, TOKEN_STRING, start, index);
            }
            CLASS_OPERATOR => {
                let start = index;
                index += 1;
                while index < source.len() && byte_class(source[index]) == CLASS_OPERATOR {
                    index += 1;
                }
                push_token(&mut tokens, TOKEN_OPERATOR, start, index);
            }
            CLASS_PUNCT => {
                let start = index;
                index += 1;
                push_token(&mut tokens, TOKEN_PUNCT, start, index);
            }
            _ => {
                let start = index;
                index += 1;
                push_token(&mut tokens, TOKEN_OTHER, start, index);
            }
        }
    }
    tokens
}

fn small_state_transition_tokens(source: &[u8]) -> Vec<LexerToken> {
    let table = byte_class_table();
    let mut tokens = Vec::new();
    let mut state = SmallState::Ground;
    let mut token_start = 0_usize;
    let mut index = 0_usize;

    while index <= source.len() {
        let class = if index == source.len() {
            CLASS_END
        } else {
            table[usize::from(source[index])]
        };
        match state {
            SmallState::Ground => match class {
                CLASS_END => break,
                CLASS_SPACE => index += 1,
                CLASS_IDENT => {
                    token_start = index;
                    state = SmallState::Ident;
                    index += 1;
                }
                CLASS_DIGIT => {
                    token_start = index;
                    state = SmallState::Number;
                    index += 1;
                }
                CLASS_QUOTE => {
                    token_start = index;
                    state = SmallState::String;
                    index += 1;
                }
                CLASS_OPERATOR => {
                    token_start = index;
                    state = SmallState::Operator;
                    index += 1;
                }
                CLASS_PUNCT => {
                    push_token(&mut tokens, TOKEN_PUNCT, index, index + 1);
                    index += 1;
                }
                _ => {
                    push_token(&mut tokens, TOKEN_OTHER, index, index + 1);
                    index += 1;
                }
            },
            SmallState::Ident => {
                if is_ident_continue_class(class) {
                    index += 1;
                } else {
                    push_token(&mut tokens, TOKEN_IDENT, token_start, index);
                    state = SmallState::Ground;
                }
            }
            SmallState::Number => {
                if class == CLASS_DIGIT {
                    index += 1;
                } else {
                    push_token(&mut tokens, TOKEN_NUMBER, token_start, index);
                    state = SmallState::Ground;
                }
            }
            SmallState::String => match class {
                CLASS_END => {
                    push_token(&mut tokens, TOKEN_STRING, token_start, index);
                    break;
                }
                _ if source[index] == b'\\' => {
                    state = SmallState::StringEscape;
                    index += 1;
                }
                CLASS_QUOTE => {
                    index += 1;
                    push_token(&mut tokens, TOKEN_STRING, token_start, index);
                    state = SmallState::Ground;
                }
                _ => index += 1,
            },
            SmallState::StringEscape => {
                if class == CLASS_END {
                    push_token(&mut tokens, TOKEN_STRING, token_start, index);
                    break;
                }
                state = SmallState::String;
                index += 1;
            }
            SmallState::Operator => {
                if class == CLASS_OPERATOR {
                    index += 1;
                } else {
                    push_token(&mut tokens, TOKEN_OPERATOR, token_start, index);
                    state = SmallState::Ground;
                }
            }
        }
    }

    tokens
}

fn scan_string(source: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    while index < source.len() {
        if source[index] == b'\\' {
            index = index.saturating_add(2);
        } else if source[index] == b'"' {
            return index + 1;
        } else {
            index += 1;
        }
    }
    source.len()
}

fn push_token(tokens: &mut Vec<LexerToken>, kind: u32, start: usize, end: usize) {
    let len = u32::try_from(end.saturating_sub(start)).unwrap_or(u32::MAX);
    let start = u32::try_from(start).unwrap_or(u32::MAX);
    tokens.push(LexerToken { kind, start, len });
}

fn byte_class_table() -> [u8; 256] {
    let mut table = [CLASS_OTHER; 256];
    let mut byte = 0_usize;
    while byte < table.len() {
        table[byte] = byte_class(byte as u8);
        byte += 1;
    }
    table
}

fn byte_class(byte: u8) -> u8 {
    match byte {
        b' ' | b'\n' | b'\r' | b'\t' | 0 => CLASS_SPACE,
        b'a'..=b'z' | b'A'..=b'Z' | b'_' => CLASS_IDENT,
        b'0'..=b'9' => CLASS_DIGIT,
        b'"' => CLASS_QUOTE,
        b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'!' | b'<' | b'>' | b'&' | b'|' | b'^' => {
            CLASS_OPERATOR
        }
        b'(' | b')' | b'{' | b'}' | b'[' | b']' | b';' | b',' | b'.' | b'#' | b':' => CLASS_PUNCT,
        _ => CLASS_OTHER,
    }
}

fn is_ident_continue_class(class: u8) -> bool {
    matches!(class, CLASS_IDENT | CLASS_DIGIT)
}

fn encode_tokens(tokens: &[LexerToken]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(4 + tokens.len() * 12);
    encoded.extend_from_slice(
        &u32::try_from(tokens.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for token in tokens {
        encoded.extend_from_slice(&token.kind.to_le_bytes());
        encoded.extend_from_slice(&token.start.to_le_bytes());
        encoded.extend_from_slice(&token.len.to_le_bytes());
    }
    encoded
}

fn lexer_transition_metric_points(
    baseline: LexerTransitionBaseline,
    wall_ns: u64,
    baseline_wall_ns: u64,
    token_parity: u64,
) -> Vec<MetricPoint> {
    vec![
        metric(
            "lexer_transition_schema_version",
            u64::from(baseline.schema_version),
        ),
        metric("lexer_transition_source_bytes", u64::from(baseline.source_bytes)),
        metric("lexer_transition_token_count", u64::from(baseline.token_count)),
        metric(
            "lexer_transition_token_parity",
            token_parity,
        ),
        metric(
            "lexer_transition_small_state_count",
            u64::from(baseline.small_state_state_count),
        ),
        metric(
            "lexer_transition_small_state_equivalence_classes",
            u64::from(baseline.small_state_equivalence_classes),
        ),
        metric(
            "lexer_transition_small_state_table_bytes",
            baseline.small_state_table_bytes,
        ),
        metric(
            "lexer_transition_small_state_branch_proxy_per_byte",
            u64::from(baseline.small_state_branch_proxy_per_byte),
        ),
        metric(
            "lexer_transition_sparse_gpu_static_storage_bytes",
            baseline.sparse_gpu_static_storage_bytes,
        ),
        metric(
            "lexer_transition_sparse_gpu_table_proxy_bytes",
            baseline.sparse_gpu_table_proxy_bytes,
        ),
        metric(
            "lexer_transition_sparse_gpu_state_proxy",
            u64::from(baseline.sparse_gpu_state_proxy),
        ),
        metric(
            "lexer_transition_sparse_gpu_branch_proxy",
            baseline.sparse_gpu_branch_proxy,
        ),
        metric(
            "lexer_transition_sparse_gpu_buffer_count",
            u64::from(baseline.sparse_gpu_buffer_count),
        ),
        metric(
            "lexer_transition_sparse_gpu_workgroup_lanes",
            u64::from(baseline.sparse_gpu_workgroup_lanes),
        ),
        metric(
            "lexer_transition_small_state_throughput_bytes_per_s",
            bytes_per_second(u64::from(baseline.source_bytes), wall_ns),
        ),
        metric(
            "lexer_transition_sparse_reference_throughput_bytes_per_s",
            bytes_per_second(u64::from(baseline.source_bytes), baseline_wall_ns),
        ),
    ]
}

fn bytes_per_second(bytes: u64, wall_ns: u64) -> u64 {
    if wall_ns == 0 {
        return 0;
    }
    ((u128::from(bytes) * 1_000_000_000_u128) / u128::from(wall_ns))
        .min(u128::from(u64::MAX)) as u64
}

fn metric(name: &str, value: u64) -> MetricPoint {
    MetricPoint {
        name: name.to_string(),
        value,
    }
}

inventory::submit! {
    &LexerSmallStateTransition as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_state_transition_matches_sparse_reference_on_corpus() {
        let sparse = sparse_reference_tokens(C_TRANSITION_CORPUS);
        let compact = small_state_transition_tokens(C_TRANSITION_CORPUS);

        assert_eq!(compact, sparse);
        assert!(!compact.is_empty());
    }

    #[test]
    fn lexer_transition_metrics_report_required_baseline_axes() {
        let source = C_TRANSITION_CORPUS.to_vec();
        let sparse_program = sparse_c_lexer_program(source.len()).unwrap();
        let sparse_tokens = sparse_reference_tokens(&source);
        let baseline = lexer_transition_baseline(&source, &sparse_tokens, &sparse_program).unwrap();
        validate_lexer_transition_baseline(&baseline).unwrap();
        let metrics = lexer_transition_metric_points(baseline, 10, 20, 1);

        assert!(metrics
            .iter()
            .any(|metric| metric.name == "lexer_transition_small_state_table_bytes"));
        assert!(metrics
            .iter()
            .any(|metric| metric.name == "lexer_transition_small_state_count"));
        assert!(metrics
            .iter()
            .any(|metric| metric.name == "lexer_transition_sparse_gpu_branch_proxy"));
        assert!(metrics.iter().any(|metric| {
            metric.name == "lexer_transition_small_state_throughput_bytes_per_s"
                && metric.value > 0
        }));
        assert!(metrics.iter().any(|metric| {
            metric.name == "lexer_transition_token_parity" && metric.value == 1
        }));
    }

    #[test]
    fn lexer_transition_validation_rejects_missing_table_bytes() {
        let source = C_TRANSITION_CORPUS.to_vec();
        let sparse_program = sparse_c_lexer_program(source.len()).unwrap();
        let sparse_tokens = sparse_reference_tokens(&source);
        let mut baseline =
            lexer_transition_baseline(&source, &sparse_tokens, &sparse_program).unwrap();
        baseline.small_state_table_bytes = 0;

        let error = validate_lexer_transition_baseline(&baseline).unwrap_err();
        assert!(error.to_string().contains("small-state table bytes"));
    }
}
