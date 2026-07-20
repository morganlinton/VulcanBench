use super::*;
use crate::backend::CompiledPipeline;
use crate::{OutputBuffers, Resource};

#[test]
fn compiled_pipeline_borrowed_batch_default_preserves_order() {
    #[derive(Default)]
    struct BatchDefaultPipeline {
        calls: std::sync::Mutex<Vec<Vec<u8>>>,
    }

    impl crate::backend::private::Sealed for BatchDefaultPipeline {}

    impl CompiledPipeline for BatchDefaultPipeline {
        fn id(&self) -> &str {
            "batch-default"
        }

        fn dispatch(
            &self,
            _: &[Vec<u8>],
            _: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "batch default test should use dispatch_borrowed. Fix: keep borrowed batch default zero-copy until each single dispatch.",
            ))
        }

        fn dispatch_borrowed(
            &self,
            inputs: &[&[u8]],
            _: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            let first = inputs.first().copied().unwrap_or_default().to_vec();
            self.calls.lock().unwrap().push(first.clone());
            Ok(vec![first])
        }
    }

    let pipeline = BatchDefaultPipeline::default();
    let a = [1_u8, 2];
    let b = [3_u8, 4];
    let batch_a: [&[u8]; 1] = [a.as_slice()];
    let batch_b: [&[u8]; 1] = [b.as_slice()];
    let batches: [&[&[u8]]; 2] = [&batch_a, &batch_b];

    let outputs = pipeline
        .dispatch_borrowed_batched(&batches, &DispatchConfig::default())
        .unwrap();

    assert_eq!(outputs, vec![vec![a.to_vec()], vec![b.to_vec()]]);
    assert_eq!(
        *pipeline.calls.lock().unwrap(),
        vec![a.to_vec(), b.to_vec()]
    );
}

#[test]
fn compiled_pipeline_default_into_records_dispatch_telemetry() {
    struct TelemetryPipeline;

    impl crate::backend::private::Sealed for TelemetryPipeline {}

    impl CompiledPipeline for TelemetryPipeline {
        fn id(&self) -> &str {
            "compiled-telemetry"
        }

        fn dispatch(
            &self,
            inputs: &[Vec<u8>],
            _: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Ok(inputs.to_vec())
        }
    }

    let before = crate::observability::snapshot_dispatch_telemetry();
    let pipeline = TelemetryPipeline;
    let input = [1_u8, 2, 3];
    let mut outputs = vec![Vec::with_capacity(8)];

    pipeline
        .dispatch_borrowed_into(
            &[input.as_slice()],
            &DispatchConfig::default(),
            &mut outputs,
        )
        .expect("default compiled-pipeline dispatch into must succeed");

    let after = crate::observability::snapshot_dispatch_telemetry();
    assert!(after.launches >= before.launches + 1);
    assert!(after.input_bytes >= before.input_bytes + 3);
    assert!(after.output_bytes >= before.output_bytes + 3);
    assert!(after.output_slots >= before.output_slots + 1);
    assert!(after.output_slots_reused >= before.output_slots_reused + 1);
}

#[test]
fn compiled_pipeline_borrowed_batch_into_reuses_output_slots() {
    #[derive(Default)]
    struct BatchDefaultPipeline {
        calls: std::sync::Mutex<Vec<Vec<u8>>>,
    }

    impl crate::backend::private::Sealed for BatchDefaultPipeline {}

    impl CompiledPipeline for BatchDefaultPipeline {
        fn id(&self) -> &str {
            "batch-default-into"
        }

        fn dispatch(
            &self,
            _: &[Vec<u8>],
            _: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "batch into default test should use dispatch_borrowed. Fix: keep borrowed batch default zero-copy until each single dispatch.",
            ))
        }

        fn dispatch_borrowed(
            &self,
            inputs: &[&[u8]],
            _: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            let first = inputs.first().copied().unwrap_or_default().to_vec();
            self.calls.lock().unwrap().push(first.clone());
            Ok(vec![first])
        }
    }

    let pipeline = BatchDefaultPipeline::default();
    let a = [1_u8, 2];
    let b = [3_u8, 4];
    let batch_a: [&[u8]; 1] = [a.as_slice()];
    let batch_b: [&[u8]; 1] = [b.as_slice()];
    let batches: [&[&[u8]]; 2] = [&batch_a, &batch_b];
    let mut outputs = vec![
        vec![Vec::with_capacity(8)],
        vec![Vec::with_capacity(8)],
        vec![Vec::with_capacity(8)],
    ];
    let outer_ptr = outputs.as_ptr();
    let first_inner_ptr = outputs[0].as_ptr();
    let second_inner_ptr = outputs[1].as_ptr();
    let first_slot_ptr = outputs[0][0].as_ptr();
    let second_slot_ptr = outputs[1][0].as_ptr();

    pipeline
        .dispatch_borrowed_batched_into(&batches, &DispatchConfig::default(), &mut outputs)
        .unwrap();

    assert_eq!(outputs, vec![vec![a.to_vec()], vec![b.to_vec()]]);
    assert_eq!(outputs.as_ptr(), outer_ptr);
    assert_eq!(outputs[0].as_ptr(), first_inner_ptr);
    assert_eq!(outputs[1].as_ptr(), second_inner_ptr);
    assert_eq!(outputs[0][0].as_ptr(), first_slot_ptr);
    assert_eq!(outputs[1][0].as_ptr(), second_slot_ptr);
}

#[test]
fn compiled_pipeline_persistent_handle_into_default_reuses_output_slots() {
    #[derive(Default)]
    struct PersistentDefaultPipeline {
        calls: std::sync::Mutex<Vec<Vec<u8>>>,
    }

    impl crate::backend::private::Sealed for PersistentDefaultPipeline {}

    impl CompiledPipeline for PersistentDefaultPipeline {
        fn id(&self) -> &str {
            "persistent-default-into"
        }

        fn dispatch(
            &self,
            _: &[Vec<u8>],
            _: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            Err(BackendError::new(
                "persistent into default test should use resident-handle dispatch. Fix: keep persistent batch default on the resident API.",
            ))
        }

        fn dispatch_persistent_handles(
            &self,
            inputs: &[Resource],
            _: &DispatchConfig,
        ) -> Result<OutputBuffers, BackendError> {
            let bytes = match inputs.first() {
                Some(Resource::Borrowed(bytes)) => bytes.clone(),
                Some(Resource::Resident(id)) => id.to_le_bytes().to_vec(),
                None => Vec::new(),
            };
            self.calls.lock().unwrap().push(bytes.clone());
            Ok(vec![bytes])
        }
    }

    let pipeline = PersistentDefaultPipeline::default();
    let mut outputs = vec![Vec::with_capacity(8)];
    let outer_ptr = outputs.as_ptr();
    let first_slot_ptr = outputs[0].as_ptr();

    pipeline
        .dispatch_persistent_handles_into(
            &[Resource::Borrowed(vec![9_u8, 8, 7])],
            &DispatchConfig::default(),
            &mut outputs,
        )
        .unwrap();

    assert_eq!(outputs, vec![vec![9_u8, 8, 7]]);
    assert_eq!(outputs.as_ptr(), outer_ptr);
    assert_eq!(outputs[0].as_ptr(), first_slot_ptr);
    assert_eq!(*pipeline.calls.lock().unwrap(), vec![vec![9_u8, 8, 7]]);
}
