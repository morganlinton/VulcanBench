use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime};

use vyre_frontend_c::api::{parse_translation_unit, VyreCompileOptions};

/// Per-file timeout. Bumped to 900s after the v2 corpus run showed
/// sign-file.c historically completes at ~565s (heavy openssl chain).
/// 900s lets sign-file pass while still bounding genuine GPU hangs.
///
/// Each file runs in a SEPARATE child process (fork of this test
/// binary, gated by the `R2_CORPUS_SINGLE_FILE` env var) so a hang
/// inside one file does not leak its GPU work to the next file. On
/// timeout the child is killed, which tears down its CUDA context
/// cleanly and frees the GPU for the next file. This avoids the
/// cascade where a single hang false-failed every subsequent file.
pub(super) const PER_FILE_TIMEOUT: Duration = Duration::from_secs(900);
pub(super) const POLL_SPIN_LIMIT: u32 = 16;
pub(super) const POLL_SLEEP_MAX: Duration = Duration::from_millis(10);
pub(super) const SINGLE_FILE_ENV: &str = "R2_CORPUS_SINGLE_FILE";

pub(super) fn chrono_like_now() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("epoch {now}s")
}
pub(super) const CORPUS_ROOT: &str = "tests/corpus/r2_kernel_scripts";
/// Skip system-include headers  -  empty after the runtime-BufLen kernel
/// refactor + gnu_attribute pre-check removal made libc-bearing TUs
/// parseable. Every file gets attempted; failures are real parser
/// feature gaps or include-path issues.
pub(super) const SKIP_SYSTEM_INCLUDE_HEADERS: &[&str] = &[];

/// Skip local-sibling-include headers  -  also empty for the same reason.
pub(super) const SKIP_LOCAL_INCLUDE_HEADERS: &[&str] = &[];

/// Build the include-dir search path the corpus test passes to
/// `parse_translation_unit`. Standard /usr/include for system headers,
/// plus every host-installed `linux-hwe-*-headers-*/scripts/` and
/// `linux-headers-*/scripts/` subtree so the kernel-scripts sibling
/// headers (`list.h`, `dialog.h`, `gendwarfksyms.h`, `xalloc.h`,
/// `images.h`, `mnconf-common.h`) resolve without needing the corpus
/// to vendor them. Each known sibling-header location is added.
pub(super) fn discover_kernel_scripts_include_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/include"),
        PathBuf::from("/usr/include/x86_64-linux-gnu"),
        // GTK-3 chain (kconfig/gconf.c uses gtk/gtk.h).
        PathBuf::from("/usr/include/gtk-3.0"),
        PathBuf::from("/usr/include/glib-2.0"),
        PathBuf::from("/usr/lib/x86_64-linux-gnu/glib-2.0/include"),
        PathBuf::from("/usr/include/pango-1.0"),
        PathBuf::from("/usr/include/harfbuzz"),
        PathBuf::from("/usr/include/freetype2"),
        PathBuf::from("/usr/include/cairo"),
        PathBuf::from("/usr/include/gdk-pixbuf-2.0"),
        PathBuf::from("/usr/include/atk-1.0"),
    ];
    append_compiler_builtin_include_dirs(&mut dirs);
    let scripts_subdirs = &[
        // Top-level kernel `include/` gives us linux/build-salt.h,
        // linux/kconfig.h, linux/kbuild.h, linux/list.h, linux/asn1_*.h.
        "include",
        "include/uapi",
        // Kernel tools/include for tools/be_byteshift.h etc.
        "tools/include",
        "tools/include/uapi",
        // scripts/ root itself so e.g. `#include "recordmcount.h"`
        // from scripts/recordmcount.c resolves.
        "scripts",
        "scripts/include",
        "scripts/kconfig",
        "scripts/kconfig/lxdialog",
        "scripts/gendwarfksyms",
        "scripts/mod",
        "scripts/basic",
        "scripts/ipe/polgen",
        "scripts/selinux/mdp",
    ];
    let kernel_root_globs = &[
        "/usr/src/linux-hwe-6.17-headers-6.17.0-19",
        "/usr/src/linux-hwe-6.17-headers-6.17.0-20",
        "/usr/src/linux-hwe-6.17-headers-6.17.0-14",
        "/usr/src/linux-headers-6.17.0-14-generic",
        "/usr/src/linux-headers-6.17.0-19-generic",
        "/usr/src/linux-headers-6.17.0-20-generic",
    ];
    for root in kernel_root_globs {
        let root_path = Path::new(root);
        if !root_path.exists() {
            continue;
        }
        for sub in scripts_subdirs {
            let candidate = root_path.join(sub);
            if candidate.exists() {
                dirs.push(candidate);
            }
        }
    }
    // Vendored stub headers (be_byteshift.h, classmap.h, …) for files
    // whose source-of-truth headers ship only in the kernel-source
    // package (linux-source-6.17), not the linux-headers package the
    // CI runner has installed. Stubs live under
    // tests/corpus/r2_kernel_scripts/vendor-headers/{tools,selinux}.
    let vendor_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/r2_kernel_scripts/vendor-headers");
    if vendor_root.exists() {
        dirs.push(vendor_root.clone());
        // selinux/ subdir gets its own entry so `#include "classmap.h"`
        // (used as a local-style include from scripts/selinux/mdp/mdp.c)
        // resolves against vendor-headers/selinux/classmap.h.
        dirs.push(vendor_root.join("selinux"));
    }
    dirs
}

