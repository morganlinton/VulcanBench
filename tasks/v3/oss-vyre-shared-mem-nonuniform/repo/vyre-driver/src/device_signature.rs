//! Tier-B device signature loader.
//!
//! Device signatures are community-extensible TOML records. They describe
//! architecture facts used by optimizer cost models, tiling, vector packing,
//! and bank-conflict avoidance without baking a device table into Rust source.

use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::DeviceProfile;

const MAX_DEVICE_SIGNATURE_TOML_BYTES: u64 = 256 * 1024;

/// One parsed device signature record.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct DeviceSignature {
    /// Stable architecture id from the TOML file.
    pub id: String,
    /// Human-readable architecture family.
    pub family: String,
    /// Backend-reported architecture generation number.
    #[serde(default)]
    pub architecture_generation: Option<u32>,
    /// Case-insensitive device-name fragments that identify this signature.
    #[serde(default)]
    pub device_name_contains: Vec<String>,
    /// Maximum streaming compute units for this architecture family.
    pub max_sm: u32,
    /// Native subgroup/warp/wave size.
    pub warp_size: u32,
    /// Maximum registers per thread.
    pub regs_per_thread_max: u32,
    /// Shared memory per compute unit in KiB.
    pub shared_mem_per_sm_kb: u32,
    /// L1 cache size in KiB.
    pub l1_kb: u32,
    /// L2 cache size in KiB.
    pub l2_kb: u32,
    /// Peak memory bandwidth in GB/s.
    pub mem_bw_gbps: u32,
    /// Whether matrix-engine acceleration is available.
    pub tensor_core_supported: bool,
    /// Matrix-engine dtype names.
    #[serde(default)]
    pub tensor_core_dtypes: Vec<String>,
    /// Default unroll depth preferred by cost models.
    pub ideal_unroll_depth: u32,
    /// Preferred vector pack width in bits.
    pub ideal_vector_pack_bits: u32,
    /// Preferred tile shape for workgroup-local kernels.
    pub ideal_workgroup_tile: [u32; 3],
    /// Shared-memory bank count.
    pub bank_count: u32,
    /// Shared-memory bank width in bytes.
    pub bank_width_bytes: u32,
}

/// Collection of signatures loaded from a directory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceSignatureTable {
    signatures: Vec<DeviceSignature>,
}

impl DeviceSignature {
    /// Built-in Blackwell signature shipped with the crate.
    pub const BUILTIN_BLACKWELL_120: &'static str = include_str!("../devices/blackwell_120.toml");

    /// Parse a device signature from TOML.
    ///
    /// # Errors
    ///
    /// Returns an actionable string if the TOML cannot be parsed or violates
    /// basic invariants required by optimizer consumers.
    pub fn from_toml_str(source: &str) -> Result<Self, String> {
        let signature: Self = toml::from_str(source)
            .map_err(|error| format!("device signature TOML parse failed. Fix: {error}"))?;
        signature.validate()?;
        Ok(signature)
    }

