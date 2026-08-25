//! `cargo_full run --bin xtask -- op-matrix`  -  generate and check the canonical op matrix.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process;

use vyre_harness::{classify_op_id, OpTier};

const DEFAULT_MATRIX_PATH: &str = "docs/optimization/OP_MATRIX.toml";
const MAX_OP_MATRIX_TEXT_BYTES: u64 = 4_194_304;

#[derive(Clone)]
struct OpRecord {
    family: String,
    tier: OpTier,
    owners: Vec<String>,
    ops: Vec<String>,
    registry_sources: Vec<String>,
    duplicate_ok: bool,
    reference: &'static str,
    foundation_ir: &'static str,
    cuda: &'static str,
    wgpu: &'static str,
    spirv: &'static str,
    release_blocking_notes: String,
    tests: Vec<String>,
    bench_targets: Vec<String>,
}

pub(crate) fn run(args: &[String]) {
    let mut check = false;
    let mut write = false;
    let mut path = DEFAULT_MATRIX_PATH.to_string();

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--check" => check = true,
            "--write" => {
                write = true;
                if let Some(next) = args.get(i + 1).filter(|value| !value.starts_with("--")) {
                    path = next.clone();
                    i += 1;
                }
            }
            other => {
                eprintln!(
                    "Fix: unknown op-matrix argument `{other}`. Use --check or --write [PATH]."
                );
                process::exit(1);
            }
        }
        i += 1;
    }

    if !check && !write {
        write = true;
    }

    let matrix = match build_matrix() {
        Ok(matrix) => matrix,
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    };

    if check {
        let current = match read_text_bounded(Path::new(&path)) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("Fix: read `{path}` before op-matrix check: {error}");
                process::exit(1);
            }
        };
        if normalize_newline(&current) != normalize_newline(&matrix) {
            eprintln!(
                "Fix: `{path}` is not the source-backed op matrix. Run `cargo_full run --bin xtask -- op-matrix --write`."
            );
            process::exit(1);
        }
    }

    if write {
        if let Some(parent) = Path::new(&path).parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                eprintln!(
                    "Fix: create `{}` before writing op matrix: {error}",
                    parent.display()
                );
                process::exit(1);
            }
        }
        if let Err(error) = fs::write(&path, matrix) {
            eprintln!("Fix: write `{path}`: {error}");
            process::exit(1);
        }
    }
}

fn normalize_newline(value: &str) -> String {
    value.replace("\r\n", "\n")
}

fn build_matrix() -> Result<String, String> {
    let mut records = manual_records();
    records.extend(registered_records()?);
    validate_records(&records)?;

    records.sort_by(|left, right| {
        (
            left.tier.matrix_value(),
            left.family.as_str(),
            left.ops.first().map(String::as_str),
        )
            .cmp(&(
                right.tier.matrix_value(),
                right.family.as_str(),
                right.ops.first().map(String::as_str),
            ))
    });

    Ok(render_matrix(&records))
}

fn manual_records() -> Vec<OpRecord> {
    vec![
        OpRecord {
            family: "integer_strength_reduction".to_string(),
            tier: OpTier::FoundationIr,
            owners: vec!["vyre-foundation/src/optimizer/passes/algebraic/strength_reduce".to_string()],
            ops: vec![
                "mul_power_of_two_to_shift".to_string(),
                "div_power_of_two_to_shift".to_string(),
                "mod_power_of_two_to_and".to_string(),
                "shift_add_decomposition".to_string(),
                "constant_division".to_string(),
            ],
            registry_sources: vec!["manual.foundation_ir".to_string()],
            duplicate_ok: false,
            reference: "not_applicable",
            foundation_ir: "supported",
            cuda: "not_applicable",
            wgpu: "not_applicable",
            spirv: "not_applicable",
            release_blocking_notes:
                "Backend rows are not applicable because the original IR should be rewritten before lowering."
                    .to_string(),
            tests: vec![
                "vyre-foundation/src/optimizer/passes/algebraic/strength_reduce/tests.rs"
                    .to_string(),
            ],
            bench_targets: vec!["integer_arithmetic_micro".to_string()],
        },
        OpRecord {
            family: "elementwise_add".to_string(),
            tier: OpTier::FoundationIr,
            owners: vec!["vyre-bench/src/cases/elementwise.rs".to_string()],
            ops: vec!["f32_add".to_string()],
            registry_sources: vec!["manual.bench".to_string()],
            duplicate_ok: false,
            reference: "supported",
            foundation_ir: "supported",
            cuda: "supported",
            wgpu: "supported",
            spirv: "experimental",
            release_blocking_notes:
                "CUDA is the canonical performance backend for this release; active-time benchmark target is in BENCH_TARGETS.toml."
                    .to_string(),
            tests: vec!["vyre-driver-cuda/tests/resident_dispatch_contracts.rs".to_string()],
            bench_targets: vec!["foundation.elementwise.add.1m".to_string()],
        },
    ]
}