pub(super) fn append_compiler_builtin_include_dirs(dirs: &mut Vec<PathBuf>) {
    append_gcc_builtin_include_dirs(dirs, Path::new("/usr/lib/gcc"));
    append_clang_builtin_include_dirs(dirs, Path::new("/usr/lib"));
}

pub(super) fn push_existing_include_dir(dirs: &mut Vec<PathBuf>, candidate: PathBuf) {
    if candidate.exists() && !dirs.iter().any(|dir| dir == &candidate) {
        dirs.push(candidate);
    }
}

pub(super) fn append_gcc_builtin_include_dirs(dirs: &mut Vec<PathBuf>, gcc_root: &Path) {
    let Ok(targets) = std::fs::read_dir(gcc_root) else {
        return;
    };
    for target in targets.flatten() {
        let target_path = target.path();
        let Ok(versions) = std::fs::read_dir(&target_path) else {
            continue;
        };
        for version in versions.flatten() {
            let include_dir = version.path().join("include");
            if include_dir.join("stdarg.h").exists() {
                push_existing_include_dir(dirs, include_dir);
            }
        }
    }
}

pub(super) fn append_clang_builtin_include_dirs(dirs: &mut Vec<PathBuf>, lib_root: &Path) {
    let Ok(entries) = std::fs::read_dir(lib_root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("llvm-") {
            continue;
        }
        let clang_root = path.join("lib/clang");
        let Ok(versions) = std::fs::read_dir(clang_root) else {
            continue;
        };
        for version in versions.flatten() {
            let include_dir = version.path().join("include");
            if include_dir.join("stdarg.h").exists() {
                push_existing_include_dir(dirs, include_dir);
            }
        }
    }
}

pub(super) fn kernel_scripts_compile_options() -> VyreCompileOptions {
    let include_dirs = discover_kernel_scripts_include_dirs();
    VyreCompileOptions {
        include_dirs: include_dirs.clone(),
        system_include_dirs: include_dirs,
        disable_system_include_dirs: true,
        ..Default::default()
    }
}

pub(super) fn collect_corpus(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<(u64, PathBuf)> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) == Some("c") {
                let size = std::fs::metadata(&path)
                    .map(|m| m.len())
                    .unwrap_or(u64::MAX);
                out.push((size, path));
            }
        }
    }
    // Sort smallest-first so the per-file progress trace exhibits the
    // pipeline behaviour on simple TUs early; large kernel-script TUs
    // (asn1_compiler, etc.) come last and self-cap via SIZE_LIMIT_BYTES.
    out.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    out.into_iter().map(|(_, path)| path).collect()
}

