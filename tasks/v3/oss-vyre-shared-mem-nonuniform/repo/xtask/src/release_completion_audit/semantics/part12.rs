fn inspect_c_parser_manifest_semantics(
    evidence: &str,
    path: &Path,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let file_count = value
        .get("file_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let source_bytes = value
        .get("total_source_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let entries = value
        .get("files")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len) as u64;
    if file_count < 250 {
        blockers.push(format!(
            "{evidence}: file_count {file_count} is below Linux subsystem floor 250"
        ));
    }
    if source_bytes < 4 * 1024 * 1024 {
        blockers.push(format!(
            "{evidence}: total_source_bytes {source_bytes} is below Linux subsystem floor 4194304"
        ));
    }
    if value
        .get("linux_subsystem_candidate")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        blockers.push(format!(
            "{evidence}: linux_subsystem_candidate must be true"
        ));
    }
    if value
        .get("corpus_root_canonical")
        .and_then(serde_json::Value::as_str)
        .is_none_or(str::is_empty)
    {
        blockers.push(format!("{evidence}: missing corpus_root_canonical"));
    }
    inspect_corpus_fingerprint(evidence, value, blockers);
    inspect_linux_subsystem_provenance(evidence, value, blockers);
    inspect_c_parser_collection_provenance(evidence, value, blockers);
    for field in ["include_dirs", "macros"] {
        if value
            .get(field)
            .and_then(serde_json::Value::as_array)
            .is_none_or(Vec::is_empty)
        {
            blockers.push(format!(
                "{evidence}: reproducibility field `{field}` must be non-empty"
            ));
        }
    }
    if entries != file_count {
        blockers.push(format!(
            "{evidence}: files array has {entries} entries, file_count is {file_count}"
        ));
    }
    if let Some(parse_report) = read_sibling_json(path, "c-parser-linux-subsystem.json", blockers) {
        for (manifest_field, parse_field) in [
            ("file_count", "total_files"),
            ("total_source_bytes", "total_source_bytes"),
            ("linux_subsystem_candidate", "linux_subsystem_candidate"),
            ("corpus_root_canonical", "corpus_root_canonical"),
            ("linux_root", "linux_root"),
            ("linux_subsystem", "linux_subsystem"),
            ("linux_subsystem_depth", "linux_subsystem_depth"),
            ("linux_kbuild_file", "linux_kbuild_file"),
            ("linux_kbuild_file_in_corpus", "linux_kbuild_file_in_corpus"),
            ("corpus_fingerprint", "corpus_fingerprint"),
            ("source_collection_mode", "source_collection_mode"),
            ("visited_dir_count", "visited_dir_count"),
            ("include_dirs", "include_dirs"),
            ("macros", "macros"),
        ] {
            let manifest_value = value.get(manifest_field);
            let parse_value = parse_report.get(parse_field);
            if manifest_value != parse_value {
                blockers.push(format!(
                    "{evidence}: `{manifest_field}` does not match c-parser-linux-subsystem.json `{parse_field}`"
                ));
            }
        }
        let parsed_files = parse_report
            .get("parsed_files")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if file_count != parsed_files {
            blockers.push(format!(
                "{evidence}: file_count {file_count} does not match c-parser-linux-subsystem.json parsed_files {parsed_files}"
            ));
        }
        let parse_entries = parse_report
            .get("files")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len) as u64;
        if entries != parse_entries {
            blockers.push(format!(
                "{evidence}: files array has {entries} entries but c-parser-linux-subsystem.json has {parse_entries}"
            ));
        }
    }
    if let Some(files) = value.get("files").and_then(serde_json::Value::as_array) {
        for file in files {
            let path = file
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            if file
                .get("source_bytes")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                blockers.push(format!(
                    "{evidence}: manifest file `{path}` has zero source_bytes"
                ));
            }
            if file.get("parsed").and_then(serde_json::Value::as_bool) != Some(true) {
                blockers.push(format!(
                    "{evidence}: manifest file `{path}` was not parsed successfully"
                ));
                continue;
            }
            for field in [
                "object_bytes",
                "ast_bytes",
                "vast_bytes",
                "semantic_graph_bytes",
            ] {
                if file
                    .get(field)
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    == 0
                {
                    blockers.push(format!(
                        "{evidence}: manifest file `{path}` has zero `{field}`"
                    ));
                }
            }
            if file
                .get("wall_ns")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                blockers.push(format!(
                    "{evidence}: manifest file `{path}` has zero wall_ns"
                ));
            }
        }
    }
}

