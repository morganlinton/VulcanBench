//! Frozen operation type signatures used by validators and catalogs.

use crate::{data_type::DataType, op_contract::OperationContract};

/// Named operation parameter metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct SignatureParam {
    /// Stable parameter name.
    pub name: String,
    /// Parameter data type.
    pub ty: DataType,
    /// Optional human-readable role or constraint.
    #[serde(default)]
    pub metadata: Option<String>,
}

/// Type signature for a vyre IR operation in the frozen data contract.
///
/// Example: an addition operation can declare two `DataType::U32` inputs and
/// one `DataType::U32` output.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct OpSignature {
    /// Input parameter types.
    pub inputs: Vec<DataType>,
    /// Output type.
    pub output: DataType,
    /// Optional typed input parameters with names and metadata.
    #[serde(default)]
    pub input_params: Option<Vec<SignatureParam>>,
    /// Optional typed output parameters with names and metadata.
    #[serde(default)]
    pub output_params: Option<Vec<SignatureParam>>,
    /// Optional capability and execution contract annotations.
    #[serde(default)]
    pub contract: Option<OperationContract>,
}

impl OpSignature {
    /// Minimum valid input byte count for this signature.
    #[must_use]
    pub fn min_input_bytes(&self) -> usize {
        self.inputs
            .iter()
            .map(super::data_type::DataType::min_bytes)
            .sum()
    }
}
