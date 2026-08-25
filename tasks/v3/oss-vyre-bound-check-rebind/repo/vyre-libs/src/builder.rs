//! Shared helpers used by the per-op Cat-A builders.
//!
//! Each op in `vyre-libs` ships a chainable builder that:
//!
//! 1. Accepts [`TensorRef`]s instead of bare `&str` buffer names, so
//!    dtype + shape mismatches fail at `build()` time.
//! 2. Checks every pair of buffer names is unique.
//! 3. Verifies every [`TensorRef`]'s dtype against the op's expected dtype.
//! 4. Verifies element-count overflow.
//! 5. Allows chained overrides (workgroup size, region generator,
//!    tenant id) without churning the function signature  -  extension
//!    fields live inside a `#[non_exhaustive]` options struct so new
//!    knobs never break existing call sites.
//!
//! `BuildOptions` is intentionally small at launch; fields are added
//! rather than removed (the `#[non_exhaustive]` attribute enforces
//! this). Every Cat-A op exposes its builder as `<Op>Builder::new(...)`
//! and delegates defaults through `BuildOptions::default()`.

use vyre::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::ir::model::expr::GeneratorRef;

use crate::tensor_ref::{TensorRef, TensorRefError};

/// Shared child region for one-output indexed maps.
///
/// This is the kernel skeleton behind embedding lookup, byte shuffles,
/// quant pack/unpack, and similar data-layout transforms:
/// `for i in 0..n { out[dst(i)] = value(i) }`.
pub(crate) const INDEXED_MAP_OP_ID: &str = "vyre-libs::substrate::indexed_map";
/// Shared child region for strided per-lane workgroup accumulators.
pub(crate) const STRIDED_ACCUMULATE_OP_ID: &str = "vyre-libs::substrate::strided_accumulate";
/// Shared child region for strided writeback after a tiled row reduction.
pub(crate) const STRIDED_WRITEBACK_OP_ID: &str =
    "anonymous::vyre-libs::substrate::strided_writeback";

/// Shared options every Cat-A builder threads through. Lives here so
/// every op agrees on the same surface.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct BuildOptions {
    /// Workgroup size override. `None` = op's canonical default.
    pub workgroup_size: Option<[u32; 3]>,
    /// Region generator override. `None` = op's canonical `"vyre-libs::…"`
    /// identifier. Used when a downstream crate wraps a Cat-A op and
    /// wants its own generator id in conformance certificates.
    pub region_generator: Option<&'static str>,
    /// Tenant id baked into the region metadata for multi-tenant
    /// deployments. Routed through the megakernel's tenant-mask table
    /// when the Program runs inside `vyre-runtime`.
    pub tenant_id: Option<u32>,
}

impl BuildOptions {
    /// Fluent constructor  -  start with defaults and chain overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the workgroup size.
    #[must_use]
    pub fn with_workgroup_size(mut self, size: [u32; 3]) -> Self {
        self.workgroup_size = Some(size);
        self
    }

    /// Override the region generator name (must be `&'static str`).
    #[must_use]
    pub fn with_region_generator(mut self, name: &'static str) -> Self {
        self.region_generator = Some(name);
        self
    }

    /// Stamp a tenant id into the Cat-A op's region metadata.
    #[must_use]
    pub fn with_tenant_id(mut self, tenant_id: u32) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }
}

macro_rules! impl_cat_a_builder_options {
    ($builder:ident) => {
        impl $builder {
            /// Override the generated Program workgroup size.
            #[must_use]
            pub fn with_workgroup_size(mut self, size: [u32; 3]) -> Self {
                self.options = self.options.with_workgroup_size(size);
                self
            }

            /// Override the Region generator id.
            #[must_use]
            pub fn with_region_generator(mut self, name: &'static str) -> Self {
                self.options = self.options.with_region_generator(name);
                self
            }

            /// Stamp the Region metadata with a tenant id.
            #[must_use]
            pub fn with_tenant_id(mut self, tenant_id: u32) -> Self {
                self.options = self.options.with_tenant_id(tenant_id);
                self
            }
        }
    };
}

pub(crate) use impl_cat_a_builder_options;

