use std::path::PathBuf;

use super::preprocess_reference::ReferenceDispatcher;
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_preprocess_translation_unit, IncludeAccelerationKind, IncludeLoader, MacroDef,
};

#[test]
fn gpu_preprocess_skips_repeated_pragma_once_include() {
    struct HeaderLoader;
    impl IncludeLoader for HeaderLoader {
        fn load(
            &self,
            path: &[u8],
            _is_system: bool,
            _is_next: bool,
            _from: &std::path::Path,
        ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
            assert_eq!(path, b"once.h");
            Ok(Some((
                PathBuf::from("/tmp/vyrec-once/once.h"),
                b"#pragma once\nint once_value;\n".to_vec().into(),
            )))
        }
    }

    let dispatcher = ReferenceDispatcher;
    let loader = HeaderLoader;
    let path = PathBuf::from("/tmp/vyrec-once/main.c");
    let raw = b"#include \"once.h\"\n#include \"once.h\"\n";

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("gpu preprocess succeeds");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert_eq!(out.matches("once_value").count(), 1);
    assert!(res.include_acceleration_events.iter().any(|event| {
        event.kind == IncludeAccelerationKind::PragmaOnce
            && event.path == PathBuf::from("/tmp/vyrec-once/once.h")
            && event.skipped_include
            && event.gpu_directive_derived
    }));
}

#[test]
fn gpu_preprocess_skips_repeated_classic_include_guard() {
    struct HeaderLoader;
    impl IncludeLoader for HeaderLoader {
        fn load(
            &self,
            path: &[u8],
            _is_system: bool,
            _is_next: bool,
            _from: &std::path::Path,
        ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
            assert_eq!(path, b"guard.h");
            Ok(Some((
                PathBuf::from("/tmp/vyrec-guard/guard.h"),
                b"#ifndef VYREC_GUARD_H\n#define VYREC_GUARD_H\nint guarded_value;\n#endif\n"
                    .to_vec()
                    .into(),
            )))
        }
    }

    let dispatcher = ReferenceDispatcher;
    let loader = HeaderLoader;
    let path = PathBuf::from("/tmp/vyrec-guard/main.c");
    let raw = b"#include \"guard.h\"\n#include \"guard.h\"\n";

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("gpu preprocess succeeds");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes are UTF-8");

    assert_eq!(out.matches("guarded_value").count(), 1);
    assert!(res.include_acceleration_events.iter().any(|event| {
        event.kind == IncludeAccelerationKind::IncludeGuard
            && event.path == PathBuf::from("/tmp/vyrec-guard/guard.h")
            && event.guard_macro == b"VYREC_GUARD_H"
            && event.skipped_include
            && event.gpu_directive_derived
    }));
}

#[test]
fn gpu_preprocess_reuses_header_analysis_by_path_flags_defines_and_triple() {
    struct HeaderLoader;
    impl IncludeLoader for HeaderLoader {
        fn load(
            &self,
            path: &[u8],
            _is_system: bool,
            _is_next: bool,
            _from: &std::path::Path,
        ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
            assert_eq!(path, b"reuse.h");
            Ok(Some((
                PathBuf::from("/tmp/vyrec-header-reuse/reuse.h"),
                b"#ifdef FEATURE\nint feature_enabled;\n#endif\n"
                    .to_vec()
                    .into(),
            )))
        }
    }

    let dispatcher = ReferenceDispatcher;
    let loader = HeaderLoader;
    let raw = b"#include \"reuse.h\"\n";
    let enabled = [MacroDef {
        name: b"FEATURE".to_vec().into(),
        args: Vec::new(),
        body: b"1".to_vec().into(),
        is_function_like: false,
    }];
    let disabled = [MacroDef {
        name: b"OTHER_FEATURE".to_vec().into(),
        args: Vec::new(),
        body: b"1".to_vec().into(),
        is_function_like: false,
    }];

    let first = gpu_preprocess_translation_unit(
        &dispatcher,
        &loader,
        &PathBuf::from("/tmp/vyrec-header-reuse/a.c"),
        raw,
        &enabled,
    )
    .expect("first preprocess succeeds");
    let second = gpu_preprocess_translation_unit(
        &dispatcher,
        &loader,
        &PathBuf::from("/tmp/vyrec-header-reuse/b.c"),
        raw,
        &enabled,
    )
    .expect("second preprocess succeeds");
    let third = gpu_preprocess_translation_unit(
        &dispatcher,
        &loader,
        &PathBuf::from("/tmp/vyrec-header-reuse/c.c"),
        raw,
        &disabled,
    )
    .expect("third preprocess succeeds");

    let first_store = first
        .header_reuse_events
        .iter()
        .find(|event| event.stored && !event.hit)
        .expect("first include stores header analysis");
    let second_hit = second
        .header_reuse_events
        .iter()
        .find(|event| event.hit && event.gpu_analysis_reused)
        .expect("second include reuses header analysis");

    assert_eq!(
        first_store.path,
        PathBuf::from("/tmp/vyrec-header-reuse/reuse.h")
    );
    assert_eq!(second_hit.path, first_store.path);
    assert_eq!(second_hit.defines_hash, first_store.defines_hash);
    assert_eq!(second_hit.flags_hash, first_store.flags_hash);
    assert_eq!(second_hit.target_triple, first_store.target_triple);
    assert!(
        third.header_reuse_events.iter().all(|event| !event.hit),
        "changed live defines must invalidate the header cache key"
    );
    assert!(third
        .header_reuse_events
        .iter()
        .any(|event| event.stored && event.defines_hash != first_store.defines_hash));
}
