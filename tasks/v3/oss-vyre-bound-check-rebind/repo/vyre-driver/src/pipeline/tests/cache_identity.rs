//! Integration test crate for the containing Vyre package.

use super::*;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node};

fn store_program(output_name: &'static str, value: u32) -> Program {
    Program::wrapped(
        vec![BufferDecl::output(output_name, 0, DataType::U32).with_count(1)],
        [64, 1, 1],
        vec![Node::store(output_name, Expr::u32(0), Expr::u32(value))],
    )
}

#[test]
fn normalized_program_digest_tracks_program_structure() {
    let a = store_program("out", 7);
    let b = store_program("out", 8);
    assert_ne!(
        normalized_program_cache_digest(&a),
        normalized_program_cache_digest(&b),
        "Fix: backend shader caches must miss when program semantics change."
    );
    assert_eq!(
        normalized_program_cache_digest(&a),
        normalized_program_cache_digest(&a),
        "Fix: normalized program cache digests must be deterministic."
    );
}

#[test]
fn dispatch_policy_cache_hash_tracks_codegen_policy() {
    let base = DispatchConfig {
        ulp_budget: Some(1),
        workgroup_override: Some([64, 1, 1]),
        ..Default::default()
    };
    let mut changed = base.clone();
    changed.workgroup_override = Some([128, 1, 1]);

    let mut a = blake3::Hasher::new();
    update_dispatch_policy_cache_hash(&mut a, &base);
    let mut b = blake3::Hasher::new();
    update_dispatch_policy_cache_hash(&mut b, &changed);

    assert_ne!(a.finalize(), b.finalize());
    assert_eq!(
        dispatch_policy_cache_string(&base),
        "ulp=Some(1):wg=Some([64, 1, 1])"
    );
}

#[test]
fn shared_disk_pipeline_cache_round_trips_and_shards() {
    let dir = tempfile::tempdir().unwrap();
    let cache = DiskPipelineCache::open(dir.path()).unwrap();
    let fp = PipelineDeviceFingerprint::from_parts(1, 2, "driver-a", "runtime-b");
    let key = [7_u8; 32];
    let path = cache.path_for(key, fp);
    let cache_key = fp.cache_key(key);
    let cache_key_hex = hex_encode(&cache_key);
    assert_eq!(
        path.parent().and_then(std::path::Path::file_name),
        Some(std::ffi::OsStr::new(&cache_key_hex[..2])),
        "Fix: cryptographic device fingerprinting must happen before shard path derivation."
    );
    assert!(cache.read(key, fp).unwrap().is_none());
    cache.write(key, fp, b"compiled bytes").unwrap();
    assert_eq!(
        cache.read(key, fp).unwrap().as_deref(),
        Some(b"compiled bytes".as_slice())
    );
}

#[test]
fn subgroup_reduction_offsets_derive_from_size() {
    assert_eq!(crate::subgroup::reduction_offsets(32), vec![16, 8, 4, 2, 1]);
    assert_eq!(crate::subgroup::reduction_offsets(8), vec![4, 2, 1]);
}
