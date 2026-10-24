//! Host-side tool executor (`harness/agent/local_executor.py`): the agent's
//! tools run directly in the workspace directory.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::error::EngineError;
use crate::json::{self, obj, Value};
use crate::process;
use crate::protocol::{
    EditFileArgs, ListFilesArgs, ReadFileArgs, RunCommandArgs, SearchCodeArgs, ToolCall,
    ToolObservation,
};
use crate::pyfmt::{py_repr_str, py_splitlines, universal_newlines};
use crate::tasks::{should_ignore, vulcanbenchignore_paths, walk_files};

pub const LIST_FILES_CAP: usize = 500;
pub const SEARCH_CODE_CAP: usize = 100;

#[derive(Debug)]
pub struct LocalToolExecutor {
    workspace: PathBuf,
}

/// Python's `str(OSError)` for the failures a tool can hit on a path.
pub fn py_os_error(err: &io::Error, path: &Path) -> String {
    let quoted = py_repr_str(&path.to_string_lossy());
    match err.kind() {
        io::ErrorKind::NotFound => format!("[Errno 2] No such file or directory: {quoted}"),
        io::ErrorKind::PermissionDenied => format!("[Errno 13] Permission denied: {quoted}"),
        io::ErrorKind::IsADirectory => format!("[Errno 21] Is a directory: {quoted}"),
        io::ErrorKind::NotADirectory => format!("[Errno 20] Not a directory: {quoted}"),
        _ => err.to_string(),
    }
}

/// `Path.resolve()` with `strict=False`: canonicalize the longest existing
/// prefix, then normalize the rest lexically.
pub fn resolve_path(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while !existing.exists() {
        let (name, parent) = match (existing.file_name(), existing.parent()) {
            (Some(name), Some(parent)) => (name.to_owned(), parent.to_path_buf()),
            _ => break,
        };
        tail.push(name);
        existing = parent;
    }
    let mut out = fs::canonicalize(&existing).unwrap_or(existing);
    for part in tail.iter().rev() {
        match Path::new(part).components().next() {
            Some(Component::ParentDir) => {
                out.pop();
            }
            Some(Component::CurDir) | None => {}
            _ => out.push(part),
        }
    }
    out
}

/// `lines[start:end]` with Python slice semantics for a non-negative start.
fn py_slice<T: Clone>(items: &[T], start: i64, end: i64) -> Vec<T> {
    let len = i64::try_from(items.len()).unwrap_or(i64::MAX);
    let start = start.clamp(0, len);
    let end = if end < 0 { end + len } else { end }.clamp(0, len);
    if end <= start {
        return Vec::new();
    }
    items[start as usize..end as usize].to_vec()
}

fn read_text_py(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| py_os_error(&e, path))?;
    let text = String::from_utf8(bytes).map_err(|e| {
        let pos = e.utf8_error().valid_up_to();
        let byte = e.as_bytes().get(pos).copied().unwrap_or(0);
        format!(
            "'utf-8' codec can't decode byte 0x{byte:02x} in position {pos}: invalid start byte"
        )
    })?;
    Ok(universal_newlines(&text))
}

