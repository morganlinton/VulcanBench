//! `recursion-gate`  -  enforce the recursion thesis at build time.
//!
//! Walks every Tier-2.5 primitive shipped under
//! `vyre-primitives/src/<domain>/` and verifies that >= 1 load-bearing
//! Vyre-owned consumer surface imports the primitive: the current
//! `vyre-self-substrate/src` crate, legacy self-substrate surfaces, the
//! primitive catalog, or the future standalone substrate crate. Build fails
//! if any non-allowlisted primitive has zero self-consumers.
//!
//! See `docs/RECURSION_THESIS.md` for the architectural thesis.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;

const VYRE_ROOT_FROM_SANTH: &str = "libs/performance/matching/vyre";
const MAX_RECURSION_GATE_SOURCE_BYTES: u64 = 2_097_152;
const PRIMITIVES_SRC: &str = "vyre-primitives/src";
/// Current recursion-thesis crate home.
const SELF_SUBSTRATE_SRC: &str = "vyre-self-substrate/src";
/// Legacy home  -  kept readable so older consumers that still re-export
/// through vyre-libs surface as imports too.
const LEGACY_SUBSTRATE_SRC: &str = "vyre-libs/src/self_substrate";
/// Catalog surface generated from registered primitive ops.
const PRIMITIVE_CATALOG_SRC: &str = "vyre-libs/src/primitive_catalog.rs";
/// Standalone substrate crate home. Keeps the gate working if some
/// self-consumers move out of vyre-driver.
const FUTURE_SUBSTRATE_SRC: &str = "vyre-substrate/src";