pub(super) fn classify_error(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if lower.contains("per-file timeout") {
        return "per-file timeout (GPU hang or absurd cold compile)".to_string();
    }
    if let Some(rest) = lower.find("system #include <") {
        let tail = &message[rest..];
        if let Some(end) = tail.find('>') {
            return format!(
                "missing system include <{}>",
                &tail[("system #include <".len())..end]
            );
        }
    }
    if lower.contains("not found (tried tu dir") {
        return "missing local include (tried tu dir + -I)".to_string();
    }
    if let Some(start) = lower.find("include `") {
        let tail = &message[start + "include `".len()..];
        if let Some(end) = tail.find('`') {
            let header = &tail[..end];
            return format!("missing include `{header}`");
        }
    }
    if lower.contains("system #include") {
        return "missing system include (other)".to_string();
    }
    if lower.contains("preprocessor") {
        return "preprocessor error".to_string();
    }
    if lower.contains("lex") || lower.contains("token") {
        return "lex / tokenization error".to_string();
    }
    if lower.contains("parse") || lower.contains("ast") {
        return "parse / AST error".to_string();
    }
    if lower.contains("sema") || lower.contains("semantic") {
        return "semantic-stage error".to_string();
    }
    if lower.contains("dispatch") || lower.contains("backend") {
        return "dispatch / backend error".to_string();
    }
    "uncategorized".to_string()
}

/// Run one file in a fresh child process and wait for it with a
/// timeout. Killing the child on timeout tears down its CUDA context
/// cleanly  -  no leaked GPU work to cascade into the next file.
pub(super) fn run_file_in_subprocess(file: &Path, timeout: Duration) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("vyre-frontend-c r2 corpus: current_exe failed: {error}"))?;
    let mut child = std::process::Command::new(&exe)
        .args([
            "--ignored",
            "--exact",
            "--nocapture",
            "r2_kernel_scripts_pass_rate",
        ])
        .env(SINGLE_FILE_ENV, file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("vyre-frontend-c r2 corpus: spawn worker failed: {error}"))?;

    let started = Instant::now();
    let pid = child.id();
    let mut empty_polls = 0u32;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                if let Some(mut s) = child.stdout.take() {
                    use std::io::Read as _;
                    let _ = s.read_to_string(&mut stdout);
                }
                let mut stderr = String::new();
                if let Some(mut s) = child.stderr.take() {
                    use std::io::Read as _;
                    let _ = s.read_to_string(&mut stderr);
                }
                if status.success() {
                    return Ok(());
                }
                if let Some(idx) = stdout.find("RESULT_ERR ") {
                    let msg = &stdout[idx + "RESULT_ERR ".len()..];
                    let line_end = msg.find('\n').unwrap_or(msg.len());
                    return Err(msg[..line_end].to_string());
                }
                let exit_code = status.code().unwrap_or(-1);
                return Err(format!(
                    "worker exited with status {exit_code}; stderr tail: {}",
                    stderr.lines().rev().take(3).collect::<Vec<_>>().join(" | ")
                ));
            }
            Ok(None) => {
                let elapsed = started.elapsed();
                if elapsed >= timeout {
                    let kill_status = child
                        .kill()
                        .map_or_else(|error| format!(" kill failed: {error}."), |_| String::new());
                    let wait_status = child
                        .wait()
                        .map_or_else(|error| format!(" wait failed: {error}."), |_| String::new());
                    return Err(format!(
                        "per-file timeout after {}s (worker pid {pid} killed)  -  likely GPU kernel fixpoint or PTX compile loop.{kill_status}{wait_status}",
                        timeout.as_secs(),
                    ));
                }
                empty_polls = empty_polls.saturating_add(1);
                if empty_polls <= POLL_SPIN_LIMIT {
                    std::thread::yield_now();
                    continue;
                }
                let remaining = timeout.saturating_sub(elapsed);
                std::thread::sleep(remaining.min(POLL_SLEEP_MAX));
            }
            Err(error) => {
                return Err(format!(
                    "vyre-frontend-c r2 corpus: try_wait failed for pid {pid}: {error}"
                ));
            }
        }
    }
}

/// Single-file worker mode: the corpus driver re-execs this very test
/// binary with `R2_CORPUS_SINGLE_FILE=<path>` set so each file runs in
/// its own CUDA context. Prints `RESULT_OK` or `RESULT_ERR <message>`
/// to stdout and exits.
pub(super) fn run_single_file_and_exit(file: &Path) -> ! {
    let options = kernel_scripts_compile_options();
    match parse_translation_unit(file, &options) {
        Ok(_) => {
            println!("RESULT_OK");
            std::process::exit(0);
        }
        Err(message) => {
            // Single line, escape newlines so the parent can grep it cleanly.
            let escaped = message.replace('\n', " | ");
            println!("RESULT_ERR {escaped}");
            std::process::exit(1);
        }
    }
}
