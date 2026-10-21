#![allow(deprecated)]

use super::*;

#[test]
fn defines_prefix_inserts_lines() {
    let s = apply_cli_defines_prefix("int x;\n", &[("FOO".into(), Some("1".into()))]);
    assert!(s.starts_with("#define FOO 1\n"));
    assert!(s.contains("int x;"));
}

#[cfg(feature = "cpu-oracle")]
#[test]
fn include_expansion_inserts_file() {
    let tmp = std::env::temp_dir().join("vyre_frontend_c_tu_host_inc");
    match fs::remove_dir_all(&tmp) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!(
            "failed to clean test include directory {}: {error}",
            tmp.display()
        ),
    }
    fs::create_dir_all(&tmp).unwrap();
    let hdr = tmp.join("h.h");
    fs::write(&hdr, "//hdr\n").unwrap();
    let tu = tmp.join("t.c");
    fs::write(&tu, "").unwrap();
    let src = "#include \"h.h\"\nafter";
    let mut stack = Vec::new();
    let out = expand_local_includes(src, &tu, &[], false, None, 0, &mut stack).unwrap();
    assert!(out.contains("//hdr"));
    assert!(out.contains("after"));
}

#[test]
fn resident_prep_cache_rejects_oversized_entry() {
    let mut cache = ResidentPrepCache::new();
    insert_resident_prep_cache_with_limits(
        &mut cache,
        ResidentPrepKey {
            tu_path: PathBuf::from("oversized.c"),
            source_hash: [1; 16],
            options_hash: [2; 16],
        },
        ResidentPrepEntry {
            source: "x".repeat(17),
            deps: std::sync::Arc::from([]),
        },
        4,
        16,
    );
    assert!(cache.is_empty());
    assert_eq!(
        cache.stats().rejected_oversized,
        1,
        "oversized entries must be visible in cache telemetry"
    );
}

#[test]
fn resident_prep_cache_evicts_to_byte_budget() {
    let mut cache = ResidentPrepCache::new();
    let first = ResidentPrepEntry {
        source: "a".repeat(9),
        deps: std::sync::Arc::from([]),
    };
    let second = ResidentPrepEntry {
        source: "b".repeat(9),
        deps: std::sync::Arc::from([]),
    };
    insert_resident_prep_cache_with_limits(
        &mut cache,
        ResidentPrepKey {
            tu_path: PathBuf::from("first.c"),
            source_hash: [1; 16],
            options_hash: [1; 16],
        },
        first,
        4,
        16,
    );
    insert_resident_prep_cache_with_limits(
        &mut cache,
        ResidentPrepKey {
            tu_path: PathBuf::from("second.c"),
            source_hash: [2; 16],
            options_hash: [1; 16],
        },
        second,
        4,
        16,
    );
    assert!(resident_prep_cache_bytes(&cache) <= 16);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.stats().evictions, 1);
}

#[test]
fn resident_prep_cache_evicts_multiple_entries_to_byte_budget() {
    let mut cache = ResidentPrepCache::new();
    for idx in 0u8..4 {
        insert_resident_prep_cache_with_limits(
            &mut cache,
            ResidentPrepKey {
                tu_path: PathBuf::from(format!("old-{idx}.c")),
                source_hash: [idx; 16],
                options_hash: [0; 16],
            },
            ResidentPrepEntry {
                source: "x".repeat(4),
                deps: std::sync::Arc::from([]),
            },
            8,
            16,
        );
    }
    insert_resident_prep_cache_with_limits(
        &mut cache,
        ResidentPrepKey {
            tu_path: PathBuf::from("new.c"),
            source_hash: [9; 16],
            options_hash: [0; 16],
        },
        ResidentPrepEntry {
            source: "y".repeat(12),
            deps: std::sync::Arc::from([]),
        },
        8,
        16,
    );
    assert!(resident_prep_cache_bytes(&cache) <= 16);
    assert!(cache
        .keys()
        .any(|key| key.tu_path == PathBuf::from("new.c")));
    assert_eq!(cache.stats().evictions, 3);
}

#[test]
fn resident_prep_cache_replacement_does_not_double_count_old_entry() {
    let mut cache = ResidentPrepCache::new();
    let key = ResidentPrepKey {
        tu_path: PathBuf::from("same.c"),
        source_hash: [7; 16],
        options_hash: [0; 16],
    };
    insert_resident_prep_cache_with_limits(
        &mut cache,
        key.clone(),
        ResidentPrepEntry {
            source: "a".repeat(12),
            deps: std::sync::Arc::from([]),
        },
        4,
        16,
    );
    insert_resident_prep_cache_with_limits(
        &mut cache,
        key,
        ResidentPrepEntry {
            source: "b".repeat(12),
            deps: std::sync::Arc::from([]),
        },
        4,
        16,
    );
    assert_eq!(cache.len(), 1);
    assert_eq!(resident_prep_cache_bytes(&cache), 12);
}

