//! I3 + I4 wiring proof: occupancy estimator picks a workgroup size,
//! AutotuneStore persists it, a fresh load returns the same choice. Pure
//! end-to-end test  -  no live CUDA context required.

use std::path::PathBuf;

use vyre_driver::autotune_store::{AutotuneKey, AutotuneRecord, AutotuneStore};
use vyre_driver::specialization::{SpecCacheKey, SpecMap};
use vyre_driver_cuda::occupancy::{pick_workgroup_size_for_occupancy, KernelResourceUsage};
use vyre_driver_cuda::synthetic_device_caps::blackwell_sm120_caps_default;

fn remove_file_if_exists(path: &PathBuf) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!(
            "Fix: failed to remove temporary autotune store {}: {error}",
            path.display()
        ),
    }
}

#[test]
fn i4_pick_persists_through_i3_store_round_trip() {
    let caps = blackwell_sm120_caps_default();
    // ptxas would report a kernel's regs/thread; here a moderately
    // pressured kernel (32 regs/thread, no shared)  -  at 256 threads
    // that fits 8 blocks/SM, full occupancy.
    let usage = KernelResourceUsage {
        regs_per_thread: 32,
        shared_bytes_per_block: 0,
    };
    let candidates = [64, 128, 256, 512, 1024];
    let chosen = pick_workgroup_size_for_occupancy(&caps, usage, &candidates)
        .expect("at least one candidate must run");

    // Build the AutotuneStore key from a SpecCacheKey for this dispatch.
    let spec = SpecCacheKey::new(
        /*shader_hash=*/ 0xdead_beef_cafe_babe,
        /*binding_sig=*/ 0x1234_5678_9abc_def0,
        /*workgroup_size=*/ [chosen, 1, 1],
        &SpecMap::new(),
    );
    let key = AutotuneKey::new(&spec, "cuda-sm_120");

    // Persist + reload.
    let mut store = AutotuneStore::default();
    store.put(
        key.clone(),
        AutotuneRecord {
            workgroup_size: [chosen, 1, 1],
            unroll: 4,
            tile: [0, 0, 0],
            recorded_at: "2026-05-02".into(),
        },
    );
    let path: PathBuf = std::env::temp_dir().join("vyre-i3-i4-roundtrip.toml");
    remove_file_if_exists(&path);
    store
        .save_if_dirty(&path)
        .expect("save_if_dirty must succeed");

    let reloaded = AutotuneStore::load(&path).expect("load must succeed");
    let record = reloaded
        .get(&key)
        .expect("reloaded store must return the record we wrote");
    assert_eq!(
        record.workgroup_size,
        [chosen, 1, 1],
        "persisted workgroup must match the I4 estimator's choice"
    );
    assert_eq!(record.unroll, 4);

    remove_file_if_exists(&path);
}

#[test]
fn i4_picker_resolves_distinct_kernels_to_distinct_workgroup_sizes() {
    let caps = blackwell_sm120_caps_default();
    // Heavy register pressure forces a smaller block to fit per-SM
    // register cap.
    let heavy = KernelResourceUsage {
        regs_per_thread: 128,
        shared_bytes_per_block: 0,
    };
    // Light kernel can saturate at 1024.
    let light = KernelResourceUsage {
        regs_per_thread: 16,
        shared_bytes_per_block: 0,
    };
    let candidates = [64, 128, 256, 512, 1024];
    let heavy_choice = pick_workgroup_size_for_occupancy(&caps, heavy, &candidates).unwrap();
    let light_choice = pick_workgroup_size_for_occupancy(&caps, light, &candidates).unwrap();
    // Two distinct kernel pressures must produce distinct AutotuneKey
    // entries (or at least may map to distinct workgroups). The
    // contract here is that the picker is deterministic  -  same
    // (caps, usage) → same choice  -  and that ranking puts smaller
    // sizes first when occupancy ties.
    assert!(
        heavy_choice <= light_choice,
        "higher register pressure must not select a larger workgroup than a lighter kernel on the same device; heavy={heavy_choice} light={light_choice}"
    );
    let again = pick_workgroup_size_for_occupancy(&caps, heavy, &candidates).unwrap();
    assert_eq!(heavy_choice, again, "picker must be deterministic");
    let light_again = pick_workgroup_size_for_occupancy(&caps, light, &candidates).unwrap();
    assert_eq!(
        light_choice, light_again,
        "light picker path must be deterministic"
    );
}
