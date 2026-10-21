//! Clang AST oracle evidence for the GPU C frontend.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

const SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Serialize)]
struct ClangOracleReport {
    schema_version: u32,
    oracle: &'static str,
    corpus_root: String,
    vyre_report: String,
    clang_command: String,
    include_dirs: Vec<String>,
    macros: Vec<String>,
    file_count: usize,
    clang_successes: usize,
    clang_failures: usize,
    clang_function_definitions: u64,
    clang_call_expressions: u64,
    clang_files_with_function_definitions: usize,
    clang_files_with_call_expressions: usize,
    vyre_function_records: u64,
    vyre_call_records: u64,
    vyre_report_matched_files: usize,
    vyre_report_unmatched_files: usize,
    vyre_files_with_function_records: usize,
    vyre_files_with_call_records: usize,
    function_record_file_misses: usize,
    call_record_file_misses: usize,
    function_record_coverage_x1000: u64,
    call_record_coverage_x1000: u64,
    files: Vec<ClangOracleFile>,
    failures: Vec<ClangOracleFailure>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ClangOracleFile {
    path: String,
    clang_function_definitions: u64,
    clang_call_expressions: u64,
    vyre_function_records: u64,
    vyre_call_records: u64,
    function_record_coverage_x1000: u64,
    call_record_coverage_x1000: u64,
}

#[derive(Debug, Serialize)]
struct ClangOracleFailure {
    path: String,
    error: String,
}

#[derive(Debug)]
struct Config {
    corpus: PathBuf,
    vyre_report: PathBuf,
    output: PathBuf,
    clang: String,
    include_dirs: Vec<PathBuf>,
    macros: Vec<String>,
    limit: Option<usize>,
}

pub(crate) fn run(args: &[String]) {
    let config = match parse_args(args) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    match run_inner(&config) {
        Ok(report) => {
            if let Some(parent) = config.output.parent() {
                if let Err(error) = fs::create_dir_all(parent) {
                    eprintln!("Fix: failed to create `{}`: {error}", parent.display());
                    std::process::exit(1);
                }
            }
            let json = match serde_json::to_string_pretty(&report) {
                Ok(json) => json,
                Err(error) => {
                    eprintln!("Fix: failed to serialize clang oracle report: {error}");
                    std::process::exit(1);
                }
            };
            if let Err(error) = fs::write(&config.output, format!("{json}\n")) {
                eprintln!("Fix: failed to write `{}`: {error}", config.output.display());
                std::process::exit(1);
            }
            if !report.blockers.is_empty() {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run_inner(config: &Config) -> Result<ClangOracleReport, String> {
    let mut sources = Vec::new();
    collect_c_files(&config.corpus, &mut sources)?;
    sources.sort();
    if let Some(limit) = config.limit {
        sources.truncate(limit);
    }
    let vyre_report = read_json(&config.vyre_report)?;
    let vyre_function_records = json_u64(&vyre_report, "total_function_records");
    let vyre_call_records = json_u64(&vyre_report, "total_call_records");
    let vyre_file_records = read_vyre_file_records(&vyre_report);
    let mut files = Vec::new();
    let mut failures = Vec::new();
    let mut clang_function_definitions = 0u64;
    let mut clang_call_expressions = 0u64;
    let mut clang_files_with_function_definitions = 0usize;
    let mut clang_files_with_call_expressions = 0usize;
    let mut vyre_report_matched_files = 0usize;
    let mut vyre_report_unmatched_files = 0usize;
    let mut vyre_files_with_function_records = 0usize;
    let mut vyre_files_with_call_records = 0usize;
    let mut function_record_file_misses = 0usize;
    let mut call_record_file_misses = 0usize;

    for source in &sources {
        match run_clang_ast(config, source) {
            Ok(ast) => {
                let counts = count_clang_ast(&ast);
                let path = source.display().to_string();
                let vyre_file_record = lookup_vyre_file_record(&vyre_file_records, source);
                if vyre_file_record.is_some() {
                    vyre_report_matched_files = vyre_report_matched_files.saturating_add(1);
                } else {
                    vyre_report_unmatched_files = vyre_report_unmatched_files.saturating_add(1);
                }
                let (vyre_file_function_records, vyre_file_call_records) =
                    vyre_file_record.unwrap_or((0, 0));
                clang_function_definitions =
                    clang_function_definitions.saturating_add(counts.function_definitions);
                clang_call_expressions =
                    clang_call_expressions.saturating_add(counts.call_expressions);
                if counts.function_definitions > 0 {
                    clang_files_with_function_definitions =
                        clang_files_with_function_definitions.saturating_add(1);
                    if vyre_file_function_records > 0 {
                        vyre_files_with_function_records =
                            vyre_files_with_function_records.saturating_add(1);
                    } else {
                        function_record_file_misses =
                            function_record_file_misses.saturating_add(1);
                    }
                }
                if counts.call_expressions > 0 {
                    clang_files_with_call_expressions =
                        clang_files_with_call_expressions.saturating_add(1);
                    if vyre_file_call_records > 0 {
                        vyre_files_with_call_records =
                            vyre_files_with_call_records.saturating_add(1);
                    } else {
                        call_record_file_misses = call_record_file_misses.saturating_add(1);
                    }
                }
                files.push(ClangOracleFile {
                    path,
                    clang_function_definitions: counts.function_definitions,
                    clang_call_expressions: counts.call_expressions,
                    vyre_function_records: vyre_file_function_records,
                    vyre_call_records: vyre_file_call_records,
                    function_record_coverage_x1000: coverage_x1000(
                        vyre_file_function_records,
                        counts.function_definitions,
                    ),
                    call_record_coverage_x1000: coverage_x1000(
                        vyre_file_call_records,
                        counts.call_expressions,
                    ),
                });
            }
            Err(error) => failures.push(ClangOracleFailure {
                path: source.display().to_string(),
                error,
            }),
        }
    }

    let function_record_coverage_x1000 =
        coverage_x1000(vyre_function_records, clang_function_definitions);
    let call_record_coverage_x1000 = coverage_x1000(vyre_call_records, clang_call_expressions);
    let mut blockers = Vec::new();
    if sources.is_empty() {
        blockers.push("clang oracle corpus contains zero C files".to_string());
    }
    if !failures.is_empty() {
        blockers.push(format!("clang failed on {} file(s)", failures.len()));
    }
    if clang_function_definitions == 0 {
        blockers.push("clang oracle found zero function definitions".to_string());
    }
    if clang_call_expressions == 0 {
        blockers.push("clang oracle found zero call expressions".to_string());
    }
    if vyre_function_records == 0 {
        blockers.push("Vyre report has zero function records".to_string());
    }
    if vyre_call_records == 0 {
        blockers.push("Vyre report has zero call records".to_string());
    }
    if vyre_report_unmatched_files != 0 {
        blockers.push(format!(
            "Vyre report did not contain per-file rows for {vyre_report_unmatched_files} clang-successful source file(s)"
        ));
    }
    if function_record_coverage_x1000 < 500 {
        blockers.push(format!(
            "Vyre function-record coverage {function_record_coverage_x1000}/1000 is below release floor 500/1000"
        ));
    }
    if call_record_coverage_x1000 < 250 {
        blockers.push(format!(
            "Vyre call-record coverage {call_record_coverage_x1000}/1000 is below release floor 250/1000"
        ));
    }
    if function_record_file_misses != 0 {
        blockers.push(format!(
            "Vyre missed function records on {function_record_file_misses} clang-successful file(s) that contain function definitions"
        ));
    }
    if call_record_file_misses != 0 {
        blockers.push(format!(
            "Vyre missed call records on {call_record_file_misses} clang-successful file(s) that contain call expressions"
        ));
    }

    Ok(ClangOracleReport {
        schema_version: SCHEMA_VERSION,
        oracle: "clang-ast-dump-json",
        corpus_root: config.corpus.display().to_string(),
        vyre_report: config.vyre_report.display().to_string(),
        clang_command: config.clang.clone(),
        include_dirs: config
            .include_dirs
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        macros: config.macros.clone(),
        file_count: sources.len(),
        clang_successes: files.len(),
        clang_failures: failures.len(),
        clang_function_definitions,
        clang_call_expressions,
        clang_files_with_function_definitions,
        clang_files_with_call_expressions,
        vyre_function_records,
        vyre_call_records,
        vyre_report_matched_files,
        vyre_report_unmatched_files,
        vyre_files_with_function_records,
        vyre_files_with_call_records,
        function_record_file_misses,
        call_record_file_misses,
        function_record_coverage_x1000,
        call_record_coverage_x1000,
        files,
        failures,
        blockers,
    })
}

fn run_clang_ast(config: &Config, source: &Path) -> Result<serde_json::Value, String> {
    let mut command = Command::new(&config.clang);
    command
        .arg("-fsyntax-only")
        .arg("-Xclang")
        .arg("-ast-dump=json")
        .arg("-Wno-everything");
    for dir in &config.include_dirs {
        command.arg("-I").arg(dir);
    }
    for define in &config.macros {
        command.arg("-D").arg(define);
    }
    command.arg(source);
    let output = command
        .output()
        .map_err(|error| format!("failed to spawn clang `{}`: {error}", config.clang))?;
    if !output.status.success() {
        return Err(format!(
            "clang exited with status {}; stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("clang AST JSON parse failed: {error}"))
}

#[derive(Default)]
struct ClangCounts {
    function_definitions: u64,
    call_expressions: u64,
}

fn count_clang_ast(value: &serde_json::Value) -> ClangCounts {
    let mut counts = ClangCounts::default();
    count_clang_ast_inner(value, &mut counts);
    counts
}

fn count_clang_ast_inner(value: &serde_json::Value, counts: &mut ClangCounts) {
    if let Some(object) = value.as_object() {
        if object
            .get("kind")
            .and_then(serde_json::Value::as_str)
            == Some("CallExpr")
        {
            counts.call_expressions = counts.call_expressions.saturating_add(1);
        }
        if object
            .get("kind")
            .and_then(serde_json::Value::as_str)
            == Some("FunctionDecl")
            && object
                .get("inner")
                .is_some_and(contains_compound_stmt)
        {
            counts.function_definitions = counts.function_definitions.saturating_add(1);
        }
        if let Some(inner) = object.get("inner") {
            count_clang_ast_inner(inner, counts);
        }
    } else if let Some(array) = value.as_array() {
        for item in array {
            count_clang_ast_inner(item, counts);
        }
    }
}

fn contains_compound_stmt(value: &serde_json::Value) -> bool {
    if let Some(array) = value.as_array() {
        return array.iter().any(contains_compound_stmt);
    }
    let Some(object) = value.as_object() else {
        return false;
    };
    if object
        .get("kind")
        .and_then(serde_json::Value::as_str)
        == Some("CompoundStmt")
    {
        return true;
    }
    object.get("inner").is_some_and(contains_compound_stmt)
}

fn collect_c_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|error| format!("read_dir {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read_dir entry {}: {error}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("file_type {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_c_files(&path, out)?;
        } else if file_type.is_file()
            && path.extension().and_then(|ext| ext.to_str()) == Some("c")
        {
            out.push(path);
        }
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parse {}: {error}", path.display()))
}

fn json_u64(value: &serde_json::Value, field: &str) -> u64 {
    value
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

fn read_vyre_file_records(report: &serde_json::Value) -> HashMap<String, (u64, u64)> {
    let mut records = HashMap::new();
    let Some(files) = report.get("files").and_then(serde_json::Value::as_array) else {
        return records;
    };
    for file in files {
        let Some(path) = file.get("path").and_then(serde_json::Value::as_str) else {
            continue;
        };
        records.insert(
            path.to_string(),
            (
                json_u64(file, "function_records"),
                json_u64(file, "call_records"),
            ),
        );
    }
    records
}

fn lookup_vyre_file_record(
    records: &HashMap<String, (u64, u64)>,
    source: &Path,
) -> Option<(u64, u64)> {
    let display = source.display().to_string();
    if let Some(record) = records.get(&display).copied() {
        return Some(record);
    }
    if let Ok(canonical) = fs::canonicalize(source) {
        if let Some(record) = records.get(&canonical.display().to_string()).copied() {
            return Some(record);
        }
    }
    let source_norm = display.replace('\\', "/");
    records
        .iter()
        .find_map(|(path, record)| {
            let path_norm = path.replace('\\', "/");
            (source_norm.ends_with(&path_norm) || path_norm.ends_with(&source_norm))
                .then_some(*record)
        })
}

fn coverage_x1000(vyre: u64, clang: u64) -> u64 {
    if clang == 0 {
        return 0;
    }
    vyre.saturating_mul(1000) / clang
}

fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut corpus = None;
    let mut vyre_report = None;
    let mut output = PathBuf::from("release/evidence/parser/c-parser-clang-oracle.json");
    let mut clang = "clang".to_string();
    let mut include_dirs = Vec::new();
    let mut macros = Vec::new();

    let mut limit = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--corpus" => {
                i += 1;
                corpus = args.get(i).map(PathBuf::from);
            }
            "--vyre-report" => {
                i += 1;
                vyre_report = args.get(i).map(PathBuf::from);
            }
            "--output" => {
                i += 1;
                output = args
                    .get(i)
                    .map(PathBuf::from)
                    .ok_or_else(|| "Fix: --output requires PATH".to_string())?;
            }
            "--clang" => {
                i += 1;
                clang = args
                    .get(i)
                    .cloned()
                    .ok_or_else(|| "Fix: --clang requires COMMAND".to_string())?;
            }
            "--limit" => {
                i += 1;
                let raw = args
                    .get(i)
                    .ok_or_else(|| "Fix: --limit requires N".to_string())?;
                limit = Some(
                    raw.parse::<usize>()
                        .map_err(|error| format!("Fix: invalid --limit `{raw}`: {error}"))?,
                );
            }
            "-I" => {
                i += 1;
                include_dirs.push(
                    args.get(i)
                        .map(PathBuf::from)
                        .ok_or_else(|| "Fix: -I requires DIR".to_string())?,
                );
            }
            "-D" => {
                i += 1;
                macros.push(
                    args.get(i)
                        .cloned()
                        .ok_or_else(|| "Fix: -D requires NAME[=VALUE]".to_string())?,
                );
            }
            "--help" | "-h" => {
                return Err(
                    "USAGE:\n  cargo_full run --bin xtask -- c-parser-clang-oracle --corpus DIR --vyre-report release/evidence/parser/c-parser-linux-subsystem.json [--output PATH] [-I DIR] [-D NAME[=VALUE]] [--clang clang] [--limit N]"
                        .to_string(),
                );
            }
            other => return Err(format!("Fix: unknown c-parser-clang-oracle option `{other}`")),
        }
        i += 1;
    }
    Ok(Config {
        corpus: corpus.ok_or_else(|| "Fix: --corpus DIR is required".to_string())?,
        vyre_report: vyre_report
            .ok_or_else(|| "Fix: --vyre-report PATH is required".to_string())?,
        output,
        clang,
        include_dirs,
        macros,
        limit,
    })
}