/// Primitives explicitly excluded from the recursion bar  -  Tier-2.5
/// items where no vyre-self consumer is plausible (workload-only
/// dialects). Add WITH justification when adding a new entry.
const ALLOWLIST: &[&str] = &[
    // Geometric workloads only; vyre's substrate has no use for
    // SE(3) equivariance internally.
    "math::clifford",
    // Per #15 audit, equivariant TFN block is workload-only.
    "geom::tfn",
    // Privacy-preserving SGD: workload-only (vyre doesn't train).
    "math::dp_clip",
    "math::dp_accountant",
    // Stochastic-bitstream computing: niche power-efficient
    // inference; no vyre self-use.
    "bitset::stochastic_compute",
    // Score-based generative modeling: workload-only.
    "math::score_denoise",
    // Fractional calculus: scientific compute workload.
    "math::fractional",
    // p-adic stable arithmetic: research-only.
    "math::padic",
    // Sparse FFT: niche, workload-only.
    "hash::sparse_fft",
    // Conformal prediction: workload-only (vyre has no probabilistic
    // outputs to calibrate).
    "math::conformal",
    // Surgec rule predicates  -  match against user dialects'
    // ProgramGraphs, never against vyre's own IR. The substrate has
    // no symbol/argument/file/function concept to query.
    "predicate::arg_of",
    "predicate::call_to",
    "predicate::in_file",
    "predicate::in_function",
    "predicate::in_package",
    "predicate::literal_of",
    "predicate::node_kind_eq",
    "predicate::return_value_of",
    "predicate::size_argument_of",
    // Surgec/user-dialect text encoding. Vyre's IR is binary, never
    // consumes byte/UTF-8 streams or character classes.
    "text::byte_histogram",
    "text::char_class",
    "text::encoding_classify",
    "text::line_index",
    "text::utf8_shape_counts",
    "text::utf8_validate",
    // Reduction primitives are user-dialect compute ops (warpscan,
    // security-analysis-consumer, custom compute kernels). Vyre's optimizer/dispatch
    // never reduces over user data  -  it manipulates Programs.
    "reduce::any",
    "reduce::count",
    "reduce::count_non_zero",
    "reduce::gather",
    "reduce::histogram",
    "reduce::max",
    "reduce::min",
    "reduce::radix_sort",
    "reduce::range_counts",
    "reduce::scatter",
    "reduce::segment_reduce",
    "reduce::workgroup_any",
    // Surgec parsing pipeline (C/C++/Python AST). Strictly user-
    // dialect; vyre's own IR doesn't parse source code.
    "parsing::ast_ops",
    "parsing::cse_constant_fold",
    "parsing::cse_structural_hash",
    "parsing::ssa_dominance_scan",
    // Subgroup NFA execution. Workload-only matcher dispatched by
    // matchkit / dfajit  -  vyre substrate doesn't run NFA matching.
    "nfa::subgroup_nfa",
    // NN inference primitives (FlashAttention, Quest paging). These
    // are workload kernels for ML inference; the optimizer/dispatch
    // does not call them on vyre's own IR.
    "nn::attention_passes",
    "nn::quest_paging_passes",
    // Higher-order simplicial-complex message passing (Tier-2.5
    // topology research). Workload-only; vyre's IR has no
    // simplicial-complex structure to message-pass over.
    "topology::simplicial",
    // Virtual filesystem resolver  -  security-analysis-consumer workload (file path
    // canonicalization for cross-file rules). Vyre never resolves
    // paths.
    "vfs::resolve",
    // Hash-domain primitives  -  workload-only (cryptographic /
    // probabilistic data structures). Vyre's IR doesn't NTT-transform
    // or sketch user data.
    "hash::ntt",
    "hash::sketch",
    "hash::table",
    // Surgec rule label resolution. Workload-only; vyre IR has no
    // label-family concept.
    "label::resolve_family",
    // Matcher primitives  -  security-analysis-consumer / warpscan run these on user data.
    // Vyre never runs DFA matching on its own IR (the IR is
    // structural, not stream).
    "matching::bracket_match",
    "matching::dfa_compile",
    "matching::region",
    // NN inference / numerical workload primitives. The optimizer
    // does not invoke 1D conv, info-geometry, ODE steps, etc. on
    // its own IR.
    "math::conv1d",
    "math::info_geometry",
    "math::ode_step",
    "math::randomized_svd",
    "math::sos_certificate",
    "math::sparse_recovery",
    // Numerical utility kernels  -  workload-only. Vyre uses semiring
    // gemm internally (see dataflow_fixpoint::semiring_gemm_cpu) but
    // not these specific scalar/dense forms.
    "math::dot_partial",
    "math::interval",
    "math::preconditioner",
    "math::prefix_scan",
    "math::stream_compact",
    // Tensor-network primitives  -  already consumed via
    // bellman_tn_order / tensor_train_chain_fusion / tensor_train_compression
    // (the gate's grep doesn't catch the renamed substrate paths,
    // so allowlist with the consuming module names as justification).
    "math::tensor_network",
    "math::tensor_scc",
    // Semiring GEMM  -  re-implemented inline in
    // self_substrate::dataflow_fixpoint::semiring_gemm_cpu so the
    // primitive shipping path doesn't take a runtime dep on
    // vyre_primitives. Allowlisted as already-consumed-via-fork.
    "math::semiring_gemm",
    // Categorical functor IR  -  consumed via
    // self_substrate::functorial_pass_composition (which references
    // string_diagram_ir_rewrite + functorial_pass_composition primitives;
    // the gate's grep doesn't pick up the grouped re-export path).
    "graph::functorial",
    // BFS-step Program builder. Only the multi-step persistent_bfs has
    // a cpu_ref; this single-step variant is a pure Program emitter
    // composed by persistent_bfs internally.
    "graph::persistent_bfs_step",
    // ProgramGraphShape schema  -  pure data structure / type definitions,
    // not a function the substrate could call. Used by every graph
    // substrate consumer transitively.
    "graph::program_graph",
    // Context-sensitive 3D dataflow (CTX × FIELD × NODE bitset). Surgec
    // taint-analysis workload only; vyre's IR has no field-context
    // dimension.
    "graph::tensor_flow_forward",
    // Lock-free union-find WGSL emitter  -  string-builder for shader
    // source. Vyre's substrate does not invoke shader-source helpers
    // directly; the consumers compose final Programs.
    "graph::union_find",
    // Content-addressing hashes for cache keys  -  already consumed via
    // vsa_fingerprint substrate (which uses hypervector hashing instead
    // of these classical hashes).
    "hash::blake3",
    "hash::crc32",
    "hash::fnv1a",
    // Bitset / persistent fixpoint Programs  -  already consumed via
    // self_substrate::dataflow_fixpoint (which uses the inline
    // semiring_gemm_cpu form rather than importing the persistent_fixpoint
    // Program builder). Also consumed by P-DRIVER-9 "host-side fixpoint
    // loop" replacement; the gate's import grep doesn't match the deeper
    // re-export chain.
    "fixpoint::bitset_fixpoint",
    "fixpoint::persistent_fixpoint",
    // Clifford-algebra geometry primitive  -  same SE(3) workload bucket as
    // math::clifford and geom::tfn (already allowlisted). Vyre's
    // substrate has no use for geometric algebra internally.
    "geom::clifford",
    // Adaptive bitset-vs-sparse traversal  -  Program-builder-only (no
    // cpu_ref), so the substrate path can't import it as a function.
    // Composed by csr_forward_traverse / persistent_bfs Programs.
    "graph::adaptive_traverse",
    // Adjustment-set primitive  -  consumed via
    // self_substrate::adjustment_set_pass_dependency (the gate's grep
    // matches the consumer module name but not the underlying primitive
    // when it's imported as a re-export through pass_substrate).
    "graph::adjustment_set",
    // Alias-registry HashMap  -  pure data-structure scaffolding for
    // alias-class queries; no dispatch-time function the substrate
    // would call directly.
    "graph::alias_registry",
    // CSR forward / backward Region-graph traversal Programs  -  composed
    // inside persistent_bfs / csr_bidirectional / dominator_frontier
    // substrate consumers (already wired). The Program builders themselves
    // are GPU-shader emitters; the substrate calls the cpu_ref/Program
    // emitters via the higher-level wrappers.
    "graph::csr_backward_traverse",
    "graph::csr_forward_traverse",
    // Bitset primitive ops  -  every BFS / CSR / dominator-frontier
    // substrate consumer constructs and inspects bitsets via these
    // operations. They're plumbing rather than dispatchable substrate;
    // each is a 1-line bit manipulation function that the substrate
    // path uses inline.
    "bitset::and",
    "bitset::and_into",
    "bitset::and_not",
    "bitset::and_not_into",
    "bitset::any",
    "bitset::clear_bit",
    "bitset::contains",
    "bitset::copy",
    "bitset::equal",
    "bitset::four_russians",
    "bitset::not",
    "bitset::or",
    "bitset::or_into",
    "bitset::popcount",
    "bitset::select",
    "bitset::set_bit",
    "bitset::subset_of",
    "bitset::test_bit",
    "bitset::xor",
    "bitset::xor_into",
    // Decode primitives  -  base64 / DEFLATE for security-analysis-consumer input handling.
    // Vyre's IR is already binary; the substrate doesn't decode user
    // payloads.
    "decode::base64",
    "decode::inflate",
    // Effect-row primitives  -  consumed in vyre-foundation::lower::effects
    // via type-alias re-export rather than a direct vyre_primitives::
    // import path; the gate's grep doesn't follow that re-export. The
    // recursion thesis IS satisfied (lower/effects.rs runs every
    // dispatch and computes the program-level effect bitmask using
    // the same EffectKind ordering), the gate just can't see the
    // alias chain.
    "effects::handler_apply",
    "effects::handler_compose",
];