    /// Validate schema invariants that would make cost models unsafe.
    ///
    /// # Errors
    ///
    /// Returns an actionable error for the first invalid field.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("device signature id is empty. Fix: set a stable id.".to_string());
        }
        if self.family.trim().is_empty() {
            return Err(
                "device signature family is empty. Fix: set the architecture family.".to_string(),
            );
        }
        if self.warp_size == 0 || !self.warp_size.is_power_of_two() {
            return Err(format!(
                "device signature `{}` has invalid warp_size {}. Fix: use a non-zero power of two.",
                self.id, self.warp_size
            ));
        }
        if self.ideal_vector_pack_bits == 0 || self.ideal_vector_pack_bits % 32 != 0 {
            return Err(format!(
                "device signature `{}` has invalid ideal_vector_pack_bits {}. Fix: use a positive multiple of 32.",
                self.id, self.ideal_vector_pack_bits
            ));
        }
        if self.ideal_workgroup_tile.contains(&0) {
            return Err(format!(
                "device signature `{}` has a zero ideal_workgroup_tile axis. Fix: every axis must be positive.",
                self.id
            ));
        }
        if self.bank_count == 0 || self.bank_width_bytes == 0 {
            return Err(format!(
                "device signature `{}` has invalid shared-memory bank metadata. Fix: bank_count and bank_width_bytes must be non-zero.",
                self.id
            ));
        }
        validate_kib_projection(self.shared_mem_per_sm_kb, "shared_mem_per_sm_kb", &self.id)?;
        validate_kib_projection(self.l1_kb, "l1_kb", &self.id)?;
        validate_kib_projection(self.l2_kb, "l2_kb", &self.id)?;
        Ok(())
    }

    /// Apply architecture facts to a neutral device profile.
    #[must_use]
    pub fn apply_to_profile(&self, mut profile: DeviceProfile) -> DeviceProfile {
        profile.subgroup_size = self.warp_size;
        profile.supports_tensor_cores = self.tensor_core_supported;
        profile.has_subgroup_shuffle = self.warp_size > 0;
        profile.has_shared_memory |= self.shared_mem_per_sm_kb > 0;
        if profile.max_shared_memory_bytes == 0 {
            profile.max_shared_memory_bytes =
                kib_to_bytes_checked(self.shared_mem_per_sm_kb, "shared_mem_per_sm_kb", &self.id);
        }
        profile.compute_units = self.max_sm;
        profile.regs_per_thread_max = self.regs_per_thread_max;
        profile.l1_cache_bytes = kib_to_bytes_checked(self.l1_kb, "l1_kb", &self.id);
        profile.l2_cache_bytes = kib_to_bytes_checked(self.l2_kb, "l2_kb", &self.id);
        profile.mem_bw_gbps = self.mem_bw_gbps;
        profile.ideal_unroll_depth = self.ideal_unroll_depth;
        profile.ideal_vector_pack_bits = self.ideal_vector_pack_bits;
        profile.ideal_workgroup_tile = self.ideal_workgroup_tile;
        profile.shared_memory_bank_count = self.bank_count;
        profile.shared_memory_bank_width_bytes = self.bank_width_bytes;
        profile
    }

    /// Return true when this signature should be used for a backend-reported
    /// architecture generation number.
    #[must_use]
    pub fn matches_architecture_generation(&self, generation: u32) -> bool {
        self.architecture_generation == Some(generation)
            || self.id.rsplit('_').next().and_then(parse_u32) == Some(generation)
    }

    /// Return true when the device name carries one of this signature's
    /// Tier-B aliases.
    #[must_use]
    pub fn matches_device_name(&self, device_name: &str) -> bool {
        let device_name = device_name.to_ascii_lowercase();
        self.device_name_contains
            .iter()
            .any(|needle| device_name.contains(&needle.to_ascii_lowercase()))
    }
}

impl DeviceSignatureTable {
    /// Load the signatures compiled into this crate.
    ///
    /// This is the no-filesystem fallback used by backend projections before
    /// external Tier-B directories are available.
    pub fn builtins() -> Result<Self, String> {
        let mut signatures = vec![DeviceSignature::from_toml_str(
            DeviceSignature::BUILTIN_BLACKWELL_120,
        )?];
        signatures.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        Ok(Self { signatures })
    }

