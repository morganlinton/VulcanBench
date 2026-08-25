//! Backend-neutral specialization values and cache key inputs.

use std::collections::BTreeMap;

use vyre_foundation::ir::Program;
use vyre_spec::data_type::DataType;

/// One specializable scalar attribute value.
///
/// Not `Copy` because the `DType(DataType)` variant carries a
/// `vyre_spec::DataType` whose payload-bearing variants
/// (`Array { element_size }`, `Vec { .. }`, `Handle(_)`) are not
/// trivially copyable. Cloning is cheap regardless  -  the enum is
/// small and tag-discriminated.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SpecValue {
    /// Unsigned 32-bit integer.
    U32(u32),
    /// Signed 32-bit integer.
    I32(i32),
    /// 32-bit float, cache-hashed by its bit pattern.
    F32(f32),
    /// Boolean flag.
    Bool(bool),
    /// Element data type. ROADMAP F3  -  dtype-specialized kernel variants
    /// flow through the same `SpecMap` cache as tile-size and unroll
    /// choices, so the F1 specialization-cache key already separates
    /// (matmul, F32) from (matmul, F16) without any backend-specific
    /// extension.
    DType(DataType),
}

impl SpecValue {
    /// Convert to a lossless scalar form for backends whose override API
    /// accepts numeric constants through a common floating-point carrier.
    #[must_use]
    pub fn as_pipeline_f64(&self) -> f64 {
        match self {
            SpecValue::U32(value) => f64::from(*value),
            SpecValue::I32(value) => f64::from(*value),
            SpecValue::F32(value) => f64::from(*value),
            SpecValue::Bool(value) => f64::from(u8::from(*value)),
            SpecValue::DType(dtype) => f64::from(dtype_tag(dtype)),
        }
    }

    /// Hash this value into a 64-bit backend-neutral cache contribution.
    #[must_use]
    pub fn cache_hash(&self) -> u64 {
        match self {
            SpecValue::U32(value) => u64::from(*value) << 8,
            SpecValue::I32(value) => (1u64) | ((*value as u32 as u64) << 8),
            SpecValue::F32(value) => (2u64) | ((value.to_bits() as u64) << 8),
            SpecValue::Bool(value) => (3u64) | (u64::from(u8::from(*value)) << 8),
            SpecValue::DType(dtype) => (4u64) | (u64::from(dtype_tag(dtype)) << 8),
        }
    }
}

/// Stable u32 tag for each `DataType` variant. Used to seed
/// `SpecValue::DType` into the F1 cache hash deterministically.
/// Adding a new `DataType` variant must extend this table; the
/// `dtype_tag_covers_every_data_type` test enforces it.
///
/// Tags mirror the wire-format `data_type_tag` table so the cache
/// key, the on-disk artifact, and the conformance metadata all
/// agree. Parameterised variants (`Vec`, `TensorShaped`, `Array`,
/// `Handle`, `Opaque`, `Sparse*`, `DeviceMesh`) hash by their
/// outer-discriminant tag; consumers that need parameter-aware
/// keys must extend `SpecValue` rather than collapsing distinct
/// shapes here.
fn dtype_tag(dtype: &DataType) -> u32 {
    match dtype {
        DataType::U32 => 0x01,
        DataType::I32 => 0x02,
        DataType::U64 => 0x03,
        DataType::Vec2U32 => 0x04,
        DataType::Vec4U32 => 0x05,
        DataType::Bool => 0x06,
        DataType::Bytes => 0x07,
        DataType::Array { .. } => 0x08,
        DataType::F16 => 0x09,
        DataType::BF16 => 0x0A,
        DataType::F32 => 0x0B,
        DataType::F64 => 0x0C,
        DataType::Tensor => 0x0D,
        DataType::U8 => 0x0E,
        DataType::U16 => 0x0F,
        DataType::I8 => 0x10,
        DataType::I16 => 0x11,
        DataType::I64 => 0x12,
        DataType::Handle(_) => 0x13,
        DataType::Vec { .. } => 0x14,
        DataType::TensorShaped { .. } => 0x15,
        DataType::SparseCsr { .. } => 0x16,
        DataType::SparseCoo { .. } => 0x17,
        DataType::SparseBsr { .. } => 0x18,
        DataType::F8E4M3 => 0x19,
        DataType::F8E5M2 => 0x1A,
        DataType::I4 => 0x1B,
        DataType::FP4 => 0x1C,
        DataType::NF4 => 0x1D,
        DataType::DeviceMesh { .. } => 0x1E,
        DataType::Opaque(_) => 0x80,
        // Truly unknown variant  -  sentinel collision is a soundness
        // bug at the spec-cache layer (different DType values would
        // collapse onto one cache key and serve the wrong shader),
        // so any future variant MUST get an explicit tag here.
        _ => 0xFFFF_FFFF,
    }
}

/// Ordered specialization map.
#[derive(Debug, Default, Clone)]
pub struct SpecMap {
    entries: BTreeMap<String, SpecValue>,
}