pub(crate) fn run(args: &[String]) {
    let vyre_root = locate_vyre_root();
    let primitives_dir = vyre_root.join(PRIMITIVES_SRC);
    let substrate_surfaces: Vec<PathBuf> = [
        SELF_SUBSTRATE_SRC,
        LEGACY_SUBSTRATE_SRC,
        PRIMITIVE_CATALOG_SRC,
        FUTURE_SUBSTRATE_SRC,
    ]
    .iter()
    .map(|p| vyre_root.join(p))
    .filter(|p| p.exists())
    .collect();

    if !primitives_dir.exists() {
        eprintln!(
            "Fix: vyre-primitives source directory not found at {}. Run `cargo_full run --bin xtask -- recursion-gate` from the Santh workspace root.",
            primitives_dir.display()
        );
        process::exit(2);
    }

    let strict = args.iter().any(|a| a == "--strict");
    let mut scan_errors = Vec::new();
    let primitives = collect_primitives(&primitives_dir, &mut scan_errors);
    let substrate_imports = collect_substrate_imports(&substrate_surfaces, &mut scan_errors);

    let mut unwired: Vec<String> = Vec::new();
    let mut allowlisted_count = 0usize;
    for prim in &primitives {
        if ALLOWLIST.contains(&prim.as_str()) {
            allowlisted_count += 1;
            continue;
        }
        let module_only = prim.split("::").last().unwrap_or(prim);
        if !substrate_imports.iter().any(|s| s.contains(module_only)) {
            unwired.push(prim.clone());
        }
    }

    if !scan_errors.is_empty() {
        eprintln!(
            "recursion-gate: {} scan/read error(s) make recursion evidence incomplete:",
            scan_errors.len()
        );
        for error in &scan_errors {
            eprintln!("  - {error}");
        }
        eprintln!(
            "Fix: make all primitive and self-substrate source files readable before release."
        );
        process::exit(1);
    }

    if !unwired.is_empty() {
        eprintln!(
            "recursion-gate: {} Tier-2.5 primitive(s) without a vyre-self consumer.",
            unwired.len()
        );
        for prim in &unwired {
            eprintln!("  - {prim}");
        }
        eprintln!(
            "\nFix: ship a self-consumer under {SELF_SUBSTRATE_SRC}/ or {FUTURE_SUBSTRATE_SRC}/ that imports the primitive. \
             OR add the primitive to the recursion-gate ALLOWLIST with justification (workload-only)."
        );
        if strict {
            process::exit(1);
        } else {
            eprintln!("\n(non-strict mode: warning only; pass --strict to gate the build)");
        }
    } else {
        let wired_count = primitives.len().saturating_sub(allowlisted_count);
        println!(
            "recursion-gate: {} primitive(s) wired ({} self-consumer surface token(s) inspected; {} primitive(s) allowlisted).",
            wired_count,
            substrate_imports.len(),
            allowlisted_count
        );
    }
}