/// Validate a slice of `TensorRef`s against an expected `DataType`
/// for each position, plus name-uniqueness across the whole slice.
/// Used by every op's `build()` to consolidate the fanout of checks.
pub fn check_tensors(
    op: &'static str,
    tensors: &[(&TensorRef, DataType)],
) -> Result<(), TensorRefError> {
    // Dtype check per tensor.
    for (r, expected) in tensors {
        crate::tensor_ref::check_dtype(r, expected.clone(), op)?;
        if r.element_count().is_none() {
            return Err(TensorRefError::ElementCountOverflow {
                name: r.name.as_str().to_string(),
                shape: r.shape.to_vec(),
            });
        }
    }
    for (idx, (left, _)) in tensors.iter().enumerate() {
        for (right, _) in &tensors[idx + 1..] {
            if left.name_str() == right.name_str() {
                return Err(TensorRefError::NameCollision {
                    name: left.name.as_str().to_string(),
                    op,
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod cat_a_builder_option_macro_tests {
    #![allow(unreachable_pub)]

    use super::BuildOptions;

    #[derive(Debug, Clone)]
    struct DemoBuilder {
        options: BuildOptions,
    }

    impl DemoBuilder {
        fn new() -> Self {
            Self {
                options: BuildOptions::default(),
            }
        }
    }

    super::impl_cat_a_builder_options!(DemoBuilder);

    #[test]
    fn generated_option_surface_threads_every_shared_knob() {
        let builder = DemoBuilder::new()
            .with_workgroup_size([8, 4, 2])
            .with_region_generator("custom::generator")
            .with_tenant_id(17);

        assert_eq!(builder.options.workgroup_size, Some([8, 4, 2]));
        assert_eq!(builder.options.region_generator, Some("custom::generator"));
        assert_eq!(builder.options.tenant_id, Some(17));
    }
}

/// Build the canonical one-output indexed-map skeleton.
///
/// Callers provide buffer declarations plus the semantic mapping from logical
/// element `i` to `(dst_index, value)`. The loop, bounds guard, invocation id,
/// workgroup default, and composition region stay centralized.
pub(crate) fn build_indexed_map<F>(
    op_id: &'static str,
    buffers: Vec<BufferDecl>,
    output: &str,
    count: u32,
    workgroup_size: [u32; 3],
    f: F,
) -> Program
where
    F: FnOnce(Expr) -> (Expr, Expr),
{
    let i = Expr::var("i");
    let (dst_index, value) = f(i.clone());
    let child_body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i, Expr::u32(count)),
            vec![Node::store(output, dst_index, value)],
        ),
    ];
    let parent = GeneratorRef {
        name: op_id.to_string(),
    };

    Program::wrapped(
        buffers,
        workgroup_size,
        vec![crate::region::wrap_anonymous(
            op_id,
            vec![crate::region::wrap_child(
                INDEXED_MAP_OP_ID,
                parent,
                child_body,
            )],
        )],
    )
}

/// Build a shared strided single-accumulator child region.
///
/// The parent must bind `local = LocalId(0)` before this child. The child
/// accumulates `i = chunk * tile + local` for `chunk in 0..chunks`, guards
/// `i < n`, and stores the lane-local accumulator into `scratch[local]`.
pub(crate) fn strided_accumulate_child<F>(
    parent_op_id: &'static str,
    tile: u32,
    chunks: u32,
    n: u32,
    acc_name: &'static str,
    initial: Expr,
    scratch: &'static str,
    step: F,
) -> Node
where
    F: Fn(Expr, Expr) -> Expr,
{
    let local = Expr::var("local");
    let idx = Expr::var("idx");
    let acc = Expr::var(acc_name);
    let child_body = vec![Node::if_then(
        Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind(acc_name, initial),
            strided_loop(
                tile,
                chunks,
                n,
                vec![Node::assign(acc_name, step(idx, acc))],
            ),
            Node::store(scratch, local, Expr::var(acc_name)),
        ],
    )];

    child_region(parent_op_id, STRIDED_ACCUMULATE_OP_ID, child_body)
}

/// Build a shared strided dual-accumulator child region.
///
/// This keeps paired reductions such as `(sum, sum_sq)` in one memory pass
/// instead of forcing two separate scans over the input.
#[allow(dead_code)]
pub(crate) fn strided_accumulate2_child<F1, F2>(
    parent_op_id: &'static str,
    tile: u32,
    chunks: u32,
    n: u32,
    first: (&'static str, Expr, &'static str, F1),
    second: (&'static str, Expr, &'static str, F2),
) -> Node
where
    F1: Fn(Expr, Expr) -> Expr,
    F2: Fn(Expr, Expr) -> Expr,
{
    let (first_name, first_initial, first_scratch, first_step) = first;
    let (second_name, second_initial, second_scratch, second_step) = second;
    let local = Expr::var("local");
    let idx = Expr::var("idx");
    let child_body = vec![Node::if_then(
        Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind(first_name, first_initial),
            Node::let_bind(second_name, second_initial),
            strided_loop(
                tile,
                chunks,
                n,
                vec![
                    Node::assign(first_name, first_step(idx.clone(), Expr::var(first_name))),
                    Node::assign(second_name, second_step(idx, Expr::var(second_name))),
                ],
            ),
            Node::store(first_scratch, local.clone(), Expr::var(first_name)),
            Node::store(second_scratch, local, Expr::var(second_name)),
        ],
    )];

    child_region(parent_op_id, STRIDED_ACCUMULATE_OP_ID, child_body)
}

/// Build a shared strided writeback child region.
///
/// The parent must bind `local = LocalId(0)` before this child. Optional
/// `prelude` nodes run once in workgroup zero before the strided write loop,
/// which lets row reductions load reduced scalars exactly once per lane.
pub(crate) fn strided_writeback_child<F>(
    parent_op_id: &'static str,
    tile: u32,
    chunks: u32,
    n: u32,
    output: &str,
    prelude: Vec<Node>,
    value: F,
) -> Node
where
    F: Fn(Expr) -> Expr,
{
    let idx = Expr::var("idx");
    let mut guarded = prelude;
    guarded.push(strided_loop(
        tile,
        chunks,
        n,
        vec![Node::store(output, idx.clone(), value(idx))],
    ));
    child_region(
        parent_op_id,
        STRIDED_WRITEBACK_OP_ID,
        vec![Node::if_then(
            Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0)),
            guarded,
        )],
    )
}

