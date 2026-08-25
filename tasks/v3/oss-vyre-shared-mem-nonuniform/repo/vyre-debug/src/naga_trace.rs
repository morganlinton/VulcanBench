use naga::Module;
use std::fs::File;
use std::io::{BufRead, BufReader};
use vyre_emit_naga::BindResultEntry;

/// Error type for [`load_bind_result_log`] failures.
#[derive(Debug)]
pub enum BindResultLogError {
    /// The log file could not be opened (missing, permission denied, etc.).
    Open(std::io::Error),
    /// A line could not be read from the file.
    Read(std::io::Error),
    /// A line in the log was not valid JSON for [`BindResultEntry`].
    /// Contains the line number (1-based) and the raw parse error.
    Parse(usize, serde_json::Error),
}

pub struct FailureTrace {
    pub text: String,
}

pub fn failure_trace(module: &Module, error: &naga::valid::ValidationError) -> FailureTrace {
    let text = format!(
        "FAILURE: {:#?}\nentry_points={}\nfunctions={}\nglobals={}",
        error,
        module.entry_points.len(),
        module.functions.len(),
        module.global_variables.len()
    );
    FailureTrace { text }
}

pub fn failure_trace_wgsl(
    module: &Module,
    info: &naga::valid::ModuleInfo,
    err: &naga::back::wgsl::Error,
) -> FailureTrace {
    let text = format!(
        "FAILURE: {:#?}\nentry_points={}\nfunctions={}\nglobals={}\nmodule_info={:#?}",
        err,
        module.entry_points.len(),
        module.functions.len(),
        module.global_variables.len(),
        info
    );
    FailureTrace { text }
}

/// Load a bind-result log file produced by vyre-emit-naga.
///
/// Returns `Err` on any I/O or parse failure so the caller can surface the
/// problem. Never silently returns a partial or empty result — the complete
/// log is required for accurate trace data.
pub fn load_bind_result_log(
    path: &str,
) -> Result<Vec<BindResultEntry>, BindResultLogError> {
    let file = File::open(path).map_err(BindResultLogError::Open)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for (line_no, raw) in reader.lines().enumerate() {
        let line = raw.map_err(BindResultLogError::Read)?;
        let entry: BindResultEntry = serde_json::from_str(&line)
            .map_err(|e| BindResultLogError::Parse(line_no + 1, e))?;
        entries.push(entry);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_bind_result_log_missing_file_returns_open_error() {
        let r = load_bind_result_log("/nonexistent/path/bind_log.jsonl");
        assert!(
            matches!(r, Err(BindResultLogError::Open(_))),
            "expected Err(Open(_)) for missing file, got {:?}",
            r.err().map(|e| format!("{e:?}"))
        );
    }

    #[test]
    fn load_bind_result_log_valid_entries_parses_all() {
        // Build a minimal valid BindResultEntry JSON line.
        let entry = BindResultEntry {
            vyre_op_id: 7,
            op_kind: "Literal".to_string(),
            init_handle: 42,
            init_scalar_kind: Some("Uint".to_string()),
            child_body_depth: 0,
            value_types_at_call: None,
            publish_path: "root/op7".to_string(),
            local_allocated_ty: None,
        };
        let line = serde_json::to_string(&entry).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bind.jsonl");
        std::fs::write(&path, format!("{line}\n")).unwrap();

        let result = load_bind_result_log(path.to_str().unwrap()).unwrap();
        assert_eq!(result.len(), 1, "expected exactly 1 entry");
        assert_eq!(result[0].vyre_op_id, 7);
        assert_eq!(result[0].op_kind, "Literal");
        assert_eq!(result[0].init_handle, 42);
        assert_eq!(result[0].publish_path, "root/op7");
    }

    #[test]
    fn load_bind_result_log_malformed_json_line_returns_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.jsonl");
        // First line is valid, second is malformed.
        let entry = BindResultEntry {
            vyre_op_id: 1,
            op_kind: "Load".to_string(),
            init_handle: 0,
            init_scalar_kind: None,
            child_body_depth: 0,
            value_types_at_call: None,
            publish_path: "p".to_string(),
            local_allocated_ty: None,
        };
        let valid_line = serde_json::to_string(&entry).unwrap();
        std::fs::write(&path, format!("{valid_line}\nnot valid json\n")).unwrap();

        let r = load_bind_result_log(path.to_str().unwrap());
        assert!(
            matches!(r, Err(BindResultLogError::Parse(2, _))),
            "expected Err(Parse(2, _)) for malformed line 2, got {:?}",
            r.err().map(|e| format!("{e:?}"))
        );
    }
}