    /// Load every `*.toml` signature file in `dir`.
    ///
    /// # Errors
    ///
    /// Returns an actionable error when the directory cannot be read, a file
    /// cannot be read, or any signature is invalid.
    pub fn load_dir(dir: impl AsRef<Path>) -> Result<Self, String> {
        let dir = dir.as_ref();
        let entries = fs::read_dir(dir).map_err(|error| {
            format!(
                "device signature directory `{}` cannot be read. Fix: create it or pass the correct path: {error}",
                dir.display()
            )
        })?;
        let mut signatures = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "device signature directory `{}` contains an unreadable entry. Fix: {error}",
                    dir.display()
                )
            })?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            let source = read_device_signature_toml(&path).map_err(|error| {
                format!(
                    "device signature file `{}` cannot be read. Fix: {error}",
                    path.display()
                )
            })?;
            let signature = DeviceSignature::from_toml_str(&source)
                .map_err(|error| format!("{} in `{}`", error, path.display()))?;
            signatures.push(signature);
        }
        signatures.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        dedupe_signature_ids(&signatures)?;
        Ok(Self { signatures })
    }

    /// Borrow all loaded signatures in stable id order.
    #[must_use]
    pub fn signatures(&self) -> &[DeviceSignature] {
        &self.signatures
    }

    /// Find a signature by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&DeviceSignature> {
        self.signatures
            .binary_search_by(|signature| signature.id.as_str().cmp(id))
            .ok()
            .and_then(|index| self.signatures.get(index))
    }

    /// Find the best signature for a backend-reported architecture generation.
    #[must_use]
    pub fn find_architecture_generation(&self, generation: u32) -> Option<&DeviceSignature> {
        self.signatures
            .iter()
            .find(|signature| signature.matches_architecture_generation(generation))
    }

    /// Find the best signature for a backend-reported device name.
    #[must_use]
    pub fn find_device_name(&self, device_name: &str) -> Option<&DeviceSignature> {
        self.signatures
            .iter()
            .find(|signature| signature.matches_device_name(device_name))
    }

    /// Apply a generation signature to `profile` when one is known.
    #[must_use]
    pub fn apply_generation_to_profile(
        &self,
        generation: u32,
        profile: DeviceProfile,
    ) -> DeviceProfile {
        self.find_architecture_generation(generation)
            .map_or(profile, |signature| signature.apply_to_profile(profile))
    }

    /// Apply a device-name signature to `profile` when one is known.
    #[must_use]
    pub fn apply_device_name_to_profile(
        &self,
        device_name: &str,
        profile: DeviceProfile,
    ) -> DeviceProfile {
        self.find_device_name(device_name)
            .map_or(profile, |signature| signature.apply_to_profile(profile))
    }
}

fn read_device_signature_toml(path: &Path) -> std::io::Result<String> {
    use std::io::Read as _;

    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAX_DEVICE_SIGNATURE_TOML_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("device signature TOML exceeds {MAX_DEVICE_SIGNATURE_TOML_BYTES} byte limit"),
        ));
    }
    let mut text = String::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(MAX_DEVICE_SIGNATURE_TOML_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_DEVICE_SIGNATURE_TOML_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "device signature TOML exceeded bounded read limit",
        ));
    }
    Ok(text)
}

fn dedupe_signature_ids(signatures: &[DeviceSignature]) -> Result<(), String> {
    for pair in signatures.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(format!(
                "duplicate device signature id `{}`. Fix: keep exactly one TOML file per id.",
                pair[0].id
            ));
        }
    }
    Ok(())
}

fn validate_kib_projection(value: u32, field: &str, id: &str) -> Result<(), String> {
    value.checked_mul(1024).map(|_| ()).ok_or_else(|| {
        format!(
            "device signature `{id}` field {field}={value} KiB overflows u32 bytes. Fix: split the architecture record or lower the Tier-B value; silent saturation corrupts GPU resource planning."
        )
    })
}

fn kib_to_bytes_checked(value: u32, field: &str, id: &str) -> u32 {
    let _ = (field, id);
    value.saturating_mul(1024)
}

