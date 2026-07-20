//! Stable hashing helpers for compiled-pipeline cache identity.

use crate::backend::DispatchConfig;
use vyre_foundation::ir::Program;
use vyre_foundation::serial::wire::{append_data_type_fingerprint, append_node_list_fingerprint};
use vyre_spec::BackendId;

/// Return the normalized program digest used by backend pipeline caches.
///
/// # Errors
///
/// Returns when the program contains an IR type or node shape that cannot be
/// serialized into stable cache identity. Dispatch admission should surface the
/// error rather than panic or generate a lossy cache key.
pub fn try_normalized_program_cache_digest(program: &Program) -> Result<[u8; 32], String> {
    thread_local! {
        static SCRATCH: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(Vec::with_capacity(1024));
    }
    SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        scratch.clear();
        scratch.extend_from_slice(b"vyre-pipeline-cache-norm-v2\0wg\0");
        for axis in program.workgroup_size() {
            scratch.extend_from_slice(&axis.to_le_bytes());
        }
        scratch.extend_from_slice(b"\0op\0");
        match program.entry_op_id() {
            Some(op) => scratch.extend_from_slice(op.as_bytes()),
            None => scratch.extend_from_slice(b"<anon>"),
        }
        scratch.extend_from_slice(b"\0v\0");
        scratch.push(u8::from(program.is_structurally_validated()));
        scratch.extend_from_slice(b"\0bufs\0");
        for buffer in program.buffers().iter() {
            scratch.extend_from_slice(buffer.name().as_bytes());
            scratch.push(0);
            scratch.push(buffer.kind() as u8);
            scratch.push(buffer.access() as u8);
            append_data_type_fingerprint(&mut scratch, &buffer.element()).map_err(|message| {
                format!(
                    "failed to fingerprint pipeline-cache buffer data type `{}`: {message}. Fix: validate and normalize the Program before computing a compiled-pipeline cache key; invalid IR must not enter cache identity.",
                    buffer.name()
                )
            })?;
            scratch.push(0);
        }
        scratch.extend_from_slice(b"\0body\0");
        append_node_list_fingerprint(&mut scratch, program.entry()).map_err(|message| {
            format!(
                "failed to fingerprint pipeline-cache Program body: {message}. Fix: validate and normalize the Program before computing a compiled-pipeline cache key; invalid IR must not enter cache identity."
            )
        })?;
        Ok(*blake3::hash(&scratch).as_bytes())
    })
}

/// Return the normalized program digest used by backend pipeline caches.
#[must_use]
pub fn normalized_program_cache_digest(program: &Program) -> [u8; 32] {
    try_normalized_program_cache_digest(program).unwrap_or([0u8; 32])
}

/// Append dispatch policy fields that alter generated backend code to a cache
/// hasher.
pub fn update_dispatch_policy_cache_hash(hasher: &mut blake3::Hasher, config: &DispatchConfig) {
    hasher.update(b"ulp\0");
    match config.ulp_budget {
        Some(ulp) => {
            hasher.update(&[1, ulp]);
        }
        None => {
            hasher.update(&[0, 0]);
        }
    };
    hasher.update(b"\0wg\0");
    match config.workgroup_override {
        Some(workgroup) => {
            hasher.update(&[1]);
            for axis in workgroup {
                hasher.update(&axis.to_le_bytes());
            }
        }
        None => {
            hasher.update(&[0]);
        }
    };
}

/// Return the dispatch-policy digest used inside backend cache keys.
///
/// This keeps policy serialization single-sourced while letting backend cache
/// identities use the shared tuple-boundary-preserving key envelope instead of
/// owning a second ad hoc hasher sequence.
#[must_use]
pub fn dispatch_policy_cache_digest(config: &DispatchConfig) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    update_dispatch_policy_cache_hash(&mut hasher, config);
    *hasher.finalize().as_bytes()
}

/// Human-readable dispatch policy fingerprint for cache metadata.
#[must_use]
pub fn dispatch_policy_cache_string(config: &DispatchConfig) -> String {
    // "ulp=" (4) + max u8 decimal (3) + ":wg=" (4) + workgroup repr
    // (~32) ≈ 64 bytes worst case; pre-size so the 4 push_str calls
    // do not realloc.
    let mut policy = String::with_capacity(64);
    policy.push_str("ulp=");
    push_debug_option_u8(&mut policy, config.ulp_budget);
    policy.push_str(":wg=");
    push_debug_option_workgroup(&mut policy, config.workgroup_override);
    policy
}