fn registered_records() -> Result<Vec<OpRecord>, String> {
    let mut ids = BTreeMap::<String, BTreeSet<String>>::new();

    for entry in vyre_intrinsics::harness::all_entries() {
        push_registered(&mut ids, entry.id, "vyre-intrinsics::harness")?;
    }
    for entry in vyre_primitives::harness::all_entries() {
        push_registered(&mut ids, entry.id, "vyre-primitives::harness")?;
    }
    for entry in vyre_libs::harness::all_entries() {
        push_registered(&mut ids, entry.id, "vyre-harness")?;
    }
    for registration in inventory::iter::<vyre_driver::OpDefRegistration> {
        let def = (registration.op)();
        push_registered(&mut ids, def.id, "vyre-driver::registry")?;
    }

    ids.into_iter()
        .map(|(id, sources)| record_for_registered_id(&id, sources))
        .collect()
}

fn push_registered(
    ids: &mut BTreeMap<String, BTreeSet<String>>,
    id: &str,
    source: &str,
) -> Result<(), String> {
    let sources = ids.entry(id.to_string()).or_default();
    if !sources.insert(source.to_string()) {
        return Err(format!(
            "Fix: duplicate op id `{id}` registered more than once by `{source}`. \
             Keep one canonical registration in that registry."
        ));
    }
    Ok(())
}

fn record_for_registered_id(id: &str, sources: BTreeSet<String>) -> Result<OpRecord, String> {
    let tier = classify_op_id(id);
    if tier == OpTier::Unknown {
        return Err(format!(
            "Fix: op id `{id}` from `{sources:?}` has no canonical tier namespace."
        ));
    }
    if sources.len() > 1 && !allowed_duplicate_sources(id, &sources) {
        return Err(format!(
            "Fix: op id `{id}` is registered by `{sources:?}` without an allowed duplicate contract."
        ));
    }

    let mut record = OpRecord {
        family: id.to_string(),
        tier,
        owners: owner_paths(id, tier),
        ops: vec![id.to_string()],
        duplicate_ok: sources.len() > 1,
        registry_sources: sources.into_iter().collect(),
        reference: "supported",
        foundation_ir: "supported",
        cuda: "supported",
        wgpu: "supported",
        spirv: "experimental",
        release_blocking_notes: release_notes(id, tier),
        tests: test_paths(id, tier),
        bench_targets: Vec::new(),
    };

    if tier == OpTier::Runtime {
        record.reference = "not_applicable";
        record.cuda = "experimental";
        record.wgpu = "experimental";
        record.spirv = "experimental";
    }

    Ok(record)
}

fn allowed_duplicate_sources(id: &str, sources: &BTreeSet<String>) -> bool {
    sources.len() == 2
        && sources.contains("vyre-harness")
        && sources.contains("vyre-driver::registry")
        && id.starts_with("vyre-libs::math::atomic::")
}

