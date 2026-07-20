# Operation Definition Contract

This contract defines which operation-declaration fields are universal across
the v0.4.1 release surface and which are class-specific metadata. Required
means a declaration is incomplete without the field. Optional means the field
is valid and should be populated when known, but legacy declarations may omit
it. N/A means the field does not apply to that operation class.

| Field | Primitive | Hardware intrinsic | Composite | Photonic | Tensor-core | Distributed | Rule |
|---|---|---|---|---|---|---|---|
| OpDef.id | Required | Required | Required | Required | Required | Required | Required |
| OpDef.dialect | Required | Required | Required | Required | Required | Required | Required |
| OpDef.category | Required | Required | Required | Required | Required | Required | Required |
| OpDef.signature | Required | Required | Required | Required | Required | Required | Required |
| OpDef.lowerings | Optional | Required | Optional | Required | Required | Required | Optional |
| OpDef.laws | Optional | Optional | Optional | Optional | Optional | Optional | Optional |
| OpDef.compose | Optional | N/A | Required | Optional | Optional | Optional | Optional |
| OpDef.capability_requirements | Optional | Required | Optional | Required | Required | Required | Optional |
| OpDef.determinism | Optional | Required | Optional | Required | Required | Required | Optional |
| OpDef.side_effect | Optional | Required | Optional | Required | Required | Required | Required |
| OpDef.cost_hint | Optional | Optional | Optional | Optional | Optional | Optional | Optional |
| OpSignature.inputs | Required | Required | Required | Required | Required | Required | Required |
| OpSignature.output | Required | Required | Required | Required | Required | Required | Required |
| OpSignature.input_params | Optional | Required | Optional | Required | Required | Required | Optional |
| OpSignature.output_params | Optional | Required | Optional | Required | Required | Required | Optional |
| OpSignature.contract | Optional | Optional | Optional | Optional | Optional | Optional | Optional |
| OpMetadata.id | Required | Required | Required | Required | Required | Required | Required |
| OpMetadata.layer | Required | Required | Required | Required | Required | Required | Required |
| OpMetadata.category | Required | Required | Required | Required | Required | Required | Required |
| OpMetadata.version | Required | Required | Required | Required | Required | Required | Required |
| OpMetadata.description | Optional | Optional | Optional | Optional | Optional | Optional | Optional |
| OpMetadata.signature | Required | Required | Required | Required | Required | Required | Required |
| OpMetadata.strictness | Optional | Optional | Optional | Optional | Optional | Optional | Optional |
| OpMetadata.archetype_signature | Optional | Optional | Optional | Optional | Optional | Optional | Optional |
| OpMetadata.contract | Optional | Optional | Optional | Optional | Optional | Optional | Optional |
| IntrinsicDescriptor.name | N/A | Required | N/A | Required | Required | Optional | N/A |
| IntrinsicDescriptor.hardware | N/A | Required | N/A | Required | Required | Optional | N/A |
| IntrinsicDescriptor.cpu_fn | N/A | Required | N/A | Required | Required | Optional | N/A |
| IntrinsicDescriptor.contract | N/A | Optional | N/A | Required | Required | Optional | N/A |

The v0.4.1 spec carries `OperationContract` as the additive metadata envelope:
`capability_requirements`, `determinism`, `side_effect`, and `cost_hint` are
all optional. Existing operation catalogs can adopt these annotations without
breaking construction sites that have not been audited yet.

The terminal enum set added for v0.4.1 maps to this matrix: small integer and
generic vector/tensor data types are required for hardware intrinsic,
photonic, tensor-core, and distributed operation signatures; subgroup
`BinOp` variants are required for hardware intrinsic and tensor-core metadata;
expanded float `UnOp` variants are required for primitive, hardware, photonic,
and tensor-core metadata; `TernaryOp` covers signature-level `Fma` and
`Select`; rule condition extensions cover the rule column without pushing
domain semantics into backend execution.
