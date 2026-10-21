//! Wire round-trip coverage for first-class quantized datatypes.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Node, Program};
use vyre_spec::{QuantizationScale, QuantizationZeroPoint};

#[test]
fn quantized_i4_grouped_buffer_roundtrips_through_program_wire() {
    let quantized = DataType::Quantized {
        storage: Box::new(DataType::I4),
        scale: QuantizationScale::PerGroup { group_size: 128 },
        zero_point: QuantizationZeroPoint::Absent,
    };
    let program = Program::wrapped(
        vec![BufferDecl::output("weights", 0, quantized.clone()).with_count(1024)],
        [1, 1, 1],
        vec![Node::Return],
    );

    let wire = program
        .to_wire()
        .expect("Fix: quantized i4 buffer declarations must encode");
    let decoded = Program::from_wire(&wire).expect("Fix: quantized i4 program must decode");

    assert_eq!(decoded.buffers()[0].element(), quantized);
    assert_eq!(decoded.buffers()[0].count(), 1024);
}

#[test]
fn quantized_i8_channel_affine_buffer_roundtrips_through_program_wire() {
    let quantized = DataType::Quantized {
        storage: Box::new(DataType::I8),
        scale: QuantizationScale::PerChannel { axis: 1 },
        zero_point: QuantizationZeroPoint::PerChannel { axis: 1 },
    };
    let program = Program::wrapped(
        vec![BufferDecl::output("activations", 0, quantized.clone()).with_count(4096)],
        [1, 1, 1],
        vec![Node::Return],
    );

    let wire = program
        .to_wire()
        .expect("Fix: quantized i8 affine buffer declarations must encode");
    let decoded = Program::from_wire(&wire).expect("Fix: quantized i8 program must decode");

    assert_eq!(decoded.buffers()[0].element(), quantized);
    assert_eq!(decoded.buffers()[0].count(), 4096);
}

#[test]
fn quantized_i4_grouped_affine_buffer_roundtrips_grouped_zero_point() {
    let quantized = DataType::Quantized {
        storage: Box::new(DataType::I4),
        scale: QuantizationScale::PerGroup { group_size: 64 },
        zero_point: QuantizationZeroPoint::PerGroup { group_size: 64 },
    };
    let program = Program::wrapped(
        vec![BufferDecl::output("grouped_affine", 0, quantized.clone()).with_count(2048)],
        [1, 1, 1],
        vec![Node::Return],
    );

    let wire = program
        .to_wire()
        .expect("Fix: grouped affine quantized buffer declarations must encode");
    let decoded = Program::from_wire(&wire).expect("Fix: grouped affine program must decode");

    assert_eq!(decoded.buffers()[0].element(), quantized);
    assert_eq!(decoded.buffers()[0].count(), 2048);
}

#[test]
fn quantized_buffer_rejects_float32_storage_before_wire_emission() {
    let invalid = DataType::Quantized {
        storage: Box::new(DataType::F32),
        scale: QuantizationScale::PerTensor,
        zero_point: QuantizationZeroPoint::Absent,
    };
    let program = Program::wrapped(
        vec![BufferDecl::output("bad_weights", 0, invalid).with_count(16)],
        [1, 1, 1],
        vec![Node::Return],
    );

    let error = program
        .to_wire()
        .expect_err("Fix: quantized f32 storage must not be serialized as a valid buffer type");

    assert!(
        error
            .to_string()
            .contains("quantized memory-region storage"),
        "Fix: encode failure should identify the invalid quantized storage contract: {error}"
    );
}