impl SpecMap {
    /// Empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a `(name, value)` pair.
    pub fn insert(&mut self, name: impl Into<String>, value: SpecValue) {
        self.entries.insert(name.into(), value);
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate `(name, value)` pairs in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &SpecValue)> {
        self.entries
            .iter()
            .map(|(key, value)| (key.as_str(), value))
    }

    /// Convert to a deterministic numeric constant map.
    #[must_use]
    pub fn to_numeric_constants(&self) -> std::collections::HashMap<String, f64> {
        let mut out = std::collections::HashMap::with_capacity(self.entries.len());
        for (key, value) in &self.entries {
            out.insert(key.clone(), value.as_pipeline_f64());
        }
        out
    }

    /// Compute this map's 64-bit cache contribution.
    #[must_use]
    pub fn cache_hash(&self) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for (name, value) in self.iter() {
            for byte in name.as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            for byte in value.cache_hash().to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        hash
    }
}

/// Cache key extending a backend pipeline identity with specialization values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpecCacheKey {
    /// Hash of the shader or target module.
    pub shader_hash: u64,
    /// Stable signature of the binding layout.
    pub binding_sig: u64,
    /// Workgroup size in the dispatch.
    pub workgroup_size: [u32; 3],
    /// Hash of specialization values.
    pub spec_hash: u64,
}

impl SpecCacheKey {
    /// Fold a [`SpecMap`] into a cache key.
    #[must_use]
    pub fn new(
        shader_hash: u64,
        binding_sig: u64,
        workgroup_size: [u32; 3],
        specs: &SpecMap,
    ) -> Self {
        Self {
            shader_hash,
            binding_sig,
            workgroup_size,
            spec_hash: specs.cache_hash(),
        }
    }
}

/// Build the backend-neutral VSA specialization key used by shader caches.
///
/// The high half is the low 64 bits of the VSA fingerprint; the low half is
/// the specialization hash. Keeping this in `vyre-driver` prevents concrete
/// backends from each reimplementing the same identity folding.
#[must_use]
pub fn vsa_specialization_key(program: &Program, spec_hash: u64) -> u128 {
    let fingerprint = crate::launch::program_vsa_fingerprint_words(program);
    let fp_lo = fingerprint
        .iter()
        .take(2)
        .enumerate()
        .fold(0_u64, |acc, (i, &word)| {
            acc | (u64::from(word) << (32 * (i as u32)))
        });
    ((fp_lo as u128) << 64) | u128::from(spec_hash)
}

/// Deterministic hex key for a backend specialization artifact.
///
/// Concrete backends use this for AOT artifacts whose identity is
/// `(cache-version, specialization hash, backend fingerprint)`. Keeping the
/// length-delimited hash format here prevents each backend from inventing a
/// subtly different concatenation scheme.
#[must_use]
pub fn versioned_specialization_artifact_key(
    cache_version: u32,
    spec_hash: &str,
    backend_fingerprint: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-specialization-artifact-key-v1\0version\0");
    hasher.update(&cache_version.to_le_bytes());
    hasher.update(b"\0spec\0");
    hasher.update(&(spec_hash.len() as u64).to_le_bytes());
    hasher.update(spec_hash.as_bytes());
    hasher.update(b"\0backend\0");
    hasher.update(&(backend_fingerprint.len() as u64).to_le_bytes());
    hasher.update(backend_fingerprint.as_bytes());
    let hash = hasher.finalize();
    let mut key = String::with_capacity(64);
    push_lower_hex(hash.as_bytes(), &mut key);
    key
}