#[test]
fn resident_prep_cache_zero_entry_budget_stores_nothing() {
    let mut cache = ResidentPrepCache::new();
    insert_resident_prep_cache_with_limits(
        &mut cache,
        ResidentPrepKey {
            tu_path: PathBuf::from("zero.c"),
            source_hash: [0; 16],
            options_hash: [0; 16],
        },
        ResidentPrepEntry {
            source: "x".to_string(),
            deps: std::sync::Arc::from([]),
        },
        0,
        16,
    );
    assert!(cache.is_empty());
}

#[test]
fn resident_prep_cache_evicts_least_recently_used_entry() {
    let mut cache = ResidentPrepCache::new();
    let first_key = ResidentPrepKey {
        tu_path: PathBuf::from("first.c"),
        source_hash: [1; 16],
        options_hash: [0; 16],
    };
    let second_key = ResidentPrepKey {
        tu_path: PathBuf::from("second.c"),
        source_hash: [2; 16],
        options_hash: [0; 16],
    };
    let third_key = ResidentPrepKey {
        tu_path: PathBuf::from("third.c"),
        source_hash: [3; 16],
        options_hash: [0; 16],
    };
    for key in [first_key.clone(), second_key.clone()] {
        insert_resident_prep_cache_with_limits(
            &mut cache,
            key,
            ResidentPrepEntry {
                source: "x".repeat(8),
                deps: std::sync::Arc::from([]),
            },
            2,
            16,
        );
    }
    assert!(lookup_resident_prep_cache(&mut cache, &first_key).is_some());
    insert_resident_prep_cache_with_limits(
        &mut cache,
        third_key.clone(),
        ResidentPrepEntry {
            source: "z".repeat(8),
            deps: std::sync::Arc::from([]),
        },
        2,
        16,
    );

    assert!(cache.contains_key(&first_key));
    assert!(!cache.contains_key(&second_key));
    assert!(cache.contains_key(&third_key));
    assert_eq!(cache.stats().hits, 1);
    assert_eq!(cache.stats().evictions, 1);
}

#[test]
fn resident_prep_dependency_lookup_reuses_shared_signature_storage() {
    let mut cache = ResidentPrepCache::new();
    let key = ResidentPrepKey {
        tu_path: PathBuf::from("deps.c"),
        source_hash: [4; 16],
        options_hash: [0; 16],
    };
    let deps = std::sync::Arc::from(
        vec![resident_cache::ResidentPrepDep {
            path: PathBuf::from("header.h"),
            len: 7,
            modified_ns: 11,
            change_ns: 13,
            dev: 17,
            ino: 19,
            content_hash: [23; 16],
        }]
        .into_boxed_slice(),
    );
    insert_resident_prep_cache_with_limits(
        &mut cache,
        key.clone(),
        ResidentPrepEntry {
            source: "int x;".to_string(),
            deps: std::sync::Arc::clone(&deps),
        },
        4,
        128,
    );

    let first = lookup_resident_prep_cache_deps(&mut cache, &key)
        .expect("Fix: resident prep dependency cache should hit");
    let second = lookup_resident_prep_cache_deps(&mut cache, &key)
        .expect("Fix: resident prep dependency cache should hit repeatedly");

    assert!(std::sync::Arc::ptr_eq(&deps, &first));
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(cache.stats().hits, 2);
}

#[test]
fn resident_prep_deps_reject_same_metadata_content_change() {
    let tmp = std::env::temp_dir().join("vyre_frontend_c_resident_dep_hash");
    match fs::remove_dir_all(&tmp) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!(
            "failed to clean test dependency directory {}: {error}",
            tmp.display()
        ),
    }
    fs::create_dir_all(&tmp).unwrap();
    let header = tmp.join("h.h");
    fs::write(&header, b"#define VALUE 1\n").unwrap();
    let metadata = fs::metadata(&header).unwrap();
    let modified_ns = metadata
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mut dep = resident_dep_from_metadata(
        header,
        &metadata,
        modified_ns,
        stable_hash_bytes(b"#define VALUE 2\n"),
    );
    if resident_dep_metadata_identity_available() {
        dep.change_ns = dep.change_ns.saturating_sub(1);
    }
    let valid = resident_prep_deps_valid(&[dep]).unwrap();
    assert!(
        !valid,
        "resident prep dependency validation must reject content changes that preserve len+mtime"
    );
}

#[test]
fn resident_prep_rejects_mixed_macro_transport() {
    let mut options = VyreCompileOptions::default();
    options
        .macros
        .push(("A".to_string(), Some("1".to_string())));
    options.macro_actions.push(CliMacroAction::Undef {
        name: "A".to_string(),
    });
    let err = reject_mixed_macro_transport(&options)
        .expect_err("mixed legacy and ordered macro transport must be rejected");
    assert!(
        err.contains("mixed macro transport"),
        "unexpected diagnostic: {err}"
    );
}
