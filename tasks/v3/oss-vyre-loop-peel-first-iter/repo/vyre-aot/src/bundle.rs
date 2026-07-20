//! Submission-bundle packager.
//!
//! Takes a [`CompiledArtifact`] + uncompressed weight bytes + a launcher
//! source tree and writes the on-disk submission directory:
//!
//! ```text
//! <bundle_dir>/
//! ├── manifest.json
//! ├── kernel.<ext>.lzma         (LZMA-compressed kernel bytes)
//! ├── weights.brotli            (Brotli-11-compressed weight bytes)
//! ├── pgolf-launcher/           (Rust launcher crate source)
//! │   ├── Cargo.toml
//! │   ├── .cargo/config.toml
//! │   └── src/{main.rs,artifact.rs,...}
//! └── README.md
//! ```
//!
//! The launcher source is shipped *unbuilt* by default. Submission
//! packaging compiles it once on the target hardware (5090 / H100) and
//! ships the static binary in place of the source tree.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}
use thiserror::Error;

use crate::artifact::CompiledArtifact;
use crate::launcher::{emit_launcher_rust, LauncherError, LauncherOpts};
use crate::manifest::Manifest;

const METRIC_RECORD_WORDS: u32 = 8;

/// Produced layout of a bundle.
#[derive(Debug, Clone)]
pub struct Bundle {
    /// Files written to disk relative to bundle root, with absolute paths.
    pub files: Vec<PathBuf>,
}