fn owner_paths(id: &str, tier: OpTier) -> Vec<String> {
    match tier {
        OpTier::Intrinsic => vec!["vyre-intrinsics/src/hardware".to_string()],
        OpTier::Primitive => {
            let domain = id
                .strip_prefix("vyre-primitives::")
                .and_then(|rest| rest.split("::").next())
                .unwrap_or("unknown");
            vec![format!("vyre-primitives/src/{domain}")]
        }
        OpTier::Libs => {
            if id.starts_with("vyre-libs::catalog::") {
                vec!["vyre-libs/src/primitive_catalog.rs".to_string()]
            } else {
                let domain = id
                    .strip_prefix("vyre-libs::")
                    .and_then(|rest| rest.split("::").next())
                    .unwrap_or("unknown");
                let owner = match domain {
                    "optim" => "vyre-libs/src/nn/optim".to_string(),
                    "quant" => "vyre-libs/src/nn/quant".to_string(),
                    "substrate" => "vyre-libs/src/substrate_catalog.rs".to_string(),
                    _ => format!("vyre-libs/src/{domain}"),
                };
                vec![owner]
            }
        }
        OpTier::Runtime => {
            if id.starts_with("core.") {
                vec!["vyre-driver/src/registry/core_indirect.rs".to_string()]
            } else {
                vec!["vyre-driver/src/registry/io.rs".to_string()]
            }
        }
        OpTier::External => vec!["docs/optimization/README.md".to_string()],
        OpTier::FoundationIr | OpTier::Unknown => Vec::new(),
    }
}

fn test_paths(id: &str, tier: OpTier) -> Vec<String> {
    let mut tests = match tier {
        OpTier::Intrinsic => vec!["vyre-intrinsics/tests/hardware_conform.rs".to_string()],
        OpTier::Primitive => vec!["vyre-primitives/tests/integration.rs".to_string()],
        OpTier::Libs | OpTier::External => vec!["vyre-libs/tests/universal_harness.rs".to_string()],
        OpTier::Runtime => {
            if id.starts_with("core.") {
                vec!["vyre-driver/src/registry/core_indirect.rs".to_string()]
            } else {
                vec!["vyre-driver/src/registry/io.rs".to_string()]
            }
        }
        OpTier::FoundationIr | OpTier::Unknown => Vec::new(),
    };
    tests.push("conform/vyre-conform-enforce/tests/op_matrix_truth.rs".to_string());
    tests
}

fn release_notes(id: &str, tier: OpTier) -> String {
    match tier {
        OpTier::Intrinsic => {
            "Source-backed row generated from vyre-intrinsics::harness; every intrinsic id must stay in the hardware namespace and pass hardware_conform.".to_string()
        }
        OpTier::Primitive => {
            "Source-backed row generated from vyre-primitives::harness; primitive ids must stay in the Tier 2.5 namespace.".to_string()
        }
        OpTier::Libs if id.starts_with("vyre-libs::catalog::") => {
            "Source-backed Tier 3 wrapper row; wrapper id is distinct from the primitive id and is checked by op_matrix_truth.".to_string()
        }
        OpTier::Libs => {
            "Source-backed row generated from vyre-harness; Tier 3 ids must stay in the vyre-libs namespace.".to_string()
        }
        OpTier::Runtime => {
            "Source-backed row generated from vyre-driver::registry; backend lowering support is opt-in and must be promoted by changing this generated contract.".to_string()
        }
        OpTier::External => {
            "Source-backed row generated from the shared vyre-harness registry for an external consumer crate.".to_string()
        }
        OpTier::FoundationIr | OpTier::Unknown => String::new(),
    }
}

