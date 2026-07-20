//! Recursive well-formedness checks for data-type layout contracts.

use super::{DataType, QuantizationScale, QuantizationZeroPoint};

impl DataType {
    /// Validate recursively that this data-type value is a well-formed spec
    /// contract, not merely a constructible enum value.
    ///
    /// This rejects zero-lane vectors, zero-byte arrays, zero-sized BSR blocks,
    /// empty/non-positive device meshes, invalid quantized storage families, and
    /// zero-sized quantization groups. The enum remains constructible for
    /// migration and fuzzing, but release paths should call this before freezing
    /// signatures or allocating backend buffers.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic for the first malformed layout field.
    pub fn validate_layout(&self) -> Result<(), String> {
        self.validate_layout_at("DataType")
    }

    fn validate_layout_at(&self, path: &str) -> Result<(), String> {
        match self {
            Self::Array { element_size } => {
                if *element_size == 0 {
                    return Err(format!(
                        "Fix: {path}::Array element_size must be > 0 for a frozen layout contract."
                    ));
                }
                Ok(())
            }
            Self::Vec { element, count } => {
                if *count == 0 {
                    return Err(format!(
                        "Fix: {path}::Vec count must be > 0 for a frozen layout contract."
                    ));
                }
                element.validate_layout_at("DataType::Vec.element")
            }
            Self::TensorShaped { element, shape } => {
                for (axis, &dim) in shape.iter().enumerate() {
                    if dim == 0 {
                        return Err(format!(
                            "Fix: {path}::TensorShaped shape[{axis}] must be > 0 for a frozen layout contract."
                        ));
                    }
                }
                element.validate_layout_at("DataType::TensorShaped.element")
            }
            Self::SparseCsr { element } => {
                element.validate_layout_at("DataType::SparseCsr.element")
            }
            Self::SparseCoo { element } => {
                element.validate_layout_at("DataType::SparseCoo.element")
            }
            Self::SparseBsr {
                element,
                block_rows,
                block_cols,
            } => {
                if *block_rows == 0 {
                    return Err(format!(
                        "Fix: {path}::SparseBsr block_rows must be > 0 for a frozen layout contract."
                    ));
                }
                if *block_cols == 0 {
                    return Err(format!(
                        "Fix: {path}::SparseBsr block_cols must be > 0 for a frozen layout contract."
                    ));
                }
                element.validate_layout_at("DataType::SparseBsr.element")
            }
            Self::DeviceMesh { axes } => {
                if axes.is_empty() {
                    return Err(format!(
                        "Fix: {path}::DeviceMesh axes must not be empty for a frozen layout contract."
                    ));
                }
                for (axis, &extent) in axes.iter().enumerate() {
                    if extent == 0 {
                        return Err(format!(
                            "Fix: {path}::DeviceMesh axes[{axis}] must be > 0 for a frozen layout contract."
                        ));
                    }
                }
                Ok(())
            }
            Self::Quantized {
                storage,
                scale,
                zero_point,
            } => {
                if !storage.is_quantized_storage() {
                    return Err(format!(
                        "Fix: {path}::Quantized storage {storage} is not a supported packed quantized storage type."
                    ));
                }
                validate_quantization_scale(scale, path)?;
                validate_quantization_zero_point(zero_point, path)
            }
            _ => Ok(()),
        }
    }
}

fn validate_quantization_scale(scale: &QuantizationScale, path: &str) -> Result<(), String> {
    match scale {
        QuantizationScale::PerGroup { group_size } if *group_size == 0 => Err(format!(
            "Fix: {path}::Quantized scale PerGroup group_size must be > 0."
        )),
        _ => Ok(()),
    }
}

fn validate_quantization_zero_point(
    zero_point: &QuantizationZeroPoint,
    path: &str,
) -> Result<(), String> {
    match zero_point {
        QuantizationZeroPoint::PerGroup { group_size } if *group_size == 0 => Err(format!(
            "Fix: {path}::Quantized zero_point PerGroup group_size must be > 0."
        )),
        _ => Ok(()),
    }
}
