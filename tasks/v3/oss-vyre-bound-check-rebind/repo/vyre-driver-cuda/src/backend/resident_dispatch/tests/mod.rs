use super::helpers::{
    borrow_resident_sequence_output_slots, prepare_resident_sequence_fills,
    stage_resident_fill_payload, validate_dense_resident_input_indices,
    validate_dense_resident_output_indices,
};

fn resident_dispatch_production_source() -> String {
    [
        include_str!("../helpers.rs"),
        include_str!("../borrowed.rs"),
        include_str!("../async_dispatch.rs"),
        include_str!("../batch.rs"),
        include_str!("../sync.rs"),
        include_str!("../sequence_api.rs"),
        include_str!("../sequence_fused.rs"),
        include_str!("../timed.rs"),
    ]
    .iter()
    .map(|s| s.split("#[cfg(test)]").next().unwrap_or(""))
    .collect::<Vec<_>>()
    .join("\n")
}

#[cfg(test)]
mod tests {
    #[path = "source_contracts.rs"]
    mod source_contracts;

    use super::super::async_dispatch::resident_output_clear_for_readback;
    use super::super::borrowed::order_resident_fallback_inputs_by_logical_index;
    use super::{
        borrow_resident_sequence_output_slots, prepare_resident_sequence_fills,
        stage_resident_fill_payload, validate_dense_resident_input_indices,
        validate_dense_resident_output_indices,
    };
    use crate::backend::output_range::CudaOutputReadback;
    use crate::backend::resident::CudaResidentBuffer;

    #[test]
    fn resident_fallback_fill_payload_preserves_last_good_bytes_when_reservation_fails() {
        let mut payload = vec![0xC3, 0xC3, 0x7E, 0x11];

        let result = stage_resident_fill_payload(&mut payload, 0x5A, usize::MAX);

        assert!(
            result.is_err(),
            "oversized CUDA resident fill payload must fail preflight instead of mutating staging"
        );
        assert_eq!(
            payload,
            vec![0xC3, 0xC3, 0x7E, 0x11],
            "failed CUDA resident fill staging must preserve the last diagnostic payload"
        );
    }
    #[test]
    fn resident_fallback_fill_payload_reuses_capacity_and_overwrites_values() {
        let mut payload = Vec::new();
        {
            let bytes = stage_resident_fill_payload(&mut payload, 0xA5, 16)
                .expect("Fix: reusable resident fallback fill staging should reserve bytes");
            assert_eq!(bytes, &[0xA5; 16]);
        }
        let initial_capacity = payload.capacity();

        {
            let bytes = stage_resident_fill_payload(&mut payload, 0x5A, 8)
                .expect("Fix: smaller resident fallback fill staging should reuse capacity");
            assert_eq!(bytes, &[0x5A; 8]);
        }
        assert_eq!(
            payload.capacity(),
            initial_capacity,
            "CUDA resident fallback fill staging must reuse capacity across fills instead of allocating one Vec per fill"
        );

        {
            let bytes = stage_resident_fill_payload(&mut payload, 0x11, 0)
                .expect("Fix: zero-byte resident fallback fill staging should be valid");
            assert!(bytes.is_empty());
        }
        assert_eq!(
            payload.capacity(),
            initial_capacity,
            "zero-byte fallback fills must not release reusable staging capacity"
        );
    }

    #[test]
    fn resident_output_clear_uses_observable_readback_range() {
        let clear = resident_output_clear_for_readback(
            0x1000,
            CudaOutputReadback {
                device_offset: 128,
                byte_len: 4096,
            },
            "out",
        )
        .expect("Fix: ranged resident output clear planning must accept valid offsets.");

        assert_eq!(
            clear,
            Some((0x1080, 4096)),
            "Fix: resident dispatch must clear the declared output byte range, not the padded allocation."
        );

        let full = resident_output_clear_for_readback(
            0x2000,
            CudaOutputReadback {
                device_offset: 0,
                byte_len: 8192,
            },
            "full",
        )
        .expect("Fix: full resident output clear planning must preserve full-buffer clears.");
        assert_eq!(full, Some((0x2000, 8192)));
    }

    #[test]
    fn resident_output_clear_skips_zero_byte_ranges_and_rejects_pointer_overflow() {
        let skipped = resident_output_clear_for_readback(
            0x1000,
            CudaOutputReadback {
                device_offset: 256,
                byte_len: 0,
            },
            "empty",
        )
        .expect("Fix: zero-byte resident output ranges should not enqueue memsets.");
        assert_eq!(skipped, None);

        let error = resident_output_clear_for_readback(
            u64::MAX,
            CudaOutputReadback {
                device_offset: 1,
                byte_len: 4,
            },
            "overflow",
        )
        .expect_err("Fix: resident output clear planning must reject device-pointer overflow.");

        assert!(
            error.to_string().contains("overflowed"),
            "overflow error must explain the CUDA resident clear pointer failure: {error}"
        );
    }

