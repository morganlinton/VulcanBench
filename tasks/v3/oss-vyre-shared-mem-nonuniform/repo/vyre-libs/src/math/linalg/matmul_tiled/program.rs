use vyre::ir::{BufferAccess, BufferDecl, DataType, Program};
use vyre_foundation::ir::model::expr::GeneratorRef;
use vyre_primitives::math::semiring_gemm::OP_ID as SEMIRING_GEMM_OP_ID;

use crate::region::{wrap, wrap_child};
use crate::tensor_ref::TensorRefError;

use super::body::cooperative_matmul_body;
use super::mma_body::cooperative_matmul_body_mma;
use super::mma_fragment::{gate_mma_path, MmaCapabilityRecord};
use super::shape::{output_tile_shape, padded_tile_lane_count, MatrixShape, TileShape};
use super::tensor_core_policy::{select_matmul_kernel, MatmulKernelPath};

pub(super) struct MatmulTiledProgramSpec<'a> {
    pub(super) op_id: &'static str,
    pub(super) a: &'a str,
    pub(super) b: &'a str,
    pub(super) bias: Option<&'a str>,
    pub(super) out: &'a str,
    pub(super) m: u32,
    pub(super) k: u32,
    pub(super) n: u32,
    pub(super) tile: u32,
    pub(super) workgroup: [u32; 3],
    pub(super) generator: &'static str,
    pub(super) dtype: DataType,
    pub(super) a_tile_name: &'a str,
    pub(super) b_tile_name: &'a str,
    pub(super) mma_capabilities: MmaCapabilityRecord,
}

pub(super) fn build_matmul_tiled_program(
    spec: MatmulTiledProgramSpec<'_>,
) -> Result<Program, TensorRefError> {
    let MatmulTiledProgramSpec {
        op_id,
        a,
        b,
        bias,
        out,
        m,
        k,
        n,
        tile,
        workgroup,
        generator,
        dtype,
        a_tile_name,
        b_tile_name,
        mma_capabilities,
    } = spec;

    if tile == 0 {
        return Err(TensorRefError::ShapeMismatch {
            name: "tile".into(),
            found: vec![0],
            expected: vec![1],
            op: op_id,
        });
    }

    let matrix_shape = MatrixShape { m, k, n };
    let selected_kernel = select_matmul_kernel(&dtype, matrix_shape, tile);
    let mma_gate = gate_mma_path(selected_kernel, mma_capabilities);
    let (a_tile_count, b_tile_count, padded_out_count, dispatch_wg, kernel_body) =
        if mma_gate.selected_path == MatmulKernelPath::TensorCoreF16M16N8K16
        {
            let mma_wg = [32, 1, 1];
            let mma_out_rows = 16u32;
            let mma_out_cols = 8u32;
            let mma_lanes = 32u32;
            let mma_a_tile = mma_out_rows.checked_mul(tile).ok_or_else(|| {
                TensorRefError::ElementCountOverflow {
                    name: a_tile_name.to_string(),
                    shape: vec![mma_out_rows, tile],
                }
            })?;
            let mma_b_tile = tile.checked_mul(mma_out_cols).ok_or_else(|| {
                TensorRefError::ElementCountOverflow {
                    name: b_tile_name.to_string(),
                    shape: vec![tile, mma_out_cols],
                }
            })?;
            let out_count = checked_element_count(out, m, n)?;
            let body_nodes = cooperative_matmul_body_mma(
                a,
                b,
                bias,
                out,
                matrix_shape,
                TileShape {
                    k_tile: tile,
                    out_rows: mma_out_rows,
                    out_cols: mma_out_cols,
                    x_lanes: mma_lanes,
                    y_lanes: 1,
                    lanes: mma_lanes,
                    a_values: mma_a_tile,
                    b_values: mma_b_tile,
                },
                dtype.clone(),
                a_tile_name,
                b_tile_name,
            );
            (mma_a_tile, mma_b_tile, out_count, mma_wg, body_nodes)
        } else {
            let (out_tile_cols, out_tile_rows, lane_count) = output_tile_shape(workgroup)?;
            let a_tile_count = out_tile_rows.checked_mul(tile).ok_or_else(|| {
                TensorRefError::ElementCountOverflow {
                    name: a_tile_name.to_string(),
                    shape: vec![out_tile_rows, tile],
                }
            })?;
            let b_tile_count = tile.checked_mul(out_tile_cols).ok_or_else(|| {
                TensorRefError::ElementCountOverflow {
                    name: b_tile_name.to_string(),
                    shape: vec![tile, out_tile_cols],
                }
            })?;
            let padded_out_count =
                padded_tile_lane_count(m, n, out_tile_rows, out_tile_cols, lane_count)?;
            let flat_workgroup = [lane_count, 1, 1];
            let body_nodes = cooperative_matmul_body(
                a,
                b,
                bias,
                out,
                matrix_shape,
                TileShape {
                    k_tile: tile,
                    out_rows: out_tile_rows,
                    out_cols: out_tile_cols,
                    x_lanes: lane_count,
                    y_lanes: 1,
                    lanes: lane_count,
                    a_values: a_tile_count,
                    b_values: b_tile_count,
                },
            );
            (
                a_tile_count,
                b_tile_count,
                padded_out_count,
                flat_workgroup,
                body_nodes,
            )
        };

    let a_count = checked_element_count(a, m, k)?;
    let b_count = checked_element_count(b, k, n)?;
    let logical_out_count = checked_element_count(out, m, n)?;
    let element_size = dtype
        .size_bytes()
        .ok_or_else(|| TensorRefError::ElementCountOverflow {
            name: out.to_string(),
            shape: vec![m, n],
        })?;
    let logical_output_bytes = (logical_out_count as usize)
        .checked_mul(element_size)
        .ok_or_else(|| TensorRefError::ElementCountOverflow {
            name: out.to_string(),
            shape: vec![m, n],
        })?;
    let body = vec![wrap_child(
        SEMIRING_GEMM_OP_ID,
        GeneratorRef {
            name: generator.to_string(),
        },
        kernel_body,
    )];

    let mut buffers = vec![
        BufferDecl::storage(a, 0, BufferAccess::ReadOnly, dtype.clone()).with_count(a_count),
        BufferDecl::storage(b, 1, BufferAccess::ReadOnly, dtype.clone()).with_count(b_count),
    ];
    let out_slot = if let Some(bias) = bias {
        buffers.push(
            BufferDecl::storage(bias, 2, BufferAccess::ReadOnly, dtype.clone()).with_count(n),
        );
        3
    } else {
        2
    };
    buffers.push(BufferDecl::workgroup(
        a_tile_name,
        a_tile_count,
        dtype.clone(),
    ));
    buffers.push(BufferDecl::workgroup(
        b_tile_name,
        b_tile_count,
        dtype.clone(),
    ));
    buffers.push(
        BufferDecl::output(out, out_slot, dtype)
            .with_count(padded_out_count)
            .with_output_byte_range(0..logical_output_bytes),
    );

    Ok(Program::wrapped(
        buffers,
        dispatch_wg,
        vec![wrap(generator, body, None)],
    ))
}