fn strided_loop(tile: u32, chunks: u32, n: u32, guarded_body: Vec<Node>) -> Node {
    Node::loop_for(
        "chunk",
        Expr::u32(0),
        Expr::u32(chunks),
        vec![
            Node::let_bind(
                "idx",
                Expr::add(
                    Expr::mul(Expr::var("chunk"), Expr::u32(tile)),
                    Expr::var("local"),
                ),
            ),
            Node::if_then(Expr::lt(Expr::var("idx"), Expr::u32(n)), guarded_body),
        ],
    )
}

fn child_region(parent_op_id: &'static str, child_op_id: &'static str, body: Vec<Node>) -> Node {
    crate::region::wrap_child(
        child_op_id,
        GeneratorRef {
            name: parent_op_id.to_string(),
        },
        body,
    )
}

/// Build a scalar-output trap program for invalid Cat-A builder inputs.
///
/// This keeps public compatibility wrappers infallible without panicking on
/// user-controlled names or shapes. Typed builders should still return
/// `Result`; this helper is for legacy `fn foo(...) -> Program` surfaces.
#[allow(dead_code)]
pub(crate) fn invalid_output_program(
    op_id: &'static str,
    output: &str,
    data_type: DataType,
    message: String,
) -> Program {
    Program::wrapped(
        vec![BufferDecl::output(output, 0, data_type).with_count(1)],
        [1, 1, 1],
        vec![crate::region::wrap_anonymous(
            op_id,
            vec![Node::trap(Expr::u32(0), message)],
        )],
    )
}

