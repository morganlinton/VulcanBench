//! Linux lib/math post-preprocessing token spelling parity harness.

mod support;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[allow(unused_imports)]
use vyre_driver_cuda as _;
#[allow(unused_imports)]
use vyre_driver_wgpu as _;

use support::clang_tokens::clang_preprocessed_token_facts_with_extra_args;
use vyre_libs::parsing::c::lex::tokens::TOK_EOF;
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_preprocess_translation_unit, gpu_tokenize_and_classify, BackendDispatcher, IncludeLoader,
    MacroDef,
};

struct FilesystemLoader {
    include_roots: Vec<PathBuf>,
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
        Ok(Some((resolved, bytes.into())))
    }
}

#[test]
#[ignore = "requires VYRE_LINUX_V68_ROOT pointing at Linux v6.8 source tree"]
fn linux_lib_math_preprocessed_token_spellings_match_clang() {
    let manifest: toml::Value = toml::from_str(include_str!("../parity/linux_math_v6_8.toml"))
        .expect("release parity manifest parses");
    let root = std::env::var_os("VYRE_LINUX_V68_ROOT")
        .map(PathBuf::from)
        .expect("set VYRE_LINUX_V68_ROOT to the Linux v6.8 source root");
    let mut sources = manifest["files"]["sources"]
        .as_array()
        .expect("manifest source list exists")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("source path must be string")
                .to_string()
        })
        .collect::<Vec<_>>();
    if let Ok(max_tus) = std::env::var("VYRE_LINUX_V68_MAX_TUS") {
        let max_tus = max_tus
            .parse::<usize>()
            .expect("VYRE_LINUX_V68_MAX_TUS must be a positive integer");
        assert!(max_tus > 0, "VYRE_LINUX_V68_MAX_TUS must be positive");
        sources.truncate(max_tus);
    }
    let include_roots = linux_include_roots(&root);
    let clang_args = clang_kernel_args(&include_roots);
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("GPU backend must be available");
    let dispatcher = BackendDispatcher(backend.as_ref());
    let loader = FilesystemLoader {
        include_roots: include_roots.clone(),
    };
    let kernel_macros = clang_kernel_predefined_macros();

    for source in sources {
        let path = root.join(&source);
        let clang = clang_preprocessed_token_facts_with_extra_args(&path, clang_args.iter())
            .unwrap_or_else(|error| panic!("clang token facts {}: {error}", path.display()));
        assert!(
            clang.diagnostics.is_empty(),
            "clang token dump for {} emitted diagnostics: {:?}",
            path.display(),
            clang.diagnostics
        );
        let clang_spellings = clang
            .tokens
            .iter()
            .filter(|token| token.kind != "eof")
            .map(|token| token.spelling.as_str())
            .collect::<Vec<_>>();

        let source_bytes =
            std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let mut bytes = b"#include <linux/kconfig.h>\n".to_vec();
        bytes.extend_from_slice(&source_bytes);
        let preprocessed =
            gpu_preprocess_translation_unit(&dispatcher, &loader, &path, &bytes, &kernel_macros)
                .unwrap_or_else(|error| panic!("vyre preprocess {}: {error}", path.display()));
        let classified = gpu_tokenize_and_classify(&dispatcher, &preprocessed.bytes)
            .unwrap_or_else(|error| panic!("vyre tokenize {}: {error}", path.display()));
        let vyre_spellings = vyre_token_spellings(&classified);

        assert_eq!(
            vyre_spellings.len(),
            clang_spellings.len(),
            "token spelling count mismatch for {}. First mismatch: {}. Clang window: {}. Vyre window: {}. Vyre prefix: {:?}",
            source,
            first_spelling_mismatch(&clang_spellings, &vyre_spellings),
            spelling_window(&clang_spellings, &vyre_spellings, true),
            spelling_window(&clang_spellings, &vyre_spellings, false),
            String::from_utf8_lossy(&preprocessed.bytes[..preprocessed.bytes.len().min(512)])
        );
        for (idx, (clang, vyre)) in clang_spellings
            .iter()
            .zip(vyre_spellings.iter())
            .enumerate()
        {
            assert_eq!(
                *vyre, *clang,
                "token spelling mismatch for {source} at token {idx}"
            );
        }
    }
}

fn vyre_token_spellings(
    classified: &vyre_libs::parsing::c::preprocess::gpu_pipeline::ClassifiedTokens,
) -> Vec<String> {
    let mut spellings = Vec::new();
    for idx in 0..classified.tok_types.len() {
        if classified.tok_types[idx] == TOK_EOF {
            continue;
        }
        let start = classified.tok_starts[idx] as usize;
        let len = classified.tok_lens[idx] as usize;
        let Some(end) = start.checked_add(len) else {
            panic!("vyre token range overflow at token {idx}");
        };
        let bytes = classified
            .source
            .get(start..end)
            .unwrap_or_else(|| panic!("vyre token range outside source at token {idx}"));
        spellings.push(String::from_utf8_lossy(bytes).into_owned());
    }
    spellings
}

fn first_spelling_mismatch(clang: &[&str], vyre: &[String]) -> String {
    let limit = clang.len().min(vyre.len());
    for idx in 0..limit {
        if clang[idx] != vyre[idx] {
            return format!("idx={idx} clang={:?} vyre={:?}", clang[idx], vyre[idx]);
        }
    }
    format!(
        "no overlapping mismatch; clang_len={} vyre_len={}",
        clang.len(),
        vyre.len()
    )
}

fn spelling_window(clang: &[&str], vyre: &[String], from_clang: bool) -> String {
    let limit = clang.len().min(vyre.len());
    let mismatch = (0..limit)
        .find(|&idx| clang[idx] != vyre[idx])
        .unwrap_or(limit);
    let start = mismatch.saturating_sub(12);
    let end = mismatch
        .saturating_add(13)
        .min(if from_clang { clang.len() } else { vyre.len() });
    let mut out = String::new();
    for idx in start..end {
        if idx > start {
            out.push(' ');
        }
        out.push_str(&idx.to_string());
        out.push(':');
        if idx == mismatch {
            out.push_str(">>");
        }
        if from_clang {
            out.push_str(clang[idx]);
        } else {
            out.push_str(&vyre[idx]);
        }
        if idx == mismatch {
            out.push_str("<<");
        }
    }
    out
}

fn clang_kernel_args(include_roots: &[PathBuf]) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("-D__KERNEL__"),
        OsString::from("-include"),
        OsString::from("linux/kconfig.h"),
    ];
    for include_root in include_roots {
        args.push(OsString::from("-I"));
        args.push(include_root.as_os_str().to_os_string());
    }
    args
}

fn clang_kernel_predefined_macros() -> Vec<MacroDef> {
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

fn linux_include_roots(root: &Path) -> Vec<PathBuf> {
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
        "vyre-linux-v6.8-token-asm-overlay-{}",
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