fn checked_element_count(name: &str, rows: u32, cols: u32) -> Result<u32, TensorRefError> {
    rows.checked_mul(cols)
        .ok_or_else(|| TensorRefError::ElementCountOverflow {
            name: name.to_string(),
            shape: vec![rows, cols],
        })
}
#[cfg(test)]
mod tests {
    use super::*;

    fn f16_mma_spec(capabilities: MmaCapabilityRecord) -> MatmulTiledProgramSpec<'static> {
        MatmulTiledProgramSpec {
            op_id: "matmul_tiled.test",
            a: "a",
            b: "b",
            bias: None,
            out: "out",
            m: 32,
            k: 16,
            n: 16,
            tile: 16,
            workgroup: [16, 16, 1],
            generator: "matmul_tiled.test",
            dtype: DataType::F16,
            a_tile_name: "matmul_a_tile",
            b_tile_name: "matmul_b_tile",
            mma_capabilities: capabilities,
        }
    }

    #[test]
    fn supported_ptx_mma_capabilities_emit_mma_body() {
        let program = build_matmul_tiled_program(f16_mma_spec(MmaCapabilityRecord::ptx_sm80()))
            .expect("Fix: PTX F16 M16N8K16 tiled matmul spec must build.");
        let debug = format!("{:?}", program.entry());

        assert!(debug.contains("mma_c0"));
        assert_eq!(program.workgroup_size(), [32, 1, 1]);
    }

    #[test]
    fn unsupported_mma_backend_capabilities_emit_cooperative_body() {
        for capabilities in [MmaCapabilityRecord::metal(), MmaCapabilityRecord::wgpu()] {
            let program = build_matmul_tiled_program(f16_mma_spec(capabilities))
                .expect("Fix: unsupported MMA backends must fall back to cooperative matmul.");
            let debug = format!("{:?}", program.entry());

            assert!(!debug.contains("mma_c0"));
            assert_ne!(program.workgroup_size(), [32, 1, 1]);
        }
    }
}