/// Tensor-ref elementwise binary builder, used by `math::avg_floor`,
/// `math::algebra`, and other binary-arithmetic primitives.
#[allow(dead_code)]
pub(crate) fn build_elementwise_binary<F>(
    op_id: &'static str,
    a: crate::tensor_ref::TensorRef,
    b: crate::tensor_ref::TensorRef,
    out: crate::tensor_ref::TensorRef,
    options: BuildOptions,
    f: F,
) -> Result<vyre::ir::Program, crate::tensor_ref::TensorRefError>
where
    F: Fn(vyre::ir::Expr, vyre::ir::Expr) -> vyre::ir::Expr,
{
    check_tensors(
        op_id,
        &[
            (&a, vyre::ir::DataType::U32),
            (&b, vyre::ir::DataType::U32),
            (&out, vyre::ir::DataType::U32),
        ],
    )?;

    if a.shape != b.shape || a.shape != out.shape {
        return Err(crate::tensor_ref::TensorRefError::ShapeMismatch {
            name: "elementwise_binary".into(),
            found: vec![],
            expected: vec![],
            op: op_id,
        });
    }

    let a_count = a.element_count().ok_or_else(|| {
        crate::tensor_ref::TensorRefError::ElementCountOverflow {
            name: a.name_str().to_string(),
            shape: a.shape.to_vec(),
        }
    })?;
    let out_count = out.element_count().ok_or_else(|| {
        crate::tensor_ref::TensorRefError::ElementCountOverflow {
            name: out.name_str().to_string(),
            shape: out.shape.to_vec(),
        }
    })?;
    if out_count < a_count {
        return Err(crate::tensor_ref::TensorRefError::ShapeMismatch {
            name: out.name_str().to_string(),
            found: out.shape.to_vec(),
            expected: a.shape.to_vec(),
            op: op_id,
        });
    }

    let n = a_count;
    let body = vec![
        vyre::ir::Node::let_bind("idx", vyre::ir::Expr::InvocationId { axis: 0 }),
        vyre::ir::Node::if_then(
            vyre::ir::Expr::lt(vyre::ir::Expr::var("idx"), vyre::ir::Expr::u32(n)),
            vec![vyre::ir::Node::store(
                out.name_str(),
                vyre::ir::Expr::var("idx"),
                f(
                    vyre::ir::Expr::load(a.name_str(), vyre::ir::Expr::var("idx")),
                    vyre::ir::Expr::load(b.name_str(), vyre::ir::Expr::var("idx")),
                ),
            )],
        ),
    ];

    let group = options.workgroup_size.unwrap_or([64, 1, 1]);

    Ok(vyre::ir::Program::wrapped(
        vec![
            vyre::ir::BufferDecl::storage(
                a.name_str(),
                0,
                vyre::ir::BufferAccess::ReadOnly,
                vyre::ir::DataType::U32,
            )
            .with_count(n),
            vyre::ir::BufferDecl::storage(
                b.name_str(),
                1,
                vyre::ir::BufferAccess::ReadOnly,
                vyre::ir::DataType::U32,
            )
            .with_count(n),
            vyre::ir::BufferDecl::output(out.name_str(), 2, vyre::ir::DataType::U32).with_count(n),
        ],
        group,
        vec![crate::region::wrap_anonymous(op_id, body)],
    ))
}

