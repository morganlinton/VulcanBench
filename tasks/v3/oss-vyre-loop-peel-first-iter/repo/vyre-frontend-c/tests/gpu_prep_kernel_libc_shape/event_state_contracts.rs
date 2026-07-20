use std::path::PathBuf;

use super::preprocess_reference::ReferenceDispatcher;
use super::NullLoader;
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_preprocess_translation_unit, ConditionalEventKind, ConditionalEventResidency,
    IncludeEventResidency, IncludeLoader, MacroEventKind,
};

#[test]
fn gpu_preprocess_records_gpu_resident_include_request_events() {
    struct HeaderLoader;
    impl IncludeLoader for HeaderLoader {
        fn load(
            &self,
            path: &[u8],
            is_system: bool,
            is_next: bool,
            _from: &std::path::Path,
        ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
            assert_eq!(path, b"h.h");
            assert!(!is_system);
            assert!(!is_next);
            Ok(Some((
                PathBuf::from("/tmp/vyrec-gpu-include-event/h.h"),
                b"int from_header;\n".to_vec().into(),
            )))
        }
    }

    let dispatcher = ReferenceDispatcher;
    let loader = HeaderLoader;
    let path = PathBuf::from("/tmp/vyrec-gpu-include-event/main.c");
    let raw = b"#include \"h.h\"\nint from_source;\n";

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("gpu preprocess succeeds");

    assert!(
        std::str::from_utf8(&res.bytes)
            .expect("preprocessed bytes must be UTF-8")
            .contains("from_header"),
        "included header bytes must be materialized into preprocessed output"
    );
    assert_eq!(res.include_events.len(), 1);
    let event = &res.include_events[0];
    assert_eq!(event.includer, path);
    assert_eq!(event.requested_path, b"h.h");
    assert_eq!(event.directive_row, 0);
    assert_eq!(event.directive_byte_offset, 0);
    assert!(!event.is_system);
    assert!(!event.is_next);
    assert_eq!(
        event.request_residency,
        IncludeEventResidency::GpuResidentRequest
    );
    assert_eq!(
        event.resolution_residency,
        IncludeEventResidency::HostFilesystemMetadata
    );
}

#[test]
fn gpu_preprocess_records_nested_conditional_state_events() {
    let dispatcher = ReferenceDispatcher;
    let loader = NullLoader;
    let path = PathBuf::from("nested_conditionals.c");
    let raw = concat!(
        "#define A 1\n",
        "#define LOG(fmt, ...) fmt\n",
        "#ifdef A\n",
        "#if 0\n",
        "int no_a;\n",
        "#elif 1\n",
        "int yes;\n",
        "#else\n",
        "int no_b;\n",
        "#endif\n",
        "#endif\n",
    )
    .as_bytes();

    let res = gpu_preprocess_translation_unit(&dispatcher, &loader, &path, raw, &[])
        .expect("gpu preprocess succeeds");
    let out = std::str::from_utf8(&res.bytes).expect("preprocessed bytes must be UTF-8");

    assert!(
        out.contains("yes"),
        "expected active branch in output; out={out:?} events={:#?}",
        res.conditional_events
    );
    assert!(!out.contains("no_a"));
    assert!(!out.contains("no_b"));

    let kinds = res
        .conditional_events
        .iter()
        .map(|event| event.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            ConditionalEventKind::Ifdef,
            ConditionalEventKind::If,
            ConditionalEventKind::Elif,
            ConditionalEventKind::Else,
            ConditionalEventKind::Endif,
            ConditionalEventKind::Endif,
        ]
    );
    assert!(
        res.conditional_events
            .iter()
            .any(|event| event.depth_after == 2),
        "nested conditional depth must be recorded"
    );
    assert!(
        res.conditional_events
            .iter()
            .filter(|event| matches!(
                event.kind,
                ConditionalEventKind::Ifdef | ConditionalEventKind::If | ConditionalEventKind::Elif
            ))
            .all(|event| event.directive_residency
                == ConditionalEventResidency::GpuResidentDirective
                && event.state_residency == ConditionalEventResidency::GpuResidentTruth),
        "conditional directive payload and truth events must be GPU-resident"
    );
    let variadic = res
        .macro_events
        .iter()
        .find(|event| event.name == b"LOG")
        .expect("function-like variadic macro event must be recorded");
    assert_eq!(variadic.kind, MacroEventKind::Define);
    assert!(variadic.gpu_resident);
    assert!(variadic.is_function_like);
    assert!(variadic.is_variadic);
    assert_eq!(variadic.args, b"fmt, ...");
    assert_eq!(variadic.replacement, b"fmt");
    assert!(variadic.name_range.is_some());
    assert!(variadic.args_range.is_some());
    assert!(variadic.replacement_range.is_some());
    assert!(
        variadic.symbol_id.iter().any(|byte| *byte != 0),
        "stable macro symbol ID must not be all zeros"
    );
}