#[test]
fn generated_quantized_buffer_matrix_roundtrips_through_program_wire() {
    let storages = [
        DataType::I4,
        DataType::I8,
        DataType::I16,
        DataType::U8,
        DataType::U16,
        DataType::F8E4M3,
        DataType::F8E5M2,
        DataType::FP4,
        DataType::NF4,
    ];
    let scales = [
        QuantizationScale::PerTensor,
        QuantizationScale::PerChannel { axis: 0 },
        QuantizationScale::PerGroup { group_size: 32 },
    ];
    let zero_points = [
        QuantizationZeroPoint::Absent,
        QuantizationZeroPoint::PerTensor,
        QuantizationZeroPoint::PerChannel { axis: 1 },
        QuantizationZeroPoint::PerGroup { group_size: 32 },
    ];
    let counts = [
        1u32, 2, 3, 7, 8, 15, 16, 31, 32, 63, 64, 127, 128, 255, 256, 1024,
    ];
    let mut checked = 0usize;

    for storage in storages {
        for scale in scales.iter().cloned() {
            for zero_point in zero_points.iter().cloned() {
                for count in counts {
                    let quantized = DataType::Quantized {
                        storage: Box::new(storage.clone()),
                        scale: scale.clone(),
                        zero_point: zero_point.clone(),
                    };
                    let program = Program::wrapped(
                        vec![BufferDecl::output("q", 0, quantized.clone()).with_count(count)],
                        [1, 1, 1],
                        vec![Node::Return],
                    );

                    let wire = program.to_wire().unwrap_or_else(|error| {
                        panic!(
                            "Fix: generated quantized buffer should encode for storage={storage}, scale={scale}, zero_point={zero_point}, count={count}: {error}"
                        )
                    });
                    let decoded = Program::from_wire(&wire).unwrap_or_else(|error| {
                        panic!(
                            "Fix: generated quantized buffer should decode for storage={storage}, scale={scale}, zero_point={zero_point}, count={count}: {error}"
                        )
                    });

                    assert_eq!(decoded.buffers()[0].element(), quantized);
                    assert_eq!(decoded.buffers()[0].count(), count);
                    checked += 1;
                }
            }
        }
    }

    assert!(
        checked >= 1_728,
        "Fix: quantized wire matrix should cover at least 1728 generated cases, got {checked}"
    );
}

#[test]
fn quantized_buffer_rejects_zero_group_size_before_wire_emission() {
    let invalid = DataType::Quantized {
        storage: Box::new(DataType::I4),
        scale: QuantizationScale::PerGroup { group_size: 0 },
        zero_point: QuantizationZeroPoint::Absent,
    };
    let program = Program::wrapped(
        vec![BufferDecl::output("bad_group", 0, invalid).with_count(16)],
        [1, 1, 1],
        vec![Node::Return],
    );

    let error = program
        .to_wire()
        .expect_err("Fix: quantized PerGroup scale with group_size=0 must not serialize");

    assert!(
        error.to_string().contains("group_size > 0"),
        "Fix: encode failure should identify invalid quantized group size: {error}"
    );
}

#[test]
fn quantized_buffer_rejects_zero_point_zero_group_size_before_wire_emission() {
    let invalid = DataType::Quantized {
        storage: Box::new(DataType::I4),
        scale: QuantizationScale::PerGroup { group_size: 32 },
        zero_point: QuantizationZeroPoint::PerGroup { group_size: 0 },
    };
    let program = Program::wrapped(
        vec![BufferDecl::output("bad_zp_group", 0, invalid).with_count(16)],
        [1, 1, 1],
        vec![Node::Return],
    );

    let error = program
        .to_wire()
        .expect_err("Fix: quantized PerGroup zero-point with group_size=0 must not serialize");

    assert!(
        error
            .to_string()
            .contains("zero-point requires group_size > 0"),
        "Fix: encode failure should identify invalid quantized zero-point group size: {error}"
    );
}

#[test]
fn generated_grouped_affine_quantized_shapes_preserve_scale_zero_point_coupling() {
    let storages = [
        DataType::I4,
        DataType::I8,
        DataType::U8,
        DataType::F8E4M3,
        DataType::NF4,
    ];
    let group_sizes = [1u32, 2, 4, 8, 16, 32, 64, 128, 256];
    let counts = [1u32, 7, 8, 31, 32, 255, 256, 4096];
    let mut checked = 0usize;

    for storage in &storages {
        for group_size in group_sizes {
            for count in counts {
                let quantized = DataType::Quantized {
                    storage: Box::new(storage.clone()),
                    scale: QuantizationScale::PerGroup { group_size },
                    zero_point: QuantizationZeroPoint::PerGroup { group_size },
                };
                let program = Program::wrapped(
                    vec![BufferDecl::storage(
                        "weights",
                        0,
                        BufferAccess::ReadOnly,
                        quantized.clone(),
                    )
                    .with_count(count)],
                    [1, 1, 1],
                    vec![Node::Return],
                );

                let wire = program.to_wire().unwrap_or_else(|error| {
                    panic!(
                        "Fix: grouped affine quantized shape should encode for storage={storage}, group_size={group_size}, count={count}: {error}"
                    )
                });
                let decoded = Program::from_wire(&wire).unwrap_or_else(|error| {
                    panic!(
                        "Fix: grouped affine quantized shape should decode for storage={storage}, group_size={group_size}, count={count}: {error}"
                    )
                });

                assert_eq!(decoded.buffers()[0].element(), quantized);
                assert_eq!(decoded.buffers()[0].count(), count);
                checked += 1;
            }
        }
    }

    assert_eq!(
        checked,
        storages.len() * group_sizes.len() * counts.len(),
        "Fix: generated grouped affine quantized matrix must cover every storage/group/count combination"
    );
}
