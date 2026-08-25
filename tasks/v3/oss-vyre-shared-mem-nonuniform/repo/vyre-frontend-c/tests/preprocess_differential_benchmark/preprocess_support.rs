use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use vyre::{DispatchConfig, VyreBackend};
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{GpuDispatcher, IncludeLoader, MacroDef};

pub(crate) struct CountingGpuDispatcher<'a> {
    backend: &'a dyn VyreBackend,
    launches: AtomicU64,
    host_write_bytes: AtomicU64,
    host_readback_bytes: AtomicU64,
    op_counts: Mutex<HashMap<String, OpGpuCounters>>,
}

impl<'a> CountingGpuDispatcher<'a> {
    pub(crate) fn new(backend: &'a dyn VyreBackend) -> Self {
        Self {
            backend,
            launches: AtomicU64::new(0),
            host_write_bytes: AtomicU64::new(0),
            host_readback_bytes: AtomicU64::new(0),
            op_counts: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn counters(&self) -> BenchmarkGpuCounters {
        BenchmarkGpuCounters {
            kernel_launch_count: self.launches.load(Ordering::Relaxed),
            host_write_bytes: self.host_write_bytes.load(Ordering::Relaxed),
            host_readback_bytes: self.host_readback_bytes.load(Ordering::Relaxed),
        }
    }

    fn record_dispatch_start(
        &self,
        program: &vyre::ir::Program,
        host_write_bytes: u64,
    ) -> Result<String, String> {
        self.launches.fetch_add(1, Ordering::Relaxed);
        self.host_write_bytes
            .fetch_add(host_write_bytes, Ordering::Relaxed);
        let op_id = program
            .entry_op_id
            .as_deref()
            .unwrap_or("<anonymous>")
            .to_string();
        let mut op_counts = self
            .op_counts
            .lock()
            .map_err(|error| format!("benchmark op counter lock poisoned: {error}"))?;
        let entry = op_counts.entry(op_id.clone()).or_default();
        entry.kernel_launch_count = entry.kernel_launch_count.saturating_add(1);
        entry.host_write_bytes = entry.host_write_bytes.saturating_add(host_write_bytes);
        Ok(op_id)
    }

    fn record_dispatch_end(&self, op_id: &str, host_readback_bytes: u64) -> Result<(), String> {
        self.host_readback_bytes
            .fetch_add(host_readback_bytes, Ordering::Relaxed);
        let mut op_counts = self
            .op_counts
            .lock()
            .map_err(|error| format!("benchmark op counter lock poisoned: {error}"))?;
        let entry = op_counts.entry(op_id.to_string()).or_default();
        entry.host_readback_bytes = entry
            .host_readback_bytes
            .saturating_add(host_readback_bytes);
        Ok(())
    }

    pub(crate) fn format_top_ops(&self, limit: usize) -> Result<String, String> {
        let mut rows = self
            .op_counts
            .lock()
            .map_err(|error| format!("benchmark op counter lock poisoned: {error}"))?
            .iter()
            .map(|(op, counts)| (op.clone(), *counts))
            .collect::<Vec<_>>();
        rows.sort_unstable_by(|left, right| {
            right
                .1
                .kernel_launch_count
                .cmp(&left.1.kernel_launch_count)
                .then_with(|| right.1.host_readback_bytes.cmp(&left.1.host_readback_bytes))
                .then_with(|| right.1.host_write_bytes.cmp(&left.1.host_write_bytes))
                .then_with(|| left.0.cmp(&right.0))
        });
        let mut out = String::from("[preprocess-op-counts]");
        for (rank, (op, counts)) in rows.into_iter().take(limit).enumerate() {
            out.push_str(&format!(
                "\nrank={} launches={} host_write={} host_readback={} op={}",
                rank + 1,
                counts.kernel_launch_count,
                counts.host_write_bytes,
                counts.host_readback_bytes,
                op
            ));
        }
        Ok(out)
    }
}

impl GpuDispatcher for CountingGpuDispatcher<'_> {
    fn dispatch(
        &self,
        program: &vyre::ir::Program,
        inputs: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>, String> {
        let op_id = self.record_dispatch_start(
            program,
            inputs.iter().map(|input| input.len() as u64).sum::<u64>(),
        )?;
        let outputs = self
            .backend
            .dispatch(program, inputs, &DispatchConfig::default())
            .map_err(|error| format!("backend dispatch: {error}"))?;
        self.record_dispatch_end(
            &op_id,
            outputs
                .iter()
                .map(|output| output.len() as u64)
                .sum::<u64>(),
        )?;
        Ok(outputs)
    }

    fn dispatch_borrowed(
        &self,
        program: &vyre::ir::Program,
        inputs: &[&[u8]],
    ) -> Result<Vec<Vec<u8>>, String> {
        let op_id = self.record_dispatch_start(
            program,
            inputs.iter().map(|input| input.len() as u64).sum::<u64>(),
        )?;
        let outputs = self
            .backend
            .dispatch_borrowed(program, inputs, &DispatchConfig::default())
            .map_err(|error| format!("backend dispatch_borrowed: {error}"))?;
        self.record_dispatch_end(
            &op_id,
            outputs
                .iter()
                .map(|output| output.len() as u64)
                .sum::<u64>(),
        )?;
        Ok(outputs)
    }
}

pub(crate) struct FilesystemLoader {
    include_roots: Vec<PathBuf>,
    loaded_include_bytes: AtomicU64,
}

impl FilesystemLoader {
    pub(crate) fn new(include_roots: Vec<PathBuf>) -> Self {
        Self {
            include_roots,
            loaded_include_bytes: AtomicU64::new(0),
        }
    }

    pub(crate) fn loaded_include_bytes(&self) -> u64 {
        self.loaded_include_bytes.load(Ordering::Relaxed)
    }
}

impl IncludeLoader for FilesystemLoader {
    fn load(
        &self,
        path: &[u8],
        is_system: bool,
        _is_next: bool,
        from: &Path,
    ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
        let name = std::str::from_utf8(path).map_err(|error| error.to_string())?;
        let local_dir = from.parent().filter(|_| !is_system);
        let resolved = local_dir
            .into_iter()
            .map(|dir| dir.join(name))
            .chain(self.include_roots.iter().map(|root| root.join(name)))
            .find(|candidate| candidate.exists())
            .ok_or_else(|| {
                format!(
                    "include {name} not found from {} in {:?}",
                    from.display(),
                    self.include_roots
                )
            })?;
        let bytes = std::fs::read(&resolved)
            .map_err(|error| format!("read include {}: {error}", resolved.display()))?;
        self.loaded_include_bytes
            .fetch_add(bytes.len() as u64, Ordering::Relaxed);
        Ok(Some((resolved, bytes.into())))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BenchmarkGpuCounters {
    pub(crate) kernel_launch_count: u64,
    pub(crate) host_write_bytes: u64,
    pub(crate) host_readback_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct OpGpuCounters {
    kernel_launch_count: u64,
    host_write_bytes: u64,
    host_readback_bytes: u64,
}

#[derive(Debug)]
pub(crate) struct DifferentialPreprocessBenchmarkReport {
    pub(crate) target_id: String,
    pub(crate) subsystem_translation_units: usize,
    pub(crate) corpus_bytes: u64,
    pub(crate) clang_wall_ns: u64,
    pub(crate) vyre_wall_ns: u64,
    pub(crate) clang_bytes_per_second: u64,
    pub(crate) vyre_bytes_per_second: u64,
    pub(crate) gpu: BenchmarkGpuCounters,
}

impl DifferentialPreprocessBenchmarkReport {
    pub(crate) fn validate(&self) {
        assert_eq!(self.target_id, "linux-lib-math-v6.8");
        assert_eq!(self.subsystem_translation_units, 12);
        assert!(self.corpus_bytes > 0);
        assert!(self.clang_wall_ns > 0);
        assert!(self.vyre_wall_ns > 0);
        assert!(self.clang_bytes_per_second > 0);
        assert!(self.vyre_bytes_per_second > 0);
        assert!(self.gpu.kernel_launch_count > 0);
        assert!(self.gpu.host_write_bytes > 0);
        assert!(self.gpu.host_readback_bytes > 0);
    }
}

pub(crate) fn clang_kernel_predefined_macros() -> Vec<MacroDef> {
    [
        ("__KERNEL__", "1"),
        ("__clang__", "1"),
        ("__clang_major__", "18"),
        ("__clang_minor__", "1"),
        ("__clang_patchlevel__", "3"),
        ("__GNUC__", "4"),
        ("__GNUC_MINOR__", "2"),
        ("__GNUC_PATCHLEVEL__", "1"),
        ("__x86_64__", "1"),
        ("__x86_64", "1"),
        ("__amd64__", "1"),
        ("__amd64", "1"),
        ("__LP64__", "1"),
        ("_LP64", "1"),
        ("__CHAR_BIT__", "8"),
        ("__SIZEOF_INT128__", "16"),
        ("__SIZEOF_LONG__", "8"),
        ("__SIZEOF_LONG_LONG__", "8"),
        ("__SIZEOF_POINTER__", "8"),
        ("__BYTE_ORDER", "__LITTLE_ENDIAN"),
        ("__LITTLE_ENDIAN", "1234"),
        ("__BIG_ENDIAN", "4321"),
        ("__LITTLE_ENDIAN_BITFIELD", "1"),
    ]
    .into_iter()
    .map(|(name, body)| MacroDef {
        name: name.as_bytes().to_vec().into(),
        args: Vec::new(),
        body: body.as_bytes().to_vec().into(),
        is_function_like: false,
    })
    .collect()
}

pub(crate) fn linux_include_roots(root: &Path) -> Vec<PathBuf> {
    let build_root = std::env::var_os("VYRE_LINUX_V68_BUILD")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            root.parent()
                .map(|parent| {
                    parent.join(format!(
                        "{}-build",
                        root.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("linux-v6.8")
                    ))
                })
                .unwrap_or_else(|| root.join("build"))
        });
    let asm_overlay = std::env::temp_dir().join(format!(
        "vyre-linux-v6.8-asm-overlay-{}",
        std::process::id()
    ));
    let asm_dir = asm_overlay.join("asm");
    std::fs::create_dir_all(&asm_dir).expect("create asm-generic overlay");
    if let Ok(entries) = std::fs::read_dir(root.join("include/asm-generic")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "h") {
                let dest = asm_dir.join(path.file_name().expect("asm-generic file name"));
                if !dest.exists() {
                    std::fs::copy(&path, &dest).unwrap_or_else(|error| {
                        panic!(
                            "copy asm-generic fallback {} to {}: {error}",
                            path.display(),
                            dest.display()
                        )
                    });
                }
            }
        }
    }
    let mut roots = vec![
        build_root.join("arch/x86/include/generated"),
        build_root.join("arch/x86/include/generated/uapi"),
        build_root.join("include"),
        build_root.join("include/generated"),
        build_root.join("include/generated/uapi"),
    ];
    roots.extend(
        [
            "arch/x86/include",
            "arch/x86/include/generated",
            "arch/x86/include/uapi",
            "arch/x86/include/generated/uapi",
            "include",
            "include/generated",
            "include/uapi",
            "include/generated/uapi",
            "tools/include",
        ]
        .into_iter()
        .map(|relative| root.join(relative)),
    );
    roots.push(asm_overlay);
    roots
}

pub(crate) fn clang_preprocess(root: &Path, include_roots: &[PathBuf], path: &Path) -> Vec<u8> {
    let mut command = clang_command();
    command
        .arg("-E")
        .arg("-P")
        .arg("-x")
        .arg("c")
        .arg("-D__KERNEL__")
        .arg("-include")
        .arg("linux/kconfig.h")
        .current_dir(root);
    for include_root in include_roots {
        command.arg("-I").arg(include_root);
    }
    let output = command
        .arg(path)
        .output()
        .unwrap_or_else(|error| panic!("spawn clang for {}: {error}", path.display()));
    assert!(
        output.status.success(),
        "clang preprocess {} failed: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

pub(crate) fn clang_command() -> Command {
    Command::new(resolve_clang_path())
}

fn resolve_clang_path() -> PathBuf {
    for var in ["VYRE_CLANG", "CLANG"] {
        if let Some(path) = std::env::var_os(var).map(PathBuf::from) {
            assert!(
                path.exists(),
                "{var} points to missing clang executable {}",
                path.display()
            );
            return path;
        }
    }
    for name in ["clang", "clang-18", "clang-17", "clang-16", "clang-15"] {
        if let Some(path) = find_executable_in_path(name) {
            return path;
        }
    }
    for path in ["/usr/bin/clang", "/usr/local/bin/clang"] {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
    }
    panic!(
        "clang executable not found. Fix: install clang or set VYRE_CLANG to the absolute clang path for the differential preprocessing benchmark."
    );
}

fn find_executable_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

pub(crate) fn format_report(report: &DifferentialPreprocessBenchmarkReport) -> String {
    format!(
        "target={} tus={} bytes={} clang_ns={} vyre_ns={} clang_Bps={} vyre_Bps={} launches={} host_write={} host_readback={}",
        report.target_id,
        report.subsystem_translation_units,
        report.corpus_bytes,
        report.clang_wall_ns,
        report.vyre_wall_ns,
        report.clang_bytes_per_second,
        report.vyre_bytes_per_second,
        report.gpu.kernel_launch_count,
        report.gpu.host_write_bytes,
        report.gpu.host_readback_bytes
    )
}

pub(crate) fn assert_required_preprocess_speedup(report: &DifferentialPreprocessBenchmarkReport) {
    let required = std::env::var("VYRE_REQUIRED_PREPROCESS_SPEEDUP")
        .ok()
        .map(|value| {
            value
                .parse::<u64>()
                .expect("VYRE_REQUIRED_PREPROCESS_SPEEDUP must be a positive integer")
        })
        .unwrap_or(100);
    assert!(
        required > 0,
        "VYRE_REQUIRED_PREPROCESS_SPEEDUP must be positive"
    );
    let required_vyre_ns = report.clang_wall_ns / required;
    assert!(
        report.vyre_wall_ns <= required_vyre_ns.max(1),
        "Vyre preprocessing did not meet the required {required}x clang speedup for {}: {}",
        report.target_id,
        format_report(report)
    );
}

pub(crate) fn bytes_per_second(bytes: u64, wall_ns: u64) -> u64 {
    ((bytes as u128 * 1_000_000_000_u128) / wall_ns.max(1) as u128) as u64
}