fn inspect_linux_subsystem_provenance(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    for field in ["linux_root", "linux_subsystem", "linux_kbuild_file"] {
        if value
            .get(field)
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        {
            blockers.push(format!(
                "{evidence}: missing Linux provenance field `{field}`"
            ));
        }
    }
    if value
        .get("linux_kbuild_file_in_corpus")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        blockers.push(format!(
            "{evidence}: linux_kbuild_file_in_corpus must be true"
        ));
    }
    let linux_subsystem = value
        .get("linux_subsystem")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !matches!(
        linux_subsystem,
        "kernel" | "fs" | "mm" | "net" | "drivers" | "lib"
    ) {
        blockers.push(format!(
            "{evidence}: unsupported linux_subsystem `{linux_subsystem}`"
        ));
    }
    let linux_depth = value
        .get("linux_subsystem_depth")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if linux_depth == 0 {
        blockers.push(format!(
            "{evidence}: linux_subsystem_depth must be greater than zero"
        ));
    }
}

fn inspect_c_parser_collection_provenance(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    if value
        .get("source_collection_mode")
        .and_then(serde_json::Value::as_str)
        != Some("recursive_all_c_files")
    {
        blockers.push(format!(
            "{evidence}: source_collection_mode must be recursive_all_c_files"
        ));
    }
    let visited_dir_count = value
        .get("visited_dir_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if visited_dir_count == 0 {
        blockers.push(format!(
            "{evidence}: visited_dir_count must prove recursive corpus traversal"
        ));
    }
}

fn inspect_corpus_fingerprint(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    if value
        .get("corpus_fingerprint")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|fingerprint| !fingerprint.starts_with("fnv64:"))
    {
        blockers.push(format!("{evidence}: missing stable corpus_fingerprint"));
    }
}

fn inspect_c_parser_diagnostics_semantics(
    evidence: &str,
    path: &Path,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let failed = value
        .get("failed_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let failures = value
        .get("failures")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len) as u64;
    if failed != 0 || failures != 0 {
        blockers.push(format!(
            "{evidence}: parser diagnostics still report failed_files={failed}, failure entries={failures}"
        ));
    }
    if let Some(parse_report) = read_sibling_json(path, "c-parser-linux-subsystem.json", blockers) {
        let parse_failed = parse_report
            .get("failed_files")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::MAX);
        if failed != parse_failed {
            blockers.push(format!(
                "{evidence}: failed_files {failed} does not match c-parser-linux-subsystem.json failed_files {parse_failed}"
            ));
        }
        let parse_failures = parse_report
            .get("failures")
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len) as u64;
        if failures != parse_failures {
            blockers.push(format!(
                "{evidence}: failure entries {failures} do not match c-parser-linux-subsystem.json failure entries {parse_failures}"
            ));
        }
    }
}

