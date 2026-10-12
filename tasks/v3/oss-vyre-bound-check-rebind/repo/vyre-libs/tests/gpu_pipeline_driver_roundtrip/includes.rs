use super::*;

#[test]
fn simple_include_inlines_file_contents() {
    let mut loader = MemLoader::new();
    loader.add(b"foo.h", b"int from_foo;\n");
    let out = run(b"#include \"foo.h\"\nint main_tu;", &[], &loader);
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("from_foo"));
    assert!(s.contains("main_tu"));
}

#[test]
fn repeated_unguarded_include_reuses_gpu_header_analysis() {
    let pid = std::process::id();
    let mut loader = MemLoader::new();
    let header_name = format!("repeat_header_reuse_gpu_analysis_{pid}.h");
    let header_body = format!(
        "int repeated_header_{pid}; /* force gpu preprocessing without macro mutation */\n"
    );
    loader.add(header_name.as_bytes(), header_body.as_bytes());
    let source = format!("#include \"{header_name}\"\n#include \"{header_name}\"\n");
    let dispatcher = CountingDispatcher::new();
    let out = gpu_preprocess_translation_unit(
        &dispatcher,
        &loader,
        Path::new("<tu>"),
        source.as_bytes(),
        &[],
    )
    .expect("preprocess");
    let text = String::from_utf8_lossy(&out.bytes);
    let header_symbol = format!("repeated_header_{pid}");
    assert_eq!(text.matches(&header_symbol).count(), 2);
    assert!(
        out.header_reuse_events
            .iter()
            .any(|event| event.stored && !event.hit),
        "first include must store GPU-derived header analysis"
    );
    assert!(
        out.header_reuse_events
            .iter()
            .any(|event| event.hit && event.gpu_analysis_reused),
        "second include must reuse cached GPU-derived header analysis"
    );
    assert_eq!(
        loader.loads(),
        1,
        "repeated include must reuse loaded header bytes inside one translation-unit run"
    );
    assert_eq!(out.include_byte_cache_stats.hits, 1);
    assert_eq!(out.include_byte_cache_stats.misses, 1);
    assert_eq!(out.include_byte_cache_stats.entries, 1);
    assert_eq!(out.include_byte_cache_stats.evictions, 0);
    assert!(out.include_byte_cache_stats.retained_bytes >= header_body.len() as u64);
    assert_eq!(
        out.include_byte_cache_stats.loaded_bytes,
        header_body.len() as u64
    );
    assert_eq!(
        out.include_byte_cache_stats.reused_bytes,
        header_body.len() as u64
    );
    assert!(
        out.include_events
            .iter()
            .any(|event| { event.resolution_residency == IncludeEventResidency::HostMemoryCache }),
        "second include event must expose in-run header byte-cache residency"
    );
    let reused_dispatches = dispatcher.dispatches();

    let mut distinct_loader = MemLoader::new();
    let distinct_a = format!("repeat_header_reuse_gpu_analysis_{pid}_a.h");
    let distinct_b = format!("repeat_header_reuse_gpu_analysis_{pid}_b.h");
    distinct_loader.add(distinct_a.as_bytes(), header_body.as_bytes());
    distinct_loader.add(distinct_b.as_bytes(), header_body.as_bytes());
    let distinct_source = format!("#include \"{distinct_a}\"\n#include \"{distinct_b}\"\n");
    let distinct_dispatcher = CountingDispatcher::new();
    let distinct_out = gpu_preprocess_translation_unit(
        &distinct_dispatcher,
        &distinct_loader,
        Path::new("<tu-distinct>"),
        distinct_source.as_bytes(),
        &[],
    )
    .expect("distinct header preprocess");
    assert!(
        distinct_out
            .header_reuse_events
            .iter()
            .all(|event| !event.hit),
        "distinct headers must not report header-reuse hits"
    );
    assert_eq!(
        distinct_loader.loads(),
        2,
        "distinct headers must each resolve through the include loader"
    );
    assert_eq!(distinct_out.include_byte_cache_stats.hits, 0);
    assert_eq!(distinct_out.include_byte_cache_stats.misses, 2);
    assert_eq!(distinct_out.include_byte_cache_stats.entries, 2);
    assert_eq!(distinct_out.include_byte_cache_stats.evictions, 0);
    let distinct_dispatches = distinct_dispatcher.dispatches();
    assert!(
        reused_dispatches < distinct_dispatches,
        "repeated include must reduce GPU dispatch work versus two distinct headers; reused={reused_dispatches} distinct={distinct_dispatches}"
    );
}