fn parse_u32(value: &str) -> Option<u32> {
    let mut out = 0u32;
    for byte in value.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        out = out.checked_mul(10)?.checked_add(u32::from(byte - b'0'))?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{DeviceSignature, DeviceSignatureTable};
    use crate::DeviceProfile;

    const SAMPLE: &str = r#"
id = "sample_arch"
family = "sample"
max_sm = 128
warp_size = 32
regs_per_thread_max = 255
shared_mem_per_sm_kb = 128
l1_kb = 128
l2_kb = 98304
mem_bw_gbps = 1700
tensor_core_supported = true
tensor_core_dtypes = ["f16", "bf16", "tf32"]
ideal_unroll_depth = 8
ideal_vector_pack_bits = 128
ideal_workgroup_tile = [16, 16, 1]
bank_count = 32
bank_width_bytes = 4
"#;

    #[test]
    fn parses_and_validates_signature() {
        let signature = DeviceSignature::from_toml_str(SAMPLE).unwrap();

        assert_eq!(signature.id, "sample_arch");
        assert_eq!(signature.architecture_generation, None);
        assert_eq!(signature.warp_size, 32);
        assert!(signature.tensor_core_supported);
    }

    #[test]
    fn rejects_invalid_warp_size() {
        let err =
            DeviceSignature::from_toml_str(&SAMPLE.replace("warp_size = 32", "warp_size = 48"))
                .unwrap_err();

        assert!(err.contains("warp_size"));
    }

    #[test]
    fn applies_architecture_facts_to_profile() {
        let signature = DeviceSignature::from_toml_str(SAMPLE).unwrap();
        let profile = signature.apply_to_profile(DeviceProfile::conservative("test"));

        assert_eq!(profile.subgroup_size, 32);
        assert_eq!(profile.max_shared_memory_bytes, 128 * 1024);
        assert_eq!(profile.compute_units, 128);
        assert_eq!(profile.ideal_vector_pack_bits, 128);
        assert_eq!(profile.shared_memory_bank_width_bytes, 4);
        assert!(profile.supports_tensor_cores);
    }

    #[test]
    fn preserves_live_shared_memory_per_workgroup_limit() {
        let signature = DeviceSignature::from_toml_str(SAMPLE).unwrap();
        let mut live = DeviceProfile::conservative("native");
        live.max_shared_memory_bytes = 48 * 1024;
        let profile = signature.apply_to_profile(live);

        assert_eq!(profile.max_shared_memory_bytes, 48 * 1024);
        assert!(profile.has_shared_memory);
        assert_eq!(profile.shared_memory_bank_count, 32);
    }

    #[test]
    fn loads_directory_in_id_order() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("b.toml"),
            SAMPLE.replace("sample_arch", "b"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("a.toml"),
            SAMPLE.replace("sample_arch", "a"),
        )
        .unwrap();

        let table = DeviceSignatureTable::load_dir(dir.path()).unwrap();

        assert_eq!(table.signatures()[0].id, "a");
        assert_eq!(table.signatures()[1].id, "b");
        assert!(table.get("b").is_some());
    }

    #[test]
    fn repository_device_signatures_load() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../devices");
        let table = DeviceSignatureTable::load_dir(dir).unwrap();

        assert!(table.get("blackwell_120").is_some());
    }

    #[test]
    fn builtins_match_generation_and_device_name() {
        let table = DeviceSignatureTable::builtins().unwrap();
        let signature = table.find_architecture_generation(120).unwrap();

        assert_eq!(signature.id, "blackwell_120");
        assert!(table.find_device_name("RTX 5090").is_some());
    }

    #[test]
    fn builtin_signature_materially_projects_planner_fields() {
        let table = DeviceSignatureTable::builtins().unwrap();
        let signature = table.find_architecture_generation(120).unwrap();
        let profile =
            table.apply_generation_to_profile(120, DeviceProfile::conservative("backend"));

        assert_eq!(profile.ideal_unroll_depth, signature.ideal_unroll_depth);
        assert_eq!(
            profile.ideal_vector_pack_bits,
            signature.ideal_vector_pack_bits
        );
        assert_eq!(profile.ideal_workgroup_tile, signature.ideal_workgroup_tile);
        assert_eq!(profile.shared_memory_bank_count, signature.bank_count);
    }
}