fn inspect_c_parser_throughput_semantics(
    evidence: &str,
    path: &Path,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let total = value
        .get("total_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let parsed = value
        .get("parsed_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if value
        .get("linux_subsystem_candidate")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        blockers.push(format!(
            "{evidence}: linux_subsystem_candidate must be true"
        ));
    }
    if value
        .get("corpus_root_canonical")
        .and_then(serde_json::Value::as_str)
        .is_none_or(str::is_empty)
    {
        blockers.push(format!("{evidence}: missing corpus_root_canonical"));
    }
    inspect_corpus_fingerprint(evidence, value, blockers);
    inspect_linux_subsystem_provenance(evidence, value, blockers);
    inspect_c_parser_collection_provenance(evidence, value, blockers);
    for field in ["include_dirs", "macros"] {
        if value
            .get(field)
            .and_then(serde_json::Value::as_array)
            .is_none_or(|items| items.is_empty())
        {
            blockers.push(format!("{evidence}: `{field}` must be non-empty"));
        }
    }
    let source_bytes = value
        .get("total_source_bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let wall_ns = value
        .get("wall_ns")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let files_per_second = value
        .get("files_per_second_x1000")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let mib_per_second = value
        .get("mib_per_second_x1000")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if total < 250 || parsed != total {
        blockers.push(format!(
            "{evidence}: throughput covers parsed_files={parsed}, total_files={total}; full Linux subsystem throughput requires at least 250 parsed files"
        ));
    }
    if source_bytes < 4 * 1024 * 1024 {
        blockers.push(format!(
            "{evidence}: total_source_bytes {source_bytes} is below Linux subsystem floor 4194304"
        ));
    }
    if wall_ns == 0 || files_per_second == 0 || mib_per_second == 0 {
        blockers.push(format!(
            "{evidence}: throughput rates are incomplete: wall_ns={wall_ns}, files_per_second_x1000={files_per_second}, mib_per_second_x1000={mib_per_second}"
        ));
    }
    if let Some(parse_report) = read_sibling_json(path, "c-parser-linux-subsystem.json", blockers) {
        for field in [
            "total_files",
            "parsed_files",
            "total_source_bytes",
            "include_dirs",
            "macros",
            "corpus_root_canonical",
            "linux_subsystem_candidate",
            "linux_root",
            "linux_subsystem",
            "linux_subsystem_depth",
            "linux_kbuild_file",
            "linux_kbuild_file_in_corpus",
            "corpus_fingerprint",
            "source_collection_mode",
            "visited_dir_count",
        ] {
            if value.get(field) != parse_report.get(field) {
                blockers.push(format!(
                    "{evidence}: throughput field `{field}` does not match c-parser-linux-subsystem.json"
                ));
            }
        }
    }
}

fn read_sibling_json(
    path: &Path,
    sibling: &str,
    blockers: &mut Vec<String>,
) -> Option<serde_json::Value> {
    let sibling_path = path.parent()?.join(sibling);
    let text = match read_text_bounded(&sibling_path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "{}: failed to read sibling artifact `{}`: {error}",
                path.display(),
                sibling_path.display()
            ));
            return None;
        }
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            blockers.push(format!(
                "{}: sibling artifact `{}` is invalid JSON: {error}",
                path.display(),
                sibling_path.display()
            ));
            None
        }
    }
}

fn is_parser_contract_evidence(evidence: &str) -> bool {
    [
        "vyre-frontend-c-contracts.json",
        "vyrec-cli-contracts.json",
        "weir-contracts.json",
        "surgec-contracts.json",
        "surgec-grammar-gen-contracts.json",
    ]
    .iter()
    .any(|suffix| evidence.ends_with(suffix))
}

fn inspect_distributed_parser_map_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let Some(components) = value
        .get("components")
        .and_then(serde_json::Value::as_array)
    else {
        blockers.push(format!("{evidence}: missing components array"));
        return;
    };
    for required in [
        "vyre-frontend-c",
        "vyrec",
        "weir",
        "surgec",
        "surgec-grammar-gen",
    ] {
        if !components.iter().any(|component| {
            component.get("id").and_then(serde_json::Value::as_str) == Some(required)
                && component.get("exists").and_then(serde_json::Value::as_bool) == Some(true)
                && component
                    .get("missing_terms")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(Vec::is_empty)
                && component
                    .get("missing_contract_topics")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(Vec::is_empty)
                && component
                    .get("required_test_categories")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|categories| !categories.is_empty())
                && component
                    .get("missing_test_categories")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(Vec::is_empty)
                && component
                    .get("unresolved_ownership_markers")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(Vec::is_empty)
                && component
                    .get("required_files")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|files| {
                        !files.is_empty()
                            && files.iter().all(|file| {
                                file.get("exists").and_then(serde_json::Value::as_bool)
                                    == Some(true)
                                    && file
                                        .get("source_bytes")
                                        .and_then(serde_json::Value::as_u64)
                                        .unwrap_or(0)
                                        > 0
                                    && file
                                        .get("read_error")
                                        .is_some_and(serde_json::Value::is_null)
                            })
                    })
        }) {
            blockers.push(format!(
                "{evidence}: missing complete parser ownership component `{required}`"
            ));
        }
    }
}