/// Error variants returned by [`bundle`].
#[derive(Debug, Error)]
pub enum BundleError {
    /// I/O while writing files.
    #[error("vyre-aot bundle: i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization of the manifest failed.
    #[error("vyre-aot bundle: manifest serialization: {0}")]
    Json(#[from] serde_json::Error),

    /// LZMA compression failed.
    #[error("vyre-aot bundle: lzma error: {0}")]
    Lzma(String),

    /// Brotli compression failed.
    #[error("vyre-aot bundle: brotli error: {0}")]
    Brotli(String),

    /// Launcher generation failed.
    #[error(transparent)]
    Launcher(#[from] LauncherError),

    /// Artifact ABI cannot be represented by the emitted launcher contract.
    #[error("vyre-aot bundle: invalid artifact: {0}")]
    InvalidArtifact(String),
}

/// Write the full bundle.
///
/// `weights` is the uncompressed weight bytes (bytes the launcher will
/// upload to the `params` device buffer after Brotli decompression).
pub fn bundle(
    out_dir: &Path,
    artifact: &CompiledArtifact,
    weights: &[u8],
    artifact_name: &str,
    launcher_opts: &LauncherOpts,
    notes: &str,
) -> Result<Bundle, BundleError> {
    validate_artifact_for_bundle(artifact, weights)?;
    let launcher_tree: BTreeMap<PathBuf, String> = emit_launcher_rust(artifact, launcher_opts)?;
    fs::create_dir_all(out_dir)?;

    // 1. Compress kernel bytes via LZMA.
    let kernel_compressed = lzma_compress(&artifact.kernel_bytes)?;
    let kernel_filename = format!("kernel.{}.lzma", artifact.target.extension());
    let kernel_path = out_dir.join(&kernel_filename);
    fs::write(&kernel_path, &kernel_compressed)?;

    // 2. Compress weights via Brotli-11.
    let weights_compressed = brotli_compress(weights)?;
    let weights_filename = "weights.brotli".to_string();
    let weights_path = out_dir.join(&weights_filename);
    fs::write(&weights_path, &weights_compressed)?;

    // 3. Compute hashes of the *uncompressed* bytes for manifest.
    let kernel_sha = sha256_hex(&artifact.kernel_bytes);
    let weights_sha = sha256_hex(weights);

    // 4. Write manifest.
    let manifest = Manifest {
        schema: Manifest::SCHEMA_VERSION.to_string(),
        aot_version: artifact.aot_version.clone(),
        artifact_name: artifact_name.to_string(),
        target: artifact.target,
        entry_point: artifact.entry_point.clone(),
        dispatch: artifact.dispatch,
        kernel_file: kernel_filename.clone(),
        weights_file: weights_filename.clone(),
        kernel_compression: "lzma".to_string(),
        weights_compression: "brotli-11".to_string(),
        buffers: artifact.buffers.clone(),
        kernel_sha256_hex: kernel_sha,
        weights_sha256_hex: weights_sha,
        notes: notes.to_string(),
        vsa_fingerprint: artifact.vsa_fingerprint.clone(),
    };
    let manifest_path = out_dir.join("manifest.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    // 5. Write launcher source tree.
    let launcher_root = out_dir.join(&launcher_opts.crate_name);
    let mut written: Vec<PathBuf> = Vec::with_capacity(launcher_tree.len() + 3);
    for (rel, contents) in launcher_tree {
        let abs = launcher_root.join(rel);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&abs, contents)?;
        written.push(abs);
    }

    // 6. Write top-level README.
    let readme = format!(
        "# {artifact_name}\n\n\
        Self-contained vyre-aot bundle.\n\n\
        ## Build the launcher\n\n\
        ```\n\
        cd {crate_name}\n\
        ./cargo_full build --release\n\
        ```\n\n\
        ## Run\n\n\
        ```\n\
        {crate_name}/target/release/{crate_name} <bundle_dir>\n\
        ```\n",
        crate_name = launcher_opts.crate_name,
    );
    let readme_path = out_dir.join("README.md");
    fs::write(&readme_path, readme)?;

    written.extend([kernel_path, weights_path, manifest_path, readme_path]);

    Ok(Bundle { files: written })
}

fn validate_artifact_for_bundle(
    artifact: &CompiledArtifact,
    weights: &[u8],
) -> Result<(), BundleError> {
    if artifact.kernel_bytes.is_empty() {
        return Err(BundleError::InvalidArtifact(
            "kernel bytes are empty. Fix: compile the Program for a concrete GPU target before bundling.".to_string(),
        ));
    }
    if artifact.entry_point.is_empty() {
        return Err(BundleError::InvalidArtifact(
            "entry_point is empty. Fix: compile the artifact with a concrete visible kernel entry name.".to_string(),
        ));
    }
    if artifact.buffers.is_empty() {
        return Err(BundleError::InvalidArtifact(
            "buffer table is empty. Fix: emit at least the parameter/weight buffer required by launchers.".to_string(),
        ));
    }
    validate_dispatch_geometry(artifact)?;
    validate_buffer_table(artifact)?;
    validate_weight_payload_fits_first_finite_buffer(artifact, weights)?;
    Ok(())
}

fn validate_dispatch_geometry(artifact: &CompiledArtifact) -> Result<(), BundleError> {
    for axis in 0..3 {
        if artifact.dispatch.workgroup_size[axis] == 0 {
            return Err(BundleError::InvalidArtifact(format!(
                "workgroup_size axis {axis} is zero. Fix: derive explicit positive dispatch geometry before bundling."
            )));
        }
        if artifact.dispatch.grid_size[axis] == 0 {
            return Err(BundleError::InvalidArtifact(format!(
                "grid_size axis {axis} is zero. Fix: run vyre-aot compile() or provide explicit finite grid geometry; runtime-grid placeholders are not bundleable."
            )));
        }
    }
    checked_axis_product("workgroup_size", artifact.dispatch.workgroup_size)?;
    checked_axis_product("grid_size", artifact.dispatch.grid_size)?;
    Ok(())
}

fn checked_axis_product(label: &str, axes: [u32; 3]) -> Result<u64, BundleError> {
    u64::from(axes[0])
        .checked_mul(u64::from(axes[1]))
        .and_then(|xy| xy.checked_mul(u64::from(axes[2])))
        .ok_or_else(|| {
            BundleError::InvalidArtifact(format!(
                "{label} {axes:?} overflows u64. Fix: shard the AOT dispatch before bundling."
            ))
        })
}

fn validate_buffer_table(artifact: &CompiledArtifact) -> Result<(), BundleError> {
    let mut bindings: Vec<(u32, usize)> = Vec::with_capacity(artifact.buffers.len());
    let mut names: Vec<(&str, usize)> = Vec::with_capacity(artifact.buffers.len());
    let mut metrics_buffers = 0_usize;

    for (index, buffer) in artifact.buffers.iter().enumerate() {
        if buffer.name.is_empty() {
            return Err(BundleError::InvalidArtifact(format!(
                "buffer {index} has an empty name. Fix: emit stable buffer names before bundling."
            )));
        }
        if buffer.element_size_bytes == 0 {
            return Err(BundleError::InvalidArtifact(format!(
                "buffer {index} `{}` has element_size_bytes=0. Fix: lower the buffer to a concrete fixed-width ABI element.",
                buffer.name
            )));
        }
        let _ = checked_buffer_bytes(index, buffer)?;
        if buffer.name == "metrics" {
            metrics_buffers += 1;
            if buffer.element_size_bytes != 4 {
                return Err(BundleError::InvalidArtifact(format!(
                    "metrics buffer has element_size_bytes={} but metrics records are u32 words. Fix: emit metrics.element_size_bytes=4.",
                    buffer.element_size_bytes
                )));
            }
            if buffer.element_count < METRIC_RECORD_WORDS {
                return Err(BundleError::InvalidArtifact(format!(
                    "metrics buffer has {} word(s) but final records require at least {METRIC_RECORD_WORDS}. Fix: allocate a larger metrics ring.",
                    buffer.element_count
                )));
            }
        }
        bindings.push((buffer.binding, index));
        names.push((buffer.name.as_str(), index));
    }

    bindings.sort_unstable_by_key(|(binding, _)| *binding);
    for pair in bindings.windows(2) {
        if pair[0].0 == pair[1].0 {
            return Err(BundleError::InvalidArtifact(format!(
                "buffers {} and {} both use binding {}. Fix: emit a one-to-one CUDA argument table before bundling.",
                pair[0].1, pair[1].1, pair[0].0
            )));
        }
    }
    names.sort_unstable_by(|left, right| left.0.cmp(right.0));
    for pair in names.windows(2) {
        if pair[0].0 == pair[1].0 {
            return Err(BundleError::InvalidArtifact(format!(
                "buffers {} and {} both use name `{}`. Fix: emit unique stable buffer names before bundling.",
                pair[0].1, pair[1].1, pair[0].0
            )));
        }
    }
    if metrics_buffers > 1 {
        return Err(BundleError::InvalidArtifact(format!(
            "artifact has {metrics_buffers} metrics buffers. Fix: emit exactly one `metrics` buffer."
        )));
    }
    Ok(())
}

fn checked_buffer_bytes(
    index: usize,
    buffer: &crate::artifact::BufferEntry,
) -> Result<u64, BundleError> {
    u64::from(buffer.element_count)
        .checked_mul(u64::from(buffer.element_size_bytes))
        .ok_or_else(|| {
            BundleError::InvalidArtifact(format!(
                "buffer {index} `{}` byte size overflows u64. Fix: shard the buffer before bundling.",
                buffer.name
            ))
        })
}

fn validate_weight_payload_fits_first_finite_buffer(
    artifact: &CompiledArtifact,
    weights: &[u8],
) -> Result<(), BundleError> {
    let first = &artifact.buffers[0];
    if first.element_count == 0 {
        return Ok(());
    }
    let capacity = checked_buffer_bytes(0, first)?;
    let weight_bytes = u64::try_from(weights.len()).map_err(|error| {
        BundleError::InvalidArtifact(format!(
            "weights payload length cannot fit u64: {error}. Fix: shard the weights artifact before bundling."
        ))
    })?;
    if weight_bytes > capacity {
        return Err(BundleError::InvalidArtifact(format!(
            "weights payload has {weight_bytes} byte(s) but first buffer `{}` declares {capacity} byte(s). Fix: make buffer 0 the parameter buffer and size it to cover weights.brotli.",
            first.name
        )));
    }
    Ok(())
}

fn lzma_compress(input: &[u8]) -> Result<Vec<u8>, BundleError> {
    let mut out = Vec::with_capacity(input.len() / 2);
    let mut cursor = Cursor::new(input);
    lzma_rs::lzma_compress(&mut cursor, &mut out)
        .map_err(|e| BundleError::Lzma(format!("{e:?}")))?;
    Ok(out)
}

fn brotli_compress(input: &[u8]) -> Result<Vec<u8>, BundleError> {
    let mut out = Vec::with_capacity(input.len() / 2);
    let params = brotli::enc::BrotliEncoderParams {
        quality: 11,
        ..Default::default()
    };
    {
        let mut writer = brotli::CompressorWriter::with_params(&mut out, 4096, &params);
        writer
            .write_all(input)
            .map_err(|e| BundleError::Brotli(format!("{e}")))?;
        writer
            .flush()
            .map_err(|e| BundleError::Brotli(format!("{e}")))?;
    }
    Ok(out)
}