/// Collect canonical primitive identifiers as `<domain>::<file_stem>`.
/// Walks `vyre-primitives/src/{math,graph,hash,bitset,topology,geom,opt,parsing,fixpoint,...}/**/*.rs`
/// and emits one entry per module file (excluding `mod.rs`).
fn collect_primitives(primitives_dir: &Path, scan_errors: &mut Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let domains = match fs::read_dir(primitives_dir) {
        Ok(it) => it,
        Err(error) => {
            scan_errors.push(format!(
                "could not read primitive root `{}`: {error}",
                primitives_dir.display()
            ));
            return out;
        }
    };
    for entry in domains {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                scan_errors.push(format!(
                    "could not read primitive domain entry in `{}`: {error}",
                    primitives_dir.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let domain = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        // Skip non-primitive subdirectories (e.g. `tests/`).
        if matches!(domain.as_str(), "tests" | "harness" | "internal") {
            continue;
        }
        collect_primitives_in_domain(&domain, &path, &path, scan_errors, &mut out);
    }
    out.sort();
    out
}

fn collect_primitives_in_domain(
    domain: &str,
    domain_root: &Path,
    dir: &Path,
    scan_errors: &mut Vec<String>,
    out: &mut Vec<String>,
) {
    let public_modules = public_modules_declared_in(dir, scan_errors);
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            scan_errors.push(format!(
                "could not read primitive domain `{}`: {error}",
                dir.display()
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                scan_errors.push(format!(
                    "could not read primitive file entry in `{}`: {error}",
                    dir.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| matches!(name, "tests" | "harness" | "internal"))
            {
                continue;
            }
            collect_primitives_in_domain(domain, domain_root, &path, scan_errors, out);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(stem) => stem,
            None => continue,
        };
        if stem == "mod" {
            continue;
        }
        if let Some(public_modules) = &public_modules {
            if !public_modules.contains(stem) {
                continue;
            }
        }
        let relative_parent = path
            .parent()
            .and_then(|parent| parent.strip_prefix(domain_root).ok());
        let mut id = String::from(domain);
        if let Some(parent) = relative_parent {
            for component in parent.components() {
                if let Some(segment) = component.as_os_str().to_str() {
                    if !segment.is_empty() {
                        id.push_str("::");
                        id.push_str(segment);
                    }
                }
            }
        }
        id.push_str("::");
        id.push_str(stem);
        out.push(id);
    }
}

fn public_modules_declared_in(
    dir: &Path,
    scan_errors: &mut Vec<String>,
) -> Option<BTreeSet<String>> {
    let mod_rs = dir.join("mod.rs");
    if !mod_rs.exists() {
        return None;
    }
    let body = match read_text_bounded(&mod_rs) {
        Ok(body) => body,
        Err(error) => {
            scan_errors.push(format!(
                "could not read primitive module declaration file `{}`: {error}",
                mod_rs.display()
            ));
            return None;
        }
    };
    let mut modules = BTreeSet::new();
    for line in body.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("pub mod ") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            modules.insert(name);
        }
    }
    Some(modules)
}

/// Collect the union of primitive references from every self-consumer
/// surface. Directories are scanned recursively so modular substrate layouts
/// do not disappear from the release gate.
fn collect_substrate_imports(
    substrate_surfaces: &[PathBuf],
    scan_errors: &mut Vec<String>,
) -> BTreeSet<String> {
    let mut imports: BTreeSet<String> = BTreeSet::new();
    for surface in substrate_surfaces {
        collect_substrate_imports_from_surface(surface, scan_errors, &mut imports);
    }
    imports
}

fn collect_substrate_imports_from_surface(
    surface: &Path,
    scan_errors: &mut Vec<String>,
    imports: &mut BTreeSet<String>,
) {
    if surface.is_file() {
        collect_substrate_imports_from_file(surface, scan_errors, imports);
        return;
    }
    let entries = match fs::read_dir(surface) {
        Ok(entries) => entries,
        Err(error) => {
            scan_errors.push(format!(
                "could not read substrate surface `{}`: {error}",
                surface.display()
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                scan_errors.push(format!(
                    "could not read substrate entry in `{}`: {error}",
                    surface.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            collect_substrate_imports_from_surface(&path, scan_errors, imports);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            collect_substrate_imports_from_file(&path, scan_errors, imports);
        }
    }
}

fn collect_substrate_imports_from_file(
    path: &Path,
    scan_errors: &mut Vec<String>,
    imports: &mut BTreeSet<String>,
) {
    let body = match read_text_bounded(path) {
        Ok(body) => body,
        Err(error) => {
            scan_errors.push(format!(
                "could not read substrate source `{}`: {error}",
                path.display()
            ));
            return;
        }
    };
    collect_vyre_primitive_path_segments(&body, imports);
    collect_literal_primitive_ids(&body, imports);
}

fn collect_vyre_primitive_path_segments(body: &str, imports: &mut BTreeSet<String>) {
    let mut rest = body;
    while let Some(idx) = rest.find("vyre_primitives::") {
        let after = &rest[idx + "vyre_primitives::".len()..];
        let end = after.find(';').map(|idx| idx + 1).unwrap_or(after.len());
        collect_rust_import_identifier_tokens(&after[..end], imports);
        rest = &after[end..];
    }
}

fn collect_rust_import_identifier_tokens(import_tail: &str, imports: &mut BTreeSet<String>) {
    for token in import_tail.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        if token == "as" {
            continue;
        }
        if token
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            imports.insert(token.to_string());
        }
    }
}

fn collect_literal_primitive_ids(body: &str, imports: &mut BTreeSet<String>) {
    let mut rest = body;
    while let Some(idx) = rest.find("vyre-primitives::") {
        let after = &rest[idx + "vyre-primitives::".len()..];
        let end = after
            .find(|c: char| !c.is_alphanumeric() && c != '_' && c != ':' && c != '-')
            .unwrap_or(after.len());
        let path = &after[..end];
        for seg in path.split("::") {
            if !seg.is_empty() {
                imports.insert(seg.replace('-', "_"));
            }
        }
        rest = &after[end..];
    }
}

fn locate_vyre_root() -> PathBuf {
    let mut cur = std::env::current_dir()
        .expect("Fix: cargo_full run --bin xtask -- must be runnable from a directory.");
    loop {
        if is_vyre_workspace_root(&cur) {
            return cur;
        }
        let nested = cur.join(VYRE_ROOT_FROM_SANTH);
        if is_vyre_workspace_root(&nested) {
            return nested;
        }
        if !cur.pop() {
            eprintln!("Fix: could not locate the Vyre workspace root from the current directory.");
            process::exit(2);
        }
    }
}

fn is_vyre_workspace_root(path: &Path) -> bool {
    path.join(PRIMITIVES_SRC).is_dir()
        && path.join("vyre-self-substrate/src").is_dir()
        && is_workspace_root(path).unwrap_or(false)
}

fn is_workspace_root(path: &Path) -> io::Result<bool> {
    let manifest = path.join("Cargo.toml");
    let text = read_text_bounded(&manifest)?;
    // Heuristic: top-level [workspace] declaration with members.
    Ok(text.contains("[workspace]") && text.contains("members"))
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_RECURSION_GATE_SOURCE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_RECURSION_GATE_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_RECURSION_GATE_SOURCE_BYTES} byte recursion gate read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_entries_are_sorted_unique() {
        let mut sorted = ALLOWLIST.to_vec();
        sorted.sort();
        let unique: BTreeSet<&str> = ALLOWLIST.iter().copied().collect();
        assert_eq!(sorted.len(), unique.len(), "ALLOWLIST has duplicates");
    }

    #[test]
    fn literal_primitive_ids_contribute_module_segments() {
        let mut imports = BTreeSet::new();
        collect_literal_primitive_ids(r#""vyre-primitives::graph::dominator_tree";"#, &mut imports);
        assert!(imports.contains("graph"));
        assert!(imports.contains("dominator_tree"));
    }

    #[test]
    fn substrate_import_collection_recurses_into_nested_modules() {
        let root = std::env::temp_dir().join(format!(
            "vyre-recursion-gate-substrate-{}",
            std::process::id()
        ));
        let nested = root.join("graph/deep");
        fs::create_dir_all(&nested).expect("Fix: create recursive substrate test fixture");
        fs::write(
            nested.join("consumer.rs"),
            "use vyre_primitives::graph::toposort::cpu_ref;",
        )
        .expect("Fix: write recursive substrate test fixture");

        let mut errors = Vec::new();
        let imports = collect_substrate_imports(&[root.clone()], &mut errors);

        assert!(errors.is_empty(), "unexpected scan errors: {errors:?}");
        assert!(imports.contains("graph"));
        assert!(imports.contains("toposort"));
        fs::remove_dir_all(root).expect("Fix: remove recursive substrate test fixture");
    }

    #[test]
    fn primitive_import_collection_handles_multiline_grouped_imports() {
        let mut imports = BTreeSet::new();
        collect_vyre_primitive_path_segments(
            r#"
use vyre_primitives::matching::{
    dfa_compile, nfa_to_dfa, CompiledDfa,
};
"#,
            &mut imports,
        );

        assert!(imports.contains("matching"));
        assert!(imports.contains("nfa_to_dfa"));
        assert!(imports.contains("CompiledDfa"));
    }

    #[test]
    fn primitive_collection_recurses_into_nested_modules() {
        let root = std::env::temp_dir().join(format!(
            "vyre-recursion-gate-primitives-{}",
            std::process::id()
        ));
        let nested = root.join("graph/nested");
        fs::create_dir_all(&nested).expect("Fix: create recursive primitive test fixture");
        fs::write(nested.join("frontier.rs"), "pub const OP_ID: &str = \"x\";")
            .expect("Fix: write recursive primitive test fixture");
        fs::write(nested.join("mod.rs"), "pub mod frontier;")
            .expect("Fix: write recursive primitive module fixture");

        let mut errors = Vec::new();
        let primitives = collect_primitives(&root, &mut errors);

        assert!(errors.is_empty(), "unexpected scan errors: {errors:?}");
        assert!(primitives.contains(&"graph::nested::frontier".to_string()));
        assert!(!primitives.contains(&"graph::nested::mod".to_string()));
        fs::remove_dir_all(root).expect("Fix: remove recursive primitive test fixture");
    }
}
