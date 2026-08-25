#[test]
fn op_id_literals_match_their_catalog_tier() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap();
    let roots = [
        (
            workspace_root.join("vyre-primitives/src"),
            "vyre-primitives::",
            "vyre-libs::",
        ),
        (
            workspace_root.join("vyre-libs/src"),
            "vyre-libs::",
            "vyre-primitives::",
        ),
    ];
    let mut violations = Vec::new();

    for (root, required_prefix, forbidden_prefix) in roots {
        for path in rust_files_under(root) {
            let text = std::fs::read_to_string(&path).unwrap();
            for (line_index, line) in text.lines().enumerate() {
                let Some(value) = op_id_literal_value(line) else {
                    continue;
                };
                if value.starts_with(forbidden_prefix) || !value.starts_with(required_prefix) {
                    violations.push(format!(
                        "{}:{} op id `{}` must start with `{}`",
                        path.display(),
                        line_index + 1,
                        value,
                        required_prefix
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "op id literals must name the tier that owns the implementation. \
         Wrapper ops may compose lower-tier ops, but they must keep distinct ids. Violations:\n{}",
        violations.join("\n")
    );
}

fn manifest_dep_line_matches(line: &str, dep: &str) -> bool {
    let line = line.trim_start();
    let direct_dep = format!("{dep} ");
    let table_dep = format!("{dep}.");
    line.starts_with(dep) || line.starts_with(&direct_dep) || line.starts_with(&table_dep)
}

fn manifest_section<'a>(manifest: &'a str, section: &str) -> &'a str {
    let header = format!("[{section}]");
    let Some(start) = manifest.find(&header) else {
        return "";
    };
    let body = &manifest[start + header.len()..];
    body.find("\n[")
        .map(|end| &body[..end])
        .unwrap_or(body)
        .trim()
}

fn rust_files_under(root: PathBuf) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out
}

fn op_id_literal_value(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if !trimmed.contains("OP_ID") || !trimmed.contains("const ") {
        return None;
    }
    let quote_start = trimmed.find('"')?;
    let rest = &trimmed[quote_start + 1..];
    let quote_end = rest.find('"')?;
    Some(&rest[..quote_end])
}