    #[test]
    fn resident_output_index_validation_rejects_sparse_or_duplicate_sorted_indexes() {
        validate_dense_resident_output_indices([0, 1, 2], 3, "test output")
            .expect("Fix: dense resident output indexes must validate.");
        let duplicate =
            validate_dense_resident_output_indices([0, 0, 2], 3, "test output").unwrap_err();
        assert_eq!(
            duplicate.to_string().contains("duplicate"),
            true,
            "Fix: duplicate resident output indexes must fail before readback ordering can alias an output slot: {duplicate}"
        );
        let sparse =
            validate_dense_resident_output_indices([0, 2, 3], 3, "test output").unwrap_err();
        assert_eq!(
            sparse.to_string().contains("dense"),
            true,
            "Fix: sparse resident output indexes must fail before readback ordering can skip an output slot: {sparse}"
        );
        let truncated =
            validate_dense_resident_output_indices([0, 1], 3, "test output").unwrap_err();
        assert_eq!(
            truncated.to_string().contains("expected 3"),
            true,
            "Fix: truncated resident output indexes must fail before readback ordering can drop an output slot: {truncated}"
        );
    }

    #[test]
    fn resident_input_index_validation_rejects_sparse_duplicate_or_truncated_indexes() {
        validate_dense_resident_input_indices([0, 1, 2], 3, "test input")
            .expect("Fix: dense resident input indexes must validate.");
        let duplicate =
            validate_dense_resident_input_indices([0, 0, 2], 3, "test input").unwrap_err();
        assert_eq!(
            duplicate.to_string().contains("duplicate"),
            true,
            "Fix: duplicate resident input indexes must fail before borrowed fallback can alias a logical input slot: {duplicate}"
        );
        let sparse =
            validate_dense_resident_input_indices([0, 2, 3], 3, "test input").unwrap_err();
        assert_eq!(
            sparse.to_string().contains("dense"),
            true,
            "Fix: sparse resident input indexes must fail before borrowed fallback can skip a logical input slot: {sparse}"
        );
        let truncated =
            validate_dense_resident_input_indices([0, 1], 3, "test input").unwrap_err();
        assert_eq!(
            truncated.to_string().contains("expected 3"),
            true,
            "Fix: truncated resident input indexes must fail before borrowed fallback can drop a logical input slot: {truncated}"
        );
    }

    #[test]
    fn resident_borrowed_fallback_orders_downloaded_inputs_by_logical_slot() {
        let mut inputs = vec![(2, vec![0xCC]), (0, vec![0xAA]), (1, vec![0xBB])];

        order_resident_fallback_inputs_by_logical_index(&mut inputs, 3)
            .expect("Fix: reordered resident fallback inputs should sort by logical input slot.");

        assert_eq!(
            inputs,
            vec![
                (0, vec![0xAA]),
                (1, vec![0xBB]),
                (2, vec![0xCC]),
            ],
            "Fix: CUDA resident borrowed fallback must pass dispatch_borrowed inputs in Program::buffers logical order, not descriptor binding order."
        );

        let mut duplicate = vec![(0, vec![1]), (0, vec![2])];
        assert!(
            order_resident_fallback_inputs_by_logical_index(&mut duplicate, 2).is_err(),
            "Fix: resident fallback input ordering must reject duplicate logical input slots before launch."
        );
    }

    #[test]
    fn resident_sequence_fills_coalesce_duplicates_and_skip_full_upload_overwrites() {
        let first = CudaResidentBuffer {
            id: 1,
            byte_len: 16,
        };
        let second = CudaResidentBuffer {
            id: 2,
            byte_len: 16,
        };
        let upload = [0xFE_u8; 16];

        let effective = prepare_resident_sequence_fills(
            &[(first, 0x11), (second, 0x22), (first, 0x33)],
            &[(second, upload.as_slice())],
        )
        .expect("Fix: generated resident sequence fill coalescing must succeed.");

        assert_eq!(
            effective.as_slice(),
            &[(first, 0x33)],
            "Fix: resident sequence fills must keep the last duplicate fill and drop fills fully overwritten by same-sequence uploads."
        );
    }

    #[test]
    fn resident_sequence_fills_handle_dense_duplicate_streams_without_changing_order() {
        let handles: Vec<_> = (0..256)
            .map(|id| CudaResidentBuffer { id, byte_len: 1 })
            .collect();
        let mut fills = Vec::new();
        for round in 0..8_u8 {
            fills.extend(handles.iter().copied().map(|handle| (handle, round)));
        }

        let upload = [0xAA_u8];
        let uploads: Vec<_> = handles
            .iter()
            .copied()
            .filter(|handle| handle.id % 2 == 0)
            .map(|handle| (handle, upload.as_slice()))
            .collect();

        let effective = prepare_resident_sequence_fills(&fills, &uploads)
            .expect("Fix: dense CUDA resident fill coalescing must reserve bounded indices.");

        assert_eq!(
            effective.len(),
            128,
            "Fix: uploaded handles must suppress same-sequence fills even under dense duplicate traffic."
        );
        for (position, (handle, value)) in effective.iter().copied().enumerate() {
            assert_eq!(
                handle.id % 2,
                1,
                "Fix: uploaded resident handle {} must not retain a redundant fill.",
                handle.id
            );
            assert_eq!(
                handle.id as usize,
                position * 2 + 1,
                "Fix: first-seen fill order must be stable after duplicate coalescing."
            );
            assert_eq!(
                value, 7,
                "Fix: duplicate resident fills must keep the final value for each handle."
            );
        }
    }
}