/// Hex-encode bytes using lowercase ASCII.
#[must_use]
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Hex-encode the first eight bytes of a 32-byte digest for compact ids.
#[must_use]
pub fn hex_short(bytes: &[u8; 32]) -> String {
    hex_encode(&bytes[..8])
}

/// Stable device fingerprint for persistent pipeline caches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineDeviceFingerprint {
    /// Vendor identifier.
    pub vendor: u32,
    /// Device identifier.
    pub device: u32,
    /// Cryptographic digest of driver/runtime revision text.
    pub driver_digest: [u8; 32],
}

impl PipelineDeviceFingerprint {
    /// Build a fingerprint from numeric identifiers and revision text.
    #[must_use]
    pub fn from_parts(vendor: u32, device: u32, revision: &str, revision_extra: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"vyre-pipeline-device-fingerprint-v1\0");
        hasher.update(revision.as_bytes());
        hasher.update(b"\0extra\0");
        hasher.update(revision_extra.as_bytes());
        Self {
            vendor,
            device,
            driver_digest: *hasher.finalize().as_bytes(),
        }
    }

    /// Compose a cache key from canonical program digest and device identity.
    #[must_use]
    pub fn cache_key(self, program_digest: [u8; 32]) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"vyre-disk-pipeline-cache-key-v1\0program\0");
        hasher.update(&program_digest);
        hasher.update(b"\0vendor\0");
        hasher.update(&self.vendor.to_le_bytes());
        hasher.update(b"\0device\0");
        hasher.update(&self.device.to_le_bytes());
        hasher.update(b"\0driver\0");
        hasher.update(&self.driver_digest);
        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::{dispatch_policy_cache_digest, update_dispatch_policy_cache_hash};
    use crate::backend::DispatchConfig;

    #[test]
    fn dispatch_policy_cache_digest_matches_shared_hasher_for_generated_configs() {
        for case in 0..4096u32 {
            let mut config = DispatchConfig::default();
            if case & 1 != 0 {
                config.ulp_budget = Some((case as u8).wrapping_mul(17).wrapping_add(1));
            }
            if case & 2 != 0 {
                config.workgroup_override = Some([
                    1 + (case & 255),
                    1 + ((case.rotate_left(7) >> 3) & 31),
                    1 + ((case.rotate_right(5) >> 2) & 7),
                ]);
            }

            let mut hasher = blake3::Hasher::new();
            update_dispatch_policy_cache_hash(&mut hasher, &config);
            assert_eq!(
                dispatch_policy_cache_digest(&config),
                *hasher.finalize().as_bytes(),
                "Fix: dispatch-policy digest must stay single-sourced through update_dispatch_policy_cache_hash for generated case {case}."
            );
        }
    }
}

pub(super) fn push_debug_option_u8(out: &mut String, value: Option<u8>) {
    match value {
        Some(value) => {
            out.push_str("Some(");
            push_decimal_u8(out, value);
            out.push(')');
        }
        None => out.push_str("None"),
    }
}

pub(super) fn push_debug_option_workgroup(out: &mut String, value: Option<[u32; 3]>) {
    match value {
        Some([x, y, z]) => {
            out.push_str("Some([");
            push_decimal_u32(out, x);
            out.push_str(", ");
            push_decimal_u32(out, y);
            out.push_str(", ");
            push_decimal_u32(out, z);
            out.push_str("])");
        }
        None => out.push_str("None"),
    }
}

pub(super) fn push_decimal_u8(out: &mut String, value: u8) {
    push_decimal_u32(out, u32::from(value));
}

pub(super) fn push_decimal_u32(out: &mut String, value: u32) {
    let mut buf = [0_u8; 10];
    let mut n = value;
    let mut i = buf.len();
    if n == 0 {
        out.push('0');
        return;
    }
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    out.push_str(std::str::from_utf8(&buf[i..]).unwrap_or("0"));
}