#[allow(dead_code)]
pub(crate) fn build_elementwise_unary<F>(
    op_id: &'static str,
    a: crate::tensor_ref::TensorRef,
    out: crate::tensor_ref::TensorRef,
    options: BuildOptions,
    f: F,
) -> Result<vyre::ir::Program, crate::tensor_ref::TensorRefError>
where
    F: Fn(vyre::ir::Expr) -> vyre::ir::Expr,
{
    check_tensors(
        op_id,
        &[
            (&a, vyre::ir::DataType::U32),
            (&out, vyre::ir::DataType::U32),
        ],
    )?;

    if a.shape != out.shape {
        return Err(crate::tensor_ref::TensorRefError::ShapeMismatch {
            name: "elementwise_unary".into(),
            found: vec![],
            expected: vec![],
            op: op_id,
        });
    }

    let n = a.element_count().ok_or_else(|| {
        crate::tensor_ref::TensorRefError::ElementCountOverflow {
            name: a.name_str().to_string(),
            shape: a.shape.to_vec(),
        }
    })?;
    let body = vec![
        vyre::ir::Node::let_bind("idx", vyre::ir::Expr::InvocationId { axis: 0 }),
        vyre::ir::Node::if_then(
            vyre::ir::Expr::lt(vyre::ir::Expr::var("idx"), vyre::ir::Expr::u32(n)),
            vec![vyre::ir::Node::store(
                out.name_str(),
                vyre::ir::Expr::var("idx"),
                f(vyre::ir::Expr::load(
                    a.name_str(),
                    vyre::ir::Expr::var("idx"),
                )),
            )],
        ),
    ];

    let group = options.workgroup_size.unwrap_or([64, 1, 1]);

    Ok(vyre::ir::Program::wrapped(
        vec![
            vyre::ir::BufferDecl::storage(
                a.name_str(),
                0,
                vyre::ir::BufferAccess::ReadOnly,
                vyre::ir::DataType::U32,
            )
            .with_count(n),
            vyre::ir::BufferDecl::output(out.name_str(), 1, vyre::ir::DataType::U32).with_count(n),
        ],
        group,
        vec![crate::region::wrap_anonymous(op_id, body)],
    ))
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn build_options_defaults_are_all_none() {
        let o = BuildOptions::default();
        assert!(o.workgroup_size.is_none());
        assert!(o.region_generator.is_none());
        assert!(o.tenant_id.is_none());
    }

    #[test]
    fn build_options_chain_preserves_earlier_setters() {
        let o = BuildOptions::new()
            .with_workgroup_size([128, 1, 1])
            .with_region_generator("test::op")
            .with_tenant_id(7);
        assert_eq!(o.workgroup_size, Some([128, 1, 1]));
        assert_eq!(o.region_generator, Some("test::op"));
        assert_eq!(o.tenant_id, Some(7));
    }

    #[test]
    fn check_tensors_passes_on_clean_inputs() {
        let a = TensorRef::u32_1d("a", 4);
        let b = TensorRef::u32_1d("b", 4);
        assert!(matches!(
            check_tensors("op", &[(&a, DataType::U32), (&b, DataType::U32)]),
            Ok(())
        ));
    }

    #[test]
    fn check_tensors_catches_dtype_mismatch() {
        let a = TensorRef::u32_1d("a", 4);
        let err = check_tensors("op", &[(&a, DataType::F32)]).unwrap_err();
        assert!(matches!(err, TensorRefError::DtypeMismatch { .. }));
    }

    #[test]
    fn check_tensors_catches_overflow() {
        let a = TensorRef::new("big", DataType::U32, vec![1u32 << 20, 1u32 << 20]);
        let err = check_tensors("op", &[(&a, DataType::U32)]).unwrap_err();
        assert!(matches!(err, TensorRefError::ElementCountOverflow { .. }));
    }

    #[test]
    fn check_tensors_catches_name_collision() {
        let a = TensorRef::u32_1d("x", 4);
        let b = TensorRef::u32_1d("x", 4);
        let err = check_tensors("op", &[(&a, DataType::U32), (&b, DataType::U32)]).unwrap_err();
        assert!(matches!(err, TensorRefError::NameCollision { .. }));
    }

    #[test]
    fn indexed_map_builder_emits_shared_child_region() {
        let program = build_indexed_map(
            "vyre-libs::test::indexed_map_user",
            vec![
                BufferDecl::storage("input", 0, vyre::ir::BufferAccess::ReadOnly, DataType::U32)
                    .with_count(4),
                BufferDecl::output("output", 1, DataType::U32).with_count(4),
            ],
            "output",
            4,
            [64, 1, 1],
            |i| (i.clone(), Expr::load("input", i)),
        );
        let rendered = format!("{:?}", program.entry());
        assert!(
            rendered.contains(INDEXED_MAP_OP_ID),
            "Fix: indexed-map users must share the same child region instead of copying loop skeletons: {rendered}"
        );
    }

    #[test]
    fn strided_writeback_builder_emits_shared_child_region() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::F32).with_count(4)],
            [4, 1, 1],
            vec![crate::region::wrap_anonymous(
                "vyre-libs::test::row_reduction_user",
                vec![
                    Node::let_bind("local", Expr::LocalId { axis: 0 }),
                    strided_writeback_child(
                        "vyre-libs::test::row_reduction_user",
                        4,
                        1,
                        4,
                        "out",
                        vec![Node::let_bind("scale", Expr::f32(0.5))],
                        |_idx| Expr::var("scale"),
                    ),
                ],
            )],
        );
        let rendered = format!("{:?}", program.entry());
        assert!(
            rendered.contains(STRIDED_WRITEBACK_OP_ID),
            "Fix: row-reduction writeback users must share the same child region instead of copying loop skeletons: {rendered}"
        );
    }
}