fn validate_records(records: &[OpRecord]) -> Result<(), String> {
    let mut families = BTreeSet::new();
    let mut ops = BTreeMap::<&str, &str>::new();
    for record in records {
        if !families.insert(record.family.as_str()) {
            return Err(format!(
                "Fix: duplicate OP_MATRIX family `{}`.",
                record.family
            ));
        }
        if record.owners.is_empty() {
            return Err(format!(
                "Fix: OP_MATRIX row `{}` has no owners.",
                record.family
            ));
        }
        if record.tests.is_empty() {
            return Err(format!(
                "Fix: OP_MATRIX row `{}` has no tests.",
                record.family
            ));
        }
        for op in &record.ops {
            if let Some(first_family) = ops.insert(op, record.family.as_str()) {
                return Err(format!(
                    "Fix: op `{op}` appears in both OP_MATRIX families `{first_family}` and `{}`.",
                    record.family
                ));
            }
            // ROADMAP S7: an op id's namespace classification must match
            // its row's declared tier. A Tier-2.5 record must not carry
            // `vyre-libs::` ops, and a Tier-3 record must not carry
            // `vyre-primitives::` ops. Mismatches were the root cause of
            // the original S7 finding (some primitives shipped under
            // Tier-3 ids, making op truth ambiguous to the matrix).
            let observed = classify_op_id(op);
            if observed != OpTier::Unknown && tier_id_mismatch(record.tier, observed) {
                return Err(format!(
                    "Fix: op `{op}` is namespaced as {observed:?} but lives in OP_MATRIX family \
                     `{}` declared as {:?}. Move the id to the matching namespace, change the \
                     row tier, or split the row.",
                    record.family, record.tier,
                ));
            }
        }
    }
    Ok(())
}

/// Two `OpTier` values mismatch when one is `Primitive` and the other
/// is `Libs` (or vice versa)  -  the classes that S7 specifically
/// guards. Other combinations are accepted (FoundationIr, Intrinsic,
/// Runtime, External rows can legitimately carry varied ops by design).
fn tier_id_mismatch(declared: OpTier, observed: OpTier) -> bool {
    matches!(
        (declared, observed),
        (OpTier::Primitive, OpTier::Libs) | (OpTier::Libs, OpTier::Primitive)
    )
}

fn render_matrix(records: &[OpRecord]) -> String {
    let mut out = String::new();
    out.push_str("# Canonical op/backend optimization and coverage matrix.\n");
    out.push_str("# Generated by `cargo_full run --bin xtask -- op-matrix --write` from inventory registries plus manual foundation rows.\n");
    out.push_str(
        "# Do not hand-edit generated rows; change the source registry or generator instead.\n\n",
    );
    out.push_str("schema = 1\n\n");
    out.push_str("backend_status_values = [\n");
    out.push_str("  \"supported\",\n  \"experimental\",\n  \"not_applicable\",\n  \"blocked_release\",\n]\n\n");
    out.push_str("tier_values = [\n");
    out.push_str("  \"foundation_ir\",\n  \"intrinsic\",\n  \"primitive\",\n  \"libs\",\n  \"runtime\",\n  \"external\",\n]\n\n");

    for record in records {
        out.push_str("[[op]]\n");
        push_string(&mut out, "family", &record.family);
        push_string(&mut out, "tier", record.tier.matrix_value());
        push_array(&mut out, "owners", &record.owners);
        push_array(&mut out, "ops", &record.ops);
        push_array(&mut out, "registry_sources", &record.registry_sources);
        if record.duplicate_ok {
            out.push_str("duplicate_ok = true\n");
        }
        push_string(&mut out, "reference", record.reference);
        push_string(&mut out, "foundation_ir", record.foundation_ir);
        push_string(&mut out, "cuda", record.cuda);
        push_string(&mut out, "wgpu", record.wgpu);
        push_string(&mut out, "spirv", record.spirv);
        push_string(
            &mut out,
            "release_blocking_notes",
            &record.release_blocking_notes,
        );
        push_array(&mut out, "tests", &record.tests);
        push_array(&mut out, "bench_targets", &record.bench_targets);
        out.push('\n');
    }
    out
}

fn push_string(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push_str(" = ");
    out.push_str(&format!("{value:?}"));
    out.push('\n');
}

fn push_array(out: &mut String, key: &str, values: &[String]) {
    out.push_str(key);
    out.push_str(" = [");
    if values.is_empty() {
        out.push_str("]\n");
        return;
    }
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            out.push_str(", ");
        }
        out.push_str(&format!("{value:?}"));
    }
    out.push_str("]\n");
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_OP_MATRIX_TEXT_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_OP_MATRIX_TEXT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_OP_MATRIX_TEXT_BYTES} byte op matrix read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}