impl LocalToolExecutor {
    /// # Errors
    ///
    /// When the workspace cannot be created or resolved.
    pub fn new(workspace: &Path) -> Result<Self, EngineError> {
        fs::create_dir_all(workspace)
            .map_err(|e| EngineError::io(format!("creating {}", workspace.display()), e))?;
        let workspace = fs::canonicalize(workspace)
            .map_err(|e| EngineError::io(format!("resolving {}", workspace.display()), e))?;
        Ok(Self { workspace })
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub(crate) fn resolve(&self, p: &str) -> Result<PathBuf, String> {
        let path = resolve_path(&self.workspace.join(p));
        if !path.starts_with(&self.workspace) {
            return Err("path escapes workspace".to_owned());
        }
        Ok(path)
    }

    fn rel_posix(&self, path: &Path) -> String {
        path.strip_prefix(&self.workspace)
            .unwrap_or(path)
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/")
    }

    /// # Errors
    ///
    /// The Python exception text for an unreadable directory.
    pub fn list_files(&self, args: &ListFilesArgs) -> Result<Value, String> {
        let root = self.resolve(&args.dir)?;
        if !root.exists() {
            return Ok(Value::Array(Vec::new()));
        }
        let ignored = vulcanbenchignore_paths(&self.workspace);
        let files: Vec<PathBuf> = if args.recursive {
            walk_files(&root)
                .map_err(|e| py_os_error(&e, &root))?
                .into_iter()
                .map(|rel| root.join(rel))
                .collect()
        } else {
            fs::read_dir(&root)
                .map_err(|e| py_os_error(&e, &root))?
                .flatten()
                .map(|entry| entry.path())
                .filter(|p| p.is_file())
                .collect()
        };
        let mut rels: Vec<String> = files
            .iter()
            .filter(|p| {
                let rel = p.strip_prefix(&self.workspace).unwrap_or(p);
                !should_ignore(rel, &ignored)
            })
            .map(|p| self.rel_posix(p))
            .collect();
        rels.sort();
        if rels.len() <= LIST_FILES_CAP {
            return Ok(Value::from(rels));
        }
        let total = rels.len();
        rels.truncate(LIST_FILES_CAP);
        Ok(obj! {
            "files" => rels,
            "truncated" => true,
            "total" => total,
            "message" => format!("listing capped at {LIST_FILES_CAP} files; use search_code or narrower dir"),
        })
    }

    /// # Errors
    ///
    /// The Python exception text for a missing or undecodable file.
    pub fn read_file(&self, args: &ReadFileArgs) -> Result<Value, String> {
        let path = self.resolve(&args.path)?;
        let mut text = read_text_py(&path)?;
        if let Some(start_line) = args.start_line {
            let lines = py_splitlines(&text, true);
            let start = (start_line - 1).max(0);
            let limit = args.limit.filter(|l| *l != 0);
            let end =
                start + limit.unwrap_or_else(|| i64::try_from(lines.len()).unwrap_or(i64::MAX));
            text = py_slice(&lines, start, end).concat();
        }
        Ok(Value::Str(text))
    }

    /// # Errors
    ///
    /// The Python exception text when ripgrep fails or times out.
    pub fn search_code(&self, args: &SearchCodeArgs) -> Result<Value, String> {
        let ignored = vulcanbenchignore_paths(&self.workspace);
        let timeout = args.timeout.filter(|t| *t != 0).unwrap_or(30);
        if process::which("rg").is_some() {
            let mut command = Command::new("rg");
            command
                .args(["--json", "-n"])
                .arg(&args.query)
                .arg(&self.workspace);
            if let Some(glob) = &args.glob {
                command.args(["--glob", glob]);
            }
            let done = process::run(
                &mut command,
                Some(Duration::from_secs(timeout.max(0) as u64)),
            )
            .map_err(|e| e.to_string())?;
            if done.timed_out {
                return Err(format!("Command 'rg' timed out after {timeout} seconds"));
            }
            let mut results = Vec::new();
            for line in done.stdout.lines().filter(|l| !l.trim().is_empty()) {
                let Ok(obj) = json::parse(line) else { continue };
                if obj.get("type").and_then(Value::as_str) != Some("match") {
                    continue;
                }
                let path_text = obj
                    .get("data")
                    .and_then(|d| d.get("path"))
                    .and_then(|p| p.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if path_text.is_empty() {
                    continue;
                }
                let rel = Path::new(path_text)
                    .strip_prefix(&self.workspace)
                    .unwrap_or(Path::new(path_text));
                if should_ignore(rel, &ignored) {
                    continue;
                }
                results.push(obj);
            }
            return Ok(cap_search_results(results));
        }
        let needle = args.query.to_lowercase();
        let mut matches = Vec::new();
        for rel in walk_files(&self.workspace).unwrap_or_default() {
            if rel.extension().and_then(|e| e.to_str()) != Some("py")
                || should_ignore(&rel, &ignored)
            {
                continue;
            }
            if let Ok(text) = read_text_py(&self.workspace.join(&rel)) {
                if text.to_lowercase().contains(&needle) {
                    matches.push(obj! { "path" => self.rel_posix(&self.workspace.join(&rel)) });
                }
            }
        }
        Ok(cap_search_results(matches))
    }

    /// # Errors
    ///
    /// `old_string not found exactly`, or the Python exception text for I/O.
    pub fn edit_file(&self, args: &EditFileArgs) -> Result<Value, String> {
        let path = self.resolve(&args.path)?;
        if !path.exists() && args.old_string.is_empty() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| py_os_error(&e, parent))?;
            }
            fs::write(&path, &args.new_string).map_err(|e| py_os_error(&e, &path))?;
            return Ok(Value::Str(format!("+ {}", args.new_string)));
        }
        let original = read_text_py(&path)?;
        if !original.contains(&args.old_string) {
            return Err("old_string not found exactly".to_owned());
        }
        let new_text = original.replacen(&args.old_string, &args.new_string, 1);
        fs::write(&path, &new_text).map_err(|e| py_os_error(&e, &path))?;
        let name = path.to_string_lossy();
        Ok(Value::Str(unified_diff(
            &py_splitlines(&original, true),
            &py_splitlines(&new_text, true),
            &name,
            &name,
        )))
    }

    /// `subprocess.run(cmd, shell=True, ...)` in the workspace (or `cwd`).
    ///
    /// # Errors
    ///
    /// `TimeoutExpired` text on timeout, the Python exception text otherwise.
    pub fn run_command(&self, args: &RunCommandArgs) -> Result<Value, String> {
        let cwd = match &args.cwd {
            Some(dir) => self.resolve(dir)?,
            None => self.workspace.clone(),
        };
        let timeout = args.timeout.filter(|t| *t != 0).unwrap_or(120);
        if timeout < 0 {
            return Err(process::timeout_message(&args.cmd, timeout));
        }
        let done = process::shell(&args.cmd, &cwd, Some(Duration::from_secs(timeout as u64)))
            .map_err(|e| py_os_error(&e, &cwd))?;
        if done.timed_out {
            return Err(process::timeout_message(&args.cmd, timeout));
        }
        Ok(obj! {
            "stdout" => done.stdout,
            "stderr" => done.stderr,
            "exit_code" => i64::from(done.exit_code),
        })
    }

    /// `ToolProtocol.execute`: coerce the arguments, run the tool, and turn
    /// any failure into an error observation instead of a crash.
    pub fn execute(&self, call: &ToolCall) -> ToolObservation {
        let result = match call.tool.as_str() {
            "list_files" => ListFilesArgs::from_args(&call.args).and_then(|a| self.list_files(&a)),
            "read_file" => ReadFileArgs::from_args(&call.args).and_then(|a| self.read_file(&a)),
            "search_code" => {
                SearchCodeArgs::from_args(&call.args).and_then(|a| self.search_code(&a))
            }
            "edit_file" => EditFileArgs::from_args(&call.args).and_then(|a| self.edit_file(&a)),
            "run_command" => {
                RunCommandArgs::from_args(&call.args).and_then(|a| self.run_command(&a))
            }
            other => Err(format!("unknown tool {other}")),
        };
        match result {
            Ok(value) => ToolObservation::ok(value),
            Err(message) => ToolObservation::err(message),
        }
    }
}