#[test]
fn system_include_bytes_are_shared_across_includers_in_one_translation_unit() {
    let pid = std::process::id();
    let mut loader = MemLoader::new();
    let common_name = format!("shared_system_include_{pid}.h");
    let a_name = format!("shared_system_include_{pid}_a.h");
    let b_name = format!("shared_system_include_{pid}_b.h");
    let common_body = format!("int shared_system_symbol_{pid};\n");
    let a_body = format!("#include <{common_name}>\nint from_a_{pid};\n");
    let b_body = format!("#include <{common_name}>\nint from_b_{pid};\n");
    loader
        .add(common_name.as_bytes(), common_body.as_bytes())
        .add(a_name.as_bytes(), a_body.as_bytes())
        .add(b_name.as_bytes(), b_body.as_bytes());
    let source = format!("#include \"{a_name}\"\n#include \"{b_name}\"\n");

    let out = gpu_preprocess_translation_unit(
        &CountingDispatcher::new(),
        &loader,
        Path::new("<tu>"),
        source.as_bytes(),
        &[],
    )
    .expect("preprocess shared system include");
    let text = String::from_utf8_lossy(&out.bytes);

    assert_eq!(
        text.matches(&format!("shared_system_symbol_{pid}")).count(),
        2
    );
    assert_eq!(
        loader.loads(),
        3,
        "same angle-bracket include reached from two includers must load bytes once"
    );
    assert_eq!(out.include_byte_cache_stats.hits, 1);
    assert_eq!(out.include_byte_cache_stats.misses, 3);
    assert_eq!(out.include_byte_cache_stats.entries, 3);
    assert_eq!(out.include_byte_cache_stats.evictions, 0);
    assert_eq!(
        out.include_byte_cache_stats.loaded_bytes,
        (common_body.len() + a_body.len() + b_body.len()) as u64
    );
    assert!(
        out.include_byte_cache_stats.retained_bytes >= out.include_byte_cache_stats.loaded_bytes
    );
    assert_eq!(
        out.include_byte_cache_stats.reused_bytes,
        common_body.len() as u64
    );
    assert!(
        out.include_events.iter().any(|event| {
            event.requested_path == common_name.as_bytes()
                && event.resolution_residency == IncludeEventResidency::HostMemoryCache
        }),
        "second shared angle-bracket include must be served from the TU byte cache"
    );
}

#[test]
fn missing_include_fails_loudly() {
    let loader = MemLoader::new();
    let err = gpu_preprocess_translation_unit(
        &RefDispatcher,
        &loader,
        Path::new("<tu>"),
        b"#include \"missing.h\"\nint after;\n",
        &[],
    )
    .expect_err("missing include must fail loudly");
    assert!(err.contains("missing.h"));
    assert!(err.contains("Fix:"));
}

#[test]

fn nested_includes_recurse() {
    let mut loader = MemLoader::new();
    loader
        .add(b"a.h", b"int from_a;\n#include \"b.h\"\n")
        .add(b"b.h", b"int from_b;\n");
    let out = run(b"#include \"a.h\"\n", &[], &loader);
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("from_a"));
    assert!(s.contains("from_b"));
}

#[test]
fn cycle_protection_does_not_loop_forever() {
    let mut loader = MemLoader::new();
    loader.add(b"a.h", b"int from_a;\n#include \"a.h\"\n");
    let out = run(b"#include \"a.h\"\n", &[], &loader);
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("from_a"));
}

#[test]
fn macros_accumulate_across_files() {
    let mut loader = MemLoader::new();
    loader.add(b"defs.h", b"#define X 1\n");
    let res = gpu_preprocess_translation_unit(
        &RefDispatcher,
        &loader,
        Path::new("<tu>"),
        b"#include \"defs.h\"\n",
        &[],
    )
    .expect("preprocess");
    assert!(res.macros.iter().any(|m| m.name == b"X"));
}