fn push_lower_hex(bytes: &[u8], out: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let additional = bytes.len().saturating_mul(2);
    let _ = out.try_reserve(additional);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

    #[test]
    fn spec_map_ordering_is_commutative() {
        let mut a = SpecMap::new();
        a.insert("A", SpecValue::U32(1));
        a.insert("B", SpecValue::U32(2));
        let mut b = SpecMap::new();
        b.insert("B", SpecValue::U32(2));
        b.insert("A", SpecValue::U32(1));
        assert_eq!(a.cache_hash(), b.cache_hash());
    }

    #[test]
    fn cache_key_differs_by_spec_hash() {
        let mut a = SpecMap::new();
        a.insert("K", SpecValue::U32(1));
        let mut b = SpecMap::new();
        b.insert("K", SpecValue::U32(2));
        assert_ne!(
            SpecCacheKey::new(0xdead, 0xbeef, [64, 1, 1], &a),
            SpecCacheKey::new(0xdead, 0xbeef, [64, 1, 1], &b)
        );
    }

    #[test]
    fn vsa_specialization_key_changes_only_low_half_for_spec_hash() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
        );
        let a = vsa_specialization_key(&program, 0x11);
        let b = vsa_specialization_key(&program, 0x22);
        assert_eq!(
            a >> 64,
            b >> 64,
            "Fix: VSA specialization keys must keep program identity independent from specialization values."
        );
        assert_ne!(
            a as u64, b as u64,
            "Fix: VSA specialization keys must include the specialization hash."
        );
    }

    #[test]
    fn versioned_artifact_key_separates_variable_length_fields() {
        let a = versioned_specialization_artifact_key(1, "ab", "cd");
        let b = versioned_specialization_artifact_key(1, "abc", "d");
        assert_ne!(
            a, b,
            "Fix: specialization artifact keys must length-prefix variable fields."
        );
    }

    // ---------------- F3 dtype-spec ----------------

    #[test]
    fn dtype_spec_value_round_trips() {
        let v = SpecValue::DType(DataType::F32);
        assert!(matches!(v, SpecValue::DType(DataType::F32)));
    }

    #[test]
    fn dtype_spec_distinct_dtypes_hash_distinct() {
        let f32_hash = SpecValue::DType(DataType::F32).cache_hash();
        let u32_hash = SpecValue::DType(DataType::U32).cache_hash();
        let i32_hash = SpecValue::DType(DataType::I32).cache_hash();
        assert_ne!(f32_hash, u32_hash);
        assert_ne!(u32_hash, i32_hash);
        assert_ne!(f32_hash, i32_hash);
    }

    #[test]
    fn dtype_spec_equal_dtypes_hash_equal() {
        assert_eq!(
            SpecValue::DType(DataType::F32).cache_hash(),
            SpecValue::DType(DataType::F32).cache_hash()
        );
    }

    #[test]
    fn dtype_spec_does_not_collide_with_other_variants() {
        // The variant tag (low byte) of DType is 4. U32(0).cache_hash() is
        // 0 << 8 = 0; the DType hash carries tag 4 in the low byte plus
        // the dtype tag in the next 32 bits, so they cannot collide.
        let dtype_hash = SpecValue::DType(DataType::U32).cache_hash();
        let u32_hash = SpecValue::U32(0).cache_hash();
        let i32_hash = SpecValue::I32(0).cache_hash();
        let f32_hash = SpecValue::F32(0.0).cache_hash();
        let bool_hash = SpecValue::Bool(false).cache_hash();
        assert_ne!(dtype_hash, u32_hash);
        assert_ne!(dtype_hash, i32_hash);
        assert_ne!(dtype_hash, f32_hash);
        assert_ne!(dtype_hash, bool_hash);
    }

    #[test]
    fn dtype_spec_separates_cache_key_in_specmap() {
        let mut a = SpecMap::new();
        a.insert("dtype", SpecValue::DType(DataType::F32));
        let mut b = SpecMap::new();
        b.insert("dtype", SpecValue::DType(DataType::U32));
        assert_ne!(
            a.cache_hash(),
            b.cache_hash(),
            "Fix: dtype-keyed SpecMaps must produce distinct cache hashes."
        );
        assert_ne!(
            SpecCacheKey::new(0, 0, [1, 1, 1], &a),
            SpecCacheKey::new(0, 0, [1, 1, 1], &b)
        );
    }

    #[test]
    fn dtype_tag_covers_every_data_type() {
        // Soundness gate: any new DataType variant must extend dtype_tag
        // explicitly. Every shipped variant returns a unique non-fallback
        // (≠ 0xFFFF_FFFF) tag.
        let known = [
            DataType::U32,
            DataType::I32,
            DataType::U64,
            DataType::Vec2U32,
            DataType::Vec4U32,
            DataType::Bool,
            DataType::Bytes,
            DataType::Array { element_size: 1 },
            DataType::F16,
            DataType::BF16,
            DataType::F32,
            DataType::F64,
            DataType::Tensor,
            DataType::U8,
            DataType::U16,
            DataType::I8,
            DataType::I16,
            DataType::I64,
            DataType::Handle(vyre_spec::data_type::TypeId(0)),
            DataType::Vec {
                element: Box::new(DataType::U32),
                count: 1,
            },
            DataType::TensorShaped {
                element: Box::new(DataType::U32),
                shape: smallvec::smallvec![1],
            },
            DataType::SparseCsr {
                element: Box::new(DataType::U32),
            },
            DataType::SparseCoo {
                element: Box::new(DataType::U32),
            },
            DataType::SparseBsr {
                element: Box::new(DataType::U32),
                block_rows: 1,
                block_cols: 1,
            },
            DataType::F8E4M3,
            DataType::F8E5M2,
            DataType::I4,
            DataType::FP4,
            DataType::NF4,
            DataType::DeviceMesh {
                axes: smallvec::smallvec![1],
            },
        ];
        let mut tags = std::collections::BTreeSet::new();
        for dtype in known {
            let tag = dtype_tag(&dtype);
            assert_ne!(
                tag, 0xFFFF_FFFF,
                "Fix: dtype_tag missing arm for {dtype:?}  -  extend specialization.rs::dtype_tag."
            );
            assert!(
                tags.insert(tag),
                "Fix: dtype_tag returned duplicate tag {tag} for {dtype:?}."
            );
        }
    }
}