fn cap_search_results(results: Vec<Value>) -> Value {
    if results.len() <= SEARCH_CODE_CAP {
        return Value::Array(results);
    }
    let total = results.len();
    let mut head = results;
    head.truncate(SEARCH_CODE_CAP);
    obj! {
        "matches" => head,
        "truncated" => true,
        "total" => total,
        "message" => format!("search capped at {SEARCH_CODE_CAP} matches; refine query or glob"),
    }
}

// ---------------------------------------------------------------------------
// Unified diff (difflib.unified_diff's output format over an LCS alignment)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Equal,
    Replace,
    Delete,
    Insert,
}

type Opcode = (Tag, usize, usize, usize, usize);

/// Cells above which the LCS table is not built and the files are compared
/// as one replaced block between their common prefix and suffix.
const MAX_LCS_CELLS: usize = 16_000_000;

fn opcodes(a: &[String], b: &[String]) -> Vec<Opcode> {
    let prefix = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (a_mid, b_mid) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    let mut codes = Vec::new();
    if prefix > 0 {
        codes.push((Tag::Equal, 0, prefix, 0, prefix));
    }
    let (n, m) = (a_mid.len(), b_mid.len());
    if n * m > MAX_LCS_CELLS {
        push_change(&mut codes, prefix, prefix + n, prefix, prefix + m);
    } else if n > 0 || m > 0 {
        let mut table = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                table[i * (m + 1) + j] = if a_mid[i] == b_mid[j] {
                    table[(i + 1) * (m + 1) + j + 1] + 1
                } else {
                    table[(i + 1) * (m + 1) + j].max(table[i * (m + 1) + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        let (mut ci, mut cj) = (0, 0);
        while i < n && j < m {
            if a_mid[i] == b_mid[j] {
                push_change(&mut codes, prefix + ci, prefix + i, prefix + cj, prefix + j);
                let start = (i, j);
                while i < n && j < m && a_mid[i] == b_mid[j] {
                    i += 1;
                    j += 1;
                }
                codes.push((
                    Tag::Equal,
                    prefix + start.0,
                    prefix + i,
                    prefix + start.1,
                    prefix + j,
                ));
                ci = i;
                cj = j;
            } else if table[(i + 1) * (m + 1) + j] >= table[i * (m + 1) + j + 1] {
                i += 1;
            } else {
                j += 1;
            }
        }
        push_change(&mut codes, prefix + ci, prefix + n, prefix + cj, prefix + m);
    }
    if suffix > 0 {
        codes.push((
            Tag::Equal,
            a.len() - suffix,
            a.len(),
            b.len() - suffix,
            b.len(),
        ));
    }
    codes
}

fn push_change(codes: &mut Vec<Opcode>, i1: usize, i2: usize, j1: usize, j2: usize) {
    let tag = match (i1 < i2, j1 < j2) {
        (true, true) => Tag::Replace,
        (true, false) => Tag::Delete,
        (false, true) => Tag::Insert,
        (false, false) => return,
    };
    codes.push((tag, i1, i2, j1, j2));
}

/// `SequenceMatcher.get_grouped_opcodes(n=3)`.
fn grouped_opcodes(mut codes: Vec<Opcode>, n: usize) -> Vec<Vec<Opcode>> {
    if codes.is_empty() {
        codes.push((Tag::Equal, 0, 1, 0, 1));
    }
    if let Some(first) = codes.first_mut() {
        if first.0 == Tag::Equal {
            *first = (
                Tag::Equal,
                first.1.max(first.2.saturating_sub(n)),
                first.2,
                first.3.max(first.4.saturating_sub(n)),
                first.4,
            );
        }
    }
    if let Some(last) = codes.last_mut() {
        if last.0 == Tag::Equal {
            *last = (
                Tag::Equal,
                last.1,
                last.2.min(last.1 + n),
                last.3,
                last.4.min(last.3 + n),
            );
        }
    }
    let mut groups = Vec::new();
    let mut group: Vec<Opcode> = Vec::new();
    for (tag, mut i1, i2, mut j1, j2) in codes {
        if tag == Tag::Equal && i2 - i1 > 2 * n {
            group.push((tag, i1, i2.min(i1 + n), j1, j2.min(j1 + n)));
            groups.push(std::mem::take(&mut group));
            i1 = i1.max(i2.saturating_sub(n));
            j1 = j1.max(j2.saturating_sub(n));
        }
        group.push((tag, i1, i2, j1, j2));
    }
    let only_context = group.len() == 1 && group[0].0 == Tag::Equal;
    if !group.is_empty() && !only_context {
        groups.push(group);
    }
    groups
}

fn format_range(start: usize, stop: usize) -> String {
    let mut beginning = start + 1;
    let length = stop.saturating_sub(start);
    if length == 1 {
        return beginning.to_string();
    }
    if length == 0 {
        beginning = beginning.saturating_sub(1);
    }
    format!("{beginning},{length}")
}

/// `difflib.unified_diff(a, b, fromfile, tofile)` joined, with three context lines.
pub fn unified_diff(a: &[String], b: &[String], fromfile: &str, tofile: &str) -> String {
    let groups = grouped_opcodes(opcodes(a, b), 3);
    if groups.is_empty() {
        return String::new();
    }
    let mut out = format!("--- {fromfile}\n+++ {tofile}\n");
    for group in groups {
        let (first, last) = (group[0], group[group.len() - 1]);
        out.push_str(&format!(
            "@@ -{} +{} @@\n",
            format_range(first.1, last.2),
            format_range(first.3, last.4)
        ));
        for (tag, i1, i2, j1, j2) in group {
            match tag {
                Tag::Equal => a[i1..i2]
                    .iter()
                    .for_each(|l| out.push_str(&format!(" {l}"))),
                Tag::Delete => a[i1..i2]
                    .iter()
                    .for_each(|l| out.push_str(&format!("-{l}"))),
                Tag::Insert => b[j1..j2]
                    .iter()
                    .for_each(|l| out.push_str(&format!("+{l}"))),
                Tag::Replace => {
                    a[i1..i2]
                        .iter()
                        .for_each(|l| out.push_str(&format!("-{l}")));
                    b[j1..j2]
                        .iter()
                        .for_each(|l| out.push_str(&format!("+{l}")));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        py_splitlines(text, true)
    }

    #[test]
    fn unified_diff_matches_difflib_for_a_small_edit() {
        let before = lines("a\nb\nc\nd\ne\nf\ng\nh\n");
        let after = lines("a\nb\nc\nX\ne\nf\ng\nh\n");
        let expected = "--- f\n+++ f\n@@ -1,7 +1,7 @@\n a\n b\n c\n-d\n+X\n e\n f\n g\n";
        assert_eq!(unified_diff(&before, &after, "f", "f"), expected);
        assert_eq!(unified_diff(&before, &before, "f", "f"), "");
        let appended = lines("a\nb\nc\nd\ne\nf\ng\nh\ni\n");
        let expected = "--- f\n+++ f\n@@ -6,3 +6,4 @@\n f\n g\n h\n+i\n";
        assert_eq!(unified_diff(&before, &appended, "f", "f"), expected);
        let expected = "--- f\n+++ f\n@@ -0,0 +1 @@\n+x\n";
        assert_eq!(unified_diff(&[], &lines("x\n"), "f", "f"), expected);
    }

    #[test]
    fn tools_run_in_a_scratch_workspace() {
        let dir = std::env::temp_dir().join(format!("vb-engine-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let executor = LocalToolExecutor::new(&dir).expect("workspace");
        let created = executor
            .edit_file(&EditFileArgs {
                path: "hello.py".to_owned(),
                old_string: String::new(),
                new_string: "print(1)\n".to_owned(),
            })
            .expect("creates");
        assert_eq!(created, Value::Str("+ print(1)\n".to_owned()));
        let listed = executor
            .list_files(&ListFilesArgs {
                dir: ".".to_owned(),
                recursive: true,
            })
            .expect("lists");
        assert_eq!(listed, Value::from(vec!["hello.py".to_owned()]));
        let read = executor
            .read_file(&ReadFileArgs {
                path: "hello.py".to_owned(),
                start_line: Some(1),
                limit: Some(5),
            })
            .expect("reads");
        assert_eq!(read, Value::Str("print(1)\n".to_owned()));
        let missing = executor
            .read_file(&ReadFileArgs {
                path: "nope.py".to_owned(),
                start_line: None,
                limit: None,
            })
            .expect_err("missing file");
        assert!(
            missing.starts_with("[Errno 2] No such file or directory: '"),
            "{missing}"
        );
        let escape = executor
            .read_file(&ReadFileArgs {
                path: "../../etc/passwd".to_owned(),
                start_line: None,
                limit: None,
            })
            .expect_err("escape");
        assert_eq!(escape, "path escapes workspace");
        let ran = executor
            .run_command(&RunCommandArgs::new("printf out; exit 2", Some(5)))
            .expect("runs");
        assert_eq!(ran.get("exit_code"), Some(&Value::Int(2)));
        assert_eq!(ran.get("stdout"), Some(&Value::Str("out".to_owned())));
        let unknown = executor.execute(&ToolCall {
            tool: "run_build".to_owned(),
            args: Value::object(),
        });
        assert_eq!(unknown.error.as_deref(), Some("unknown tool run_build"));
        let _ = fs::remove_dir_all(&dir);
    }
}
