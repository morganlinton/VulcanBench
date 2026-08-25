//! Backend-neutral binding-plan construction for VYRE programs.

use smallvec::SmallVec;
use std::sync::Arc;
use vyre_foundation::ir::{BufferAccess, BufferDecl, MemoryKind, Program};

use crate::BackendError;

/// Host/device binding role assigned to one VYRE buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingRole {
    /// Host input copied to a read-only device buffer.
    Input,
    /// Device output read back after dispatch.
    Output,
    /// Host input copied to a read-write device buffer and read back later.
    InputOutput,
    /// Uniform-style read-only input.
    Uniform,
    /// Workgroup-local memory declared in target code.
    Shared,
    /// Persistent memory handle managed by runtime ingest APIs.
    Persistent,
}

/// One validated binding descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// VYRE buffer name.
    pub name: Arc<str>,
    /// VYRE binding number.
    pub binding: u32,
    /// Original buffer index in `Program::buffers`.
    pub buffer_index: usize,
    /// Host/device role for launch.
    pub role: BindingRole,
    /// Element size in bytes when statically known.
    pub element_size: usize,
    /// Preferred byte alignment for backend allocation/upload planning.
    ///
    /// This optimization contract is derived from `BufferDecl::hints` and the
    /// scalar element size. It does not change program semantics; concrete
    /// drivers use it to choose buffer allocation and launch paths without
    /// rewalking the IR.
    pub preferred_alignment: usize,
    /// Declared or input-derived element count. Zero means runtime-sized.
    pub element_count: u32,
    /// Static byte count when known.
    pub static_byte_len: Option<usize>,
    /// Index in the caller's input slice, if this binding consumes input.
    pub input_index: Option<usize>,
    /// Index in the backend output vector, if this binding is observed output.
    pub output_index: Option<usize>,
}

/// Deterministic ABI plan for a VYRE program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingPlan {
    /// Ordered binding descriptors, sorted by VYRE binding number.
    pub bindings: Vec<Binding>,
    /// Original program buffer indices that consume host inputs.
    pub input_indices: Vec<usize>,
    /// Original program buffer indices that produce host outputs.
    pub output_indices: Vec<usize>,
    /// Original program buffer indices that are workgroup-local.
    pub shared_indices: Vec<usize>,
}

#[derive(Clone, Copy)]
enum InputLengths<'a> {
    None,
    Owned(&'a [Vec<u8>]),
    Borrowed(&'a [&'a [u8]]),
    Lengths(&'a [usize]),
}

impl InputLengths<'_> {
    fn len(self) -> usize {
        match self {
            Self::None => 0,
            Self::Owned(inputs) => inputs.len(),
            Self::Borrowed(inputs) => inputs.len(),
            Self::Lengths(lengths) => lengths.len(),
        }
    }

    fn get(self, index: usize) -> Option<usize> {
        match self {
            Self::None => None,
            Self::Owned(inputs) => inputs.get(index).map(Vec::len),
            Self::Borrowed(inputs) => inputs.get(index).map(|input| input.len()),
            Self::Lengths(lengths) => lengths.get(index).copied(),
        }
    }
}

impl BindingPlan {
    /// Build a binding plan from a VYRE program without host input checks.
    ///
    /// # Errors
    ///
    /// Returns when memory/access combinations or static byte sizing cannot be
    /// represented by a concrete backend ABI.
    pub fn build(program: &Program) -> Result<Self, BackendError> {
        Self::build_inner(program, InputLengths::None, false)
    }

    /// Build and validate a binding plan from a VYRE program.
    ///
    /// # Errors
    ///
    /// Returns when input count, input byte lengths, buffer alignment, or
    /// memory/access combinations do not match the backend ABI contract.
    pub fn from_program(program: &Program, inputs: &[Vec<u8>]) -> Result<Self, BackendError> {
        Self::build_inner(program, InputLengths::Owned(inputs), true)
    }

    /// Build and validate a binding plan from borrowed input buffers.
    ///
    /// # Errors
    ///
    /// Returns when input count, input byte lengths, buffer alignment, or
    /// memory/access combinations do not match the backend ABI contract.
    pub fn from_borrowed_inputs(program: &Program, inputs: &[&[u8]]) -> Result<Self, BackendError> {
        Self::build_inner(program, InputLengths::Borrowed(inputs), true)
    }

    /// Build and validate a binding plan from backend-resident input byte lengths.
    ///
    /// # Errors
    ///
    /// Returns when resident input counts are wrong or byte lengths are
    /// smaller than the program ABI requires.
    pub fn from_input_lengths(
        program: &Program,
        input_lengths: &[usize],
    ) -> Result<Self, BackendError> {
        Self::build_inner(program, InputLengths::Lengths(input_lengths), true)
    }

    /// Verifies backend-resident input byte lengths satisfy this binding plan.
    ///
    /// # Errors
    ///
    /// Returns when the caller supplies the wrong number of resident inputs or
    /// a resident input is smaller than the buffer declaration captured in
    /// this plan.
    pub fn validate_input_byte_lengths(&self, input_lengths: &[usize]) -> Result<(), BackendError> {
        self.validate_input_lengths(InputLengths::Lengths(input_lengths))
    }

    /// Verifies dynamic input slices match the expected plan.
    ///
    /// # Errors
    ///
    /// Returns when the caller supplies the wrong number of inputs or an input
    /// length violates the buffer declaration.
    pub fn validate_inputs(&self, inputs: &[Vec<u8>]) -> Result<(), BackendError> {
        self.validate_input_lengths(InputLengths::Owned(inputs))
    }

    /// Verifies borrowed dynamic input slices match the expected plan.
    ///
    /// # Errors
    ///
    /// Returns when the caller supplies the wrong number of inputs or an input
    /// length violates the buffer declaration.
    pub fn validate_borrowed_inputs(&self, inputs: &[&[u8]]) -> Result<(), BackendError> {
        self.validate_input_lengths(InputLengths::Borrowed(inputs))
    }

    fn validate_input_lengths(&self, input_lens: InputLengths<'_>) -> Result<(), BackendError> {
        if input_lens.len() != self.input_indices.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: dispatch expected {} input buffer(s) from Program declarations but received {}.",
                    self.input_indices.len(),
                    input_lens.len()
                ),
            });
        }

        for binding in &self.bindings {
            if let Some(input_index) = binding.input_index {
                let byte_len = input_lens.get(input_index).ok_or_else(|| {
                    BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: dispatch input index {input_index} for `{}` was missing after input-count validation.",
                            binding.name
                        ),
                    }
                })?;
                validate_input_len(
                    binding,
                    byte_len,
                    !matches!(input_lens, InputLengths::Lengths(_)),
                )?;
            }
        }
        Ok(())
    }

    fn build_inner(
        program: &Program,
        input_lens: InputLengths<'_>,
        validate_inputs_now: bool,
    ) -> Result<Self, BackendError> {
        let mut ordered = SmallVec::<[(usize, &BufferDecl); 16]>::new();
        vyre_foundation::allocation::try_reserve_smallvec_to_capacity(
            &mut ordered,
            program.buffers().len(),
        )
        .map_err(|error| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: binding-plan construction could not reserve {} ordered buffer slot(s): {error}. Split the program buffers or construct a smaller pipeline.",
                    program.buffers().len()
                ),
            }
        })?;
        let buffer_count = program.buffers().len();
        ordered.extend(program.buffers().iter().enumerate());
        ordered.sort_by_key(|(_, buffer)| buffer.binding());

        let mut bindings = Vec::new();
        crate::allocation::try_reserve_vec_to_capacity(&mut bindings, ordered.len()).map_err(
            |error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: binding-plan construction could not reserve {} binding descriptor(s): {error}. Split the program buffers or construct a smaller pipeline.",
                    ordered.len()
                ),
            },
        )?;
        let (input_slot_count, output_slot_count, shared_slot_count) =
            binding_role_counts(&ordered)?;
        let mut logical_input_slots = Vec::new();
        crate::allocation::try_reserve_vec_to_capacity(&mut logical_input_slots, buffer_count)
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: binding-plan construction could not reserve {buffer_count} logical input slot(s): {error}. Split the program buffers or construct a smaller pipeline.",
                ),
            })?;
        logical_input_slots.resize(buffer_count, None);
        let mut logical_output_slots = Vec::new();
        crate::allocation::try_reserve_vec_to_capacity(&mut logical_output_slots, buffer_count)
            .map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: binding-plan construction could not reserve {buffer_count} logical output slot(s): {error}. Split the program buffers or construct a smaller pipeline.",
                ),
            })?;
        logical_output_slots.resize(buffer_count, None);
        let mut input_indices = SmallVec::<[usize; 8]>::new();
        let mut output_indices = SmallVec::<[usize; 8]>::new();
        let mut shared_indices = SmallVec::<[usize; 4]>::new();
        vyre_foundation::allocation::try_reserve_smallvec_to_capacity(
            &mut input_indices,
            input_slot_count,
        )
        .map_err(|error| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: binding-plan construction could not reserve {input_slot_count} input index slot(s): {error}. Split the program buffers or construct a smaller pipeline."
                ),
            }
        })?;
        vyre_foundation::allocation::try_reserve_smallvec_to_capacity(
            &mut output_indices,
            output_slot_count,
        )
        .map_err(|error| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: binding-plan construction could not reserve {output_slot_count} output index slot(s): {error}. Split the program buffers or construct a smaller pipeline."
                ),
            }
        })?;
        vyre_foundation::allocation::try_reserve_smallvec_to_capacity(
            &mut shared_indices,
            shared_slot_count,
        )
        .map_err(|error| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: binding-plan construction could not reserve {shared_slot_count} shared index slot(s): {error}. Split the program buffers or construct a smaller pipeline."
                ),
            }
        })?;

        for (buffer_index, buffer) in program.buffers().iter().enumerate() {
            let role = role_for_buffer(buffer)?;
            if matches!(
                role,
                BindingRole::Input | BindingRole::InputOutput | BindingRole::Uniform
            ) {
                let index = input_indices.len();
                input_indices.push(buffer_index);
                logical_input_slots[buffer_index] = Some(index);
            }
            if matches!(role, BindingRole::Output | BindingRole::InputOutput)
                || buffer.pipeline_live_out
            {
                let index = output_indices.len();
                output_indices.push(buffer_index);
                logical_output_slots[buffer_index] = Some(index);
            }
            if role == BindingRole::Shared {
                shared_indices.push(buffer_index);
            }
        }

        for (buffer_index, buffer) in ordered {
            let role = role_for_buffer(buffer)?;
            let consumes_input = matches!(
                role,
                BindingRole::Input | BindingRole::InputOutput | BindingRole::Uniform
            );
            let produces_output = matches!(role, BindingRole::Output | BindingRole::InputOutput);
            buffer
                .element()
                .validate_layout()
                .map_err(|error| BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: binding `{}` has malformed data-type layout metadata: {error}",
                        buffer.name()
                    ),
                })?;
            let element_size = buffer.element().min_bytes();
            let static_byte_len = static_byte_len(buffer)?;
            let preferred_alignment = preferred_alignment(buffer, element_size)?;

            let input_index = if consumes_input {
                Some(logical_input_slots
                    .get(buffer_index)
                    .copied()
                    .flatten()
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: binding `{}` consumes input but no logical input slot was assigned. Rebuild BindingPlan from Program::buffers order before launch.",
                            buffer.name()
                        ),
                    })?)
            } else {
                None
            };
            let output_index = if produces_output || buffer.pipeline_live_out {
                Some(logical_output_slots
                    .get(buffer_index)
                    .copied()
                    .flatten()
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: binding `{}` produces output but no logical output slot was assigned. Rebuild BindingPlan from Program::buffers order before readback.",
                            buffer.name()
                        ),
                    })?)
            } else {
                None
            };
            let element_count = if buffer.count() == 0 {
                input_index
                    .and_then(|index| input_lens.get(index))
                    .and_then(|byte_len| dynamic_element_count_from_bytes(buffer, byte_len))
                    .unwrap_or(0)
            } else {
                buffer.count()
            };

            bindings.push(Binding {
                name: Arc::clone(&buffer.name),
                binding: buffer.binding(),
                buffer_index,
                role,
                element_size,
                preferred_alignment,
                element_count,
                static_byte_len,
                input_index,
                output_index,
            });
        }

        let plan = Self {
            bindings,
            input_indices: input_indices.into_vec(),
            output_indices: output_indices.into_vec(),
            shared_indices: shared_indices.into_vec(),
        };

        if validate_inputs_now {
            plan.validate_input_lengths(input_lens)?;
        }

        Ok(plan)
    }
}

fn binding_role_counts(
    ordered: &SmallVec<[(usize, &BufferDecl); 16]>,
) -> Result<(usize, usize, usize), BackendError> {
    ordered
        .iter()
        .try_fold((0usize, 0usize, 0usize), |(inputs, outputs, shared), (_, buffer)| {
            let role = role_for_buffer(buffer)?;
            let next_inputs = inputs
                .checked_add(usize::from(matches!(
                    role,
                    BindingRole::Input | BindingRole::InputOutput | BindingRole::Uniform
                )))
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: binding-plan input role count overflowed usize. Split the program buffers before binding-plan construction.".to_string(),
                })?;
            let next_outputs = outputs
                .checked_add(usize::from(
                    matches!(role, BindingRole::Output | BindingRole::InputOutput)
                        || buffer.pipeline_live_out,
                ))
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: binding-plan output role count overflowed usize. Split the program buffers before binding-plan construction.".to_string(),
                })?;
            let next_shared = shared
                .checked_add(usize::from(role == BindingRole::Shared))
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: binding-plan shared role count overflowed usize. Split the program buffers before binding-plan construction.".to_string(),
                })?;
            Ok((next_inputs, next_outputs, next_shared))
        })
}

fn role_for_buffer(buffer: &BufferDecl) -> Result<BindingRole, BackendError> {
    if buffer.kind() == MemoryKind::Shared || buffer.access() == BufferAccess::Workgroup {
        return Ok(BindingRole::Shared);
    }
    if buffer.kind() == MemoryKind::Persistent {
        return Ok(BindingRole::Persistent);
    }
    if buffer.is_output || buffer.pipeline_live_out {
        return Ok(BindingRole::Output);
    }
    match buffer.access() {
        BufferAccess::ReadOnly => Ok(BindingRole::Input),
        BufferAccess::ReadWrite => Ok(BindingRole::InputOutput),
        BufferAccess::WriteOnly => Ok(BindingRole::Output),
        BufferAccess::Uniform => Ok(BindingRole::Uniform),
        BufferAccess::Workgroup => Ok(BindingRole::Shared),
        _ => Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: binding `{}` uses an unknown BufferAccess variant; update vyre-driver binding role mapping.",
                buffer.name()
            ),
        }),
    }
}

fn preferred_alignment(buffer: &BufferDecl, element_size: usize) -> Result<usize, BackendError> {
    let hinted = usize::try_from(buffer.hints().preferred_alignment).map_err(|_| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: binding `{}` preferred_alignment does not fit usize on this target.",
                buffer.name()
            ),
        }
    })?;
    if hinted != 0 && !hinted.is_power_of_two() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: binding `{}` preferred_alignment={} is not a power of two. Use 0 or a power-of-two byte alignment.",
                buffer.name(),
                hinted
            ),
        });
    }
    Ok(hinted.max(element_size.max(1)))
}

fn static_byte_len(buffer: &BufferDecl) -> Result<Option<usize>, BackendError> {
    let bytes = buffer
        .static_byte_len()
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: binding `{}` static byte length could not be computed: {error}",
                buffer.name(),
            ),
        })?;
    if buffer.count() == 0 {
        return Ok(None);
    }
    bytes
        .map(Some)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: binding `{}` declares {} elements of a runtime-sized data type; use a byte-addressed buffer contract or a fixed-width element type.",
                buffer.name(),
                buffer.count()
            ),
        })
}

fn dynamic_element_count_from_bytes(buffer: &BufferDecl, byte_len: usize) -> Option<u32> {
    if let Some(bits) = buffer.element().bit_width() {
        let total_bits = byte_len.checked_mul(8)?;
        return u32::try_from(total_bits / bits).ok();
    }
    buffer
        .element()
        .size_bytes()
        .and_then(|element_size| byte_len.checked_div(element_size))
        .and_then(|count| u32::try_from(count).ok())
}

fn validate_input_len(
    binding: &Binding,
    input_len: usize,
    strict_static_input_len: bool,
) -> Result<(), BackendError> {
    if binding.element_size > 1 && input_len % binding.element_size != 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: input `{}` has {} bytes, which is not aligned to its {}-byte element size.",
                binding.name, input_len, binding.element_size
            ),
        });
    }
    if let Some(expected) = binding.static_byte_len {
        if strict_static_input_len && input_len != expected {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: input `{}` expected {expected} bytes from its static buffer declaration but received {} bytes.",
                    binding.name,
                    input_len
                ),
            });
        }
        if !strict_static_input_len && input_len < expected {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: resident input `{}` expected at least {expected} bytes from its static buffer declaration but received {} bytes.",
                    binding.name, input_len
                ),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod exact_length_tests {
    use super::*;
    use vyre_foundation::ir::DataType;

    fn static_u32_input_program(count: u32) -> Program {
        Program::wrapped(
            vec![BufferDecl::read("input", 0, DataType::U32).with_count(count)],
            [1, 1, 1],
            Vec::new(),
        )
    }

    #[test]
    fn static_host_inputs_are_exact_while_resident_inputs_may_be_larger() {
        let program = static_u32_input_program(2);
        let short = vec![0u8; 4];
        let exact = vec![0u8; 8];
        let oversized = vec![0u8; 12];

        let owned_err = BindingPlan::from_program(&program, &[short.clone()])
            .expect_err("owned static input length must be exact");
        assert!(owned_err.to_string().contains("expected 8 bytes"));
        assert!(BindingPlan::from_program(&program, &[exact.clone()]).is_ok());
        let owned_oversized_err = BindingPlan::from_program(&program, &[oversized.clone()])
            .expect_err("owned static input length must remain exact");
        assert!(owned_oversized_err.to_string().contains("expected 8 bytes"));

        let borrowed_short = [short.as_slice()];
        let borrowed_err = BindingPlan::from_borrowed_inputs(&program, &borrowed_short)
            .expect_err("borrowed static input length must be exact");
        assert!(borrowed_err.to_string().contains("expected 8 bytes"));
        let borrowed_oversized = [oversized.as_slice()];
        let borrowed_oversized_err =
            BindingPlan::from_borrowed_inputs(&program, &borrowed_oversized)
                .expect_err("borrowed static input length must remain exact");
        assert!(borrowed_oversized_err
            .to_string()
            .contains("expected 8 bytes"));

        let resident_err = BindingPlan::from_input_lengths(&program, &[4])
            .expect_err("resident static input length must not be smaller than the ABI");
        assert!(resident_err.to_string().contains("at least 8 bytes"));
        let resident_exact = BindingPlan::from_input_lengths(&program, &[8])
            .expect("resident input equal to the ABI size should validate");
        assert_eq!(resident_exact.bindings[0].element_count, 2);
        let resident_oversized = BindingPlan::from_input_lengths(&program, &[12])
            .expect("resident input larger than the ABI size should validate");
        assert_eq!(resident_oversized.bindings[0].element_count, 2);
    }

    #[test]
    fn dynamic_input_length_sets_runtime_element_count() {
        let program = static_u32_input_program(0);
        let plan = BindingPlan::from_program(&program, &[vec![0u8; 12]])
            .expect("Fix: reject bindings without known element width; do not dispatch un-sized dynamic inputs - dynamic input byte length should define element count");

        assert_eq!(plan.bindings[0].element_count, 3);
        assert_eq!(plan.bindings[0].static_byte_len, None);
    }
}

// ---------------------------------------------------------------------------
// N7 binding-set merging across consecutive dispatches
// ---------------------------------------------------------------------------

/// Stable fingerprint of a binding set's *layout*  -  the parts that
/// determine whether two `BindingPlan`s can share a backend bind
/// group layout / descriptor set.
///
/// Two plans with the same [`BindingSetFingerprint`] can reuse the
/// same `portable::BindGroupLayout` or native descriptor set across
/// consecutive dispatches, skipping the layout-rebind cost. The
/// hot-path perf snapshot puts binding rebind at ~20% of warm
/// dispatch time on attention/softmax/reduce shapes.
///
/// Layout (this fingerprint) is distinct from contents (which
/// `program_vsa_fingerprint` covers)  -  two dispatches of the same
/// kernel on different input buffers share a layout fingerprint but
/// differ in their content fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindingSetFingerprint {
    /// Per-binding layout slot: `(binding_index, role, element_size)`.
    /// Ordered by `binding_index` for deterministic equality.
    pub slots: Vec<(u32, BindingRole, usize)>,
}

impl BindingSetFingerprint {
    /// Derive the layout fingerprint from a `BindingPlan`. Stable
    /// across runs and across machines (no random salts).
    #[must_use]
    pub fn from_plan(plan: &BindingPlan) -> Self {
        let mut slots: Vec<(u32, BindingRole, usize)> = plan
            .bindings
            .iter()
            .map(|b| (b.binding, b.role, b.element_size))
            .collect();
        slots.sort_by_key(|(idx, _, _)| *idx);
        Self { slots }
    }
}

/// True when two binding plans can share a backend bind group
/// layout / descriptor set. This is the N7 merge predicate; a
/// driver maintains a cache keyed by [`BindingSetFingerprint`] and
/// reuses the cached layout when this returns `true`.
#[must_use]
pub fn binding_plans_share_layout(a: &BindingPlan, b: &BindingPlan) -> bool {
    BindingSetFingerprint::from_plan(a) == BindingSetFingerprint::from_plan(b)
}

/// Backend-neutral descriptor/bind-group layout slot.
///
/// Concrete drivers own target-specific object creation, but the
/// fingerprint used to decide whether a descriptor layout is reusable is
/// shared here so portable/native/secondary do not grow separate cache-key rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendLayoutSlot {
    /// Target descriptor group/set.
    pub group: u32,
    /// Binding index inside the descriptor group/set.
    pub binding: u32,
    /// Descriptor memory class.
    pub class: BackendLayoutClass,
    /// Whether storage descriptors are read-only.
    pub read_only: bool,
    /// Element size in bytes when statically known.
    pub element_size: usize,
}

/// Backend-neutral descriptor memory class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendLayoutClass {
    /// Read-write or read-only storage buffer.
    Storage,
    /// Uniform/constant buffer.
    Uniform,
}

/// Stable descriptor-layout fingerprint for backend object caches.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BackendLayoutFingerprint {
    /// Canonical slots sorted by `(group, binding)`.
    pub slots: Vec<BackendLayoutSlot>,
}

impl BackendLayoutFingerprint {
    /// Build a deterministic fingerprint from unsorted layout slots.
    #[must_use]
    pub fn new(mut slots: Vec<BackendLayoutSlot>) -> Self {
        slots.sort_by_key(|slot| (slot.group, slot.binding));
        Self { slots }
    }
}

#[cfg(test)]
mod n7_tests {
    use super::*;
    use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Program};

    fn add_one_program() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(16),
                BufferDecl::output("out", 1, DataType::U32).with_count(16),
            ],
            [16, 1, 1],
            vec![],
        )
    }

    fn add_one_program_different_input_count() -> Program {
        // Same binding shape (slot 0 ReadOnly, slot 1 output, both
        // U32), different element_count. Layout fingerprint must match;
        // content fingerprint will not.
        Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(64),
                BufferDecl::output("out", 1, DataType::U32).with_count(64),
            ],
            [16, 1, 1],
            vec![],
        )
    }

    fn different_layout_program() -> Program {
        // Three bindings instead of two  -  must NOT share layout.
        Program::wrapped(
            vec![
                BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32).with_count(16),
                BufferDecl::storage("b", 1, BufferAccess::ReadOnly, DataType::U32).with_count(16),
                BufferDecl::output("out", 2, DataType::U32).with_count(16),
            ],
            [16, 1, 1],
            vec![],
        )
    }

    #[test]
    fn same_layout_with_different_element_counts_shares_fingerprint() {
        let a = BindingPlan::build(&add_one_program()).unwrap();
        let b = BindingPlan::build(&add_one_program_different_input_count()).unwrap();
        assert!(
            binding_plans_share_layout(&a, &b),
            "plans with same (binding, role, element_size) tuples must share layout"
        );
    }

    #[test]
    fn different_binding_count_does_not_share_layout() {
        let a = BindingPlan::build(&add_one_program()).unwrap();
        let b = BindingPlan::build(&different_layout_program()).unwrap();
        assert!(
            !binding_plans_share_layout(&a, &b),
            "plans with different binding count must not share layout"
        );
    }

    #[test]
    fn fingerprint_is_stable_across_repeated_builds() {
        let a = BindingPlan::build(&add_one_program()).unwrap();
        let b = BindingPlan::build(&add_one_program()).unwrap();
        assert_eq!(
            BindingSetFingerprint::from_plan(&a),
            BindingSetFingerprint::from_plan(&b),
            "repeated build of the same Program must produce identical fingerprints"
        );
    }

    #[test]
    fn fingerprint_slots_are_sorted_by_binding_index() {
        let plan = BindingPlan::build(&add_one_program()).unwrap();
        let fp = BindingSetFingerprint::from_plan(&plan);
        let indices: Vec<u32> = fp.slots.iter().map(|(i, _, _)| *i).collect();
        assert_eq!(indices, [0, 1], "slots must be sorted by binding index");
    }

    #[test]
    fn backend_layout_fingerprint_sorts_slots() {
        let a = BackendLayoutFingerprint::new(vec![
            BackendLayoutSlot {
                group: 1,
                binding: 4,
                class: BackendLayoutClass::Storage,
                read_only: false,
                element_size: 4,
            },
            BackendLayoutSlot {
                group: 0,
                binding: 1,
                class: BackendLayoutClass::Uniform,
                read_only: true,
                element_size: 4,
            },
        ]);
        let b = BackendLayoutFingerprint::new(vec![
            BackendLayoutSlot {
                group: 0,
                binding: 1,
                class: BackendLayoutClass::Uniform,
                read_only: true,
                element_size: 4,
            },
            BackendLayoutSlot {
                group: 1,
                binding: 4,
                class: BackendLayoutClass::Storage,
                read_only: false,
                element_size: 4,
            },
        ]);
        assert_eq!(a, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{CacheLocality, DataType, MemoryHints};

    #[test]
    fn binding_plan_carries_alignment_hints() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32)
                .with_count(16)
                .with_hints(MemoryHints {
                    coalesce_axis: Some(0),
                    preferred_alignment: 64,
                    cache_locality: CacheLocality::Streaming,
                })],
            [64, 1, 1],
            vec![],
        );
        let plan = BindingPlan::build(&program).expect("Fix: alignment hint should build");
        assert_eq!(plan.bindings[0].preferred_alignment, 64);
    }

    #[test]
    fn binding_plan_keeps_logical_slots_when_binding_numbers_are_reordered() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("declared_first_high_binding", 9, DataType::U32),
                BufferDecl::output("declared_output_first_high_binding", 8, DataType::U32)
                    .with_count(1),
                BufferDecl::read("declared_second_low_binding", 0, DataType::U32),
                BufferDecl::output("declared_output_second_low_binding", 1, DataType::U32)
                    .with_count(1),
            ],
            [1, 1, 1],
            vec![],
        );
        let inputs = [vec![0u8; 12], vec![0u8; 8]];

        let plan = BindingPlan::from_program(&program, &inputs)
            .expect("Fix: binding plan must accept logical input order before descriptor sorting");

        assert_eq!(
            plan.bindings
                .iter()
                .map(|binding| binding.binding)
                .collect::<Vec<_>>(),
            [0, 1, 8, 9],
            "descriptor ABI must remain sorted by VYRE binding number"
        );
        assert_eq!(
            plan.input_indices,
            [0, 2],
            "caller input slots must follow Program::buffers declaration order"
        );
        assert_eq!(
            plan.output_indices,
            [1, 3],
            "backend output slots must follow Program::buffers declaration order"
        );

        let high_input = plan
            .bindings
            .iter()
            .find(|binding| binding.binding == 9)
            .expect("high binding input descriptor must exist");
        assert_eq!(high_input.input_index, Some(0));
        assert_eq!(high_input.element_count, 3);

        let low_input = plan
            .bindings
            .iter()
            .find(|binding| binding.binding == 0)
            .expect("low binding input descriptor must exist");
        assert_eq!(low_input.input_index, Some(1));
        assert_eq!(low_input.element_count, 2);

        let high_output = plan
            .bindings
            .iter()
            .find(|binding| binding.binding == 8)
            .expect("high binding output descriptor must exist");
        assert_eq!(high_output.output_index, Some(0));

        let low_output = plan
            .bindings
            .iter()
            .find(|binding| binding.binding == 1)
            .expect("low binding output descriptor must exist");
        assert_eq!(low_output.output_index, Some(1));
    }

    #[test]
    fn binding_plan_rejects_non_power_of_two_alignment_hint() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32)
                .with_count(16)
                .with_hints(MemoryHints {
                    coalesce_axis: None,
                    preferred_alignment: 48,
                    cache_locality: CacheLocality::Temporal,
                })],
            [64, 1, 1],
            vec![],
        );
        let err = BindingPlan::build(&program).expect_err("bad alignment must fail");
        assert!(format!("{err}").contains("preferred_alignment=48"));
    }

    #[test]
    fn binding_plan_alignment_defaults_to_element_size() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(16)],
            [64, 1, 1],
            vec![],
        );
        let plan = BindingPlan::build(&program).expect("Fix: default alignment should build");
        assert_eq!(plan.bindings[0].preferred_alignment, 4);
    }

    #[test]
    fn binding_plan_uses_packed_static_byte_len_for_subbyte_elements() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("packed_i4", 0, BufferAccess::ReadOnly, DataType::I4)
                    .with_count(3),
            ],
            [1, 1, 1],
            vec![],
        );
        let plan =
            BindingPlan::build(&program).expect("Fix: packed I4 binding layout should build");

        assert_eq!(plan.bindings[0].element_size, 1);
        assert_eq!(plan.bindings[0].static_byte_len, Some(2));
    }

    #[test]
    fn binding_plan_validates_packed_static_input_lengths() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("packed_i4", 0, BufferAccess::ReadOnly, DataType::I4)
                    .with_count(3),
            ],
            [1, 1, 1],
            vec![],
        );
        let plan = BindingPlan::from_input_lengths(&program, &[2])
            .expect("Fix: packed I4 input should accept the exact packed byte count");

        plan.validate_input_byte_lengths(&[2])
            .expect("Fix: cached packed I4 input length should remain valid");
        plan.validate_input_byte_lengths(&[3])
            .expect("Fix: resident packed I4 input may be larger than its static ABI byte count");
        let error = plan
            .validate_input_byte_lengths(&[1])
            .expect_err("undersized resident byte length must not satisfy packed I4 contract");
        assert!(
            format!("{error}").contains("at least 2 bytes"),
            "Fix: packed resident byte mismatch must be explicit: {error}"
        );
    }

    #[test]
    fn binding_plan_rejects_malformed_data_type_layouts() {
        let program = Program::wrapped(
            vec![BufferDecl::output(
                "bad_vec",
                0,
                DataType::Vec {
                    element: Box::new(DataType::U32),
                    count: 0,
                },
            )
            .with_count(1)],
            [1, 1, 1],
            vec![],
        );

        let error = BindingPlan::build(&program)
            .expect_err("zero-lane vector layout must not enter binding planning");
        assert!(
            format!("{error}").contains("Vec count must be > 0"),
            "Fix: malformed data-type layout diagnostics must survive binding planning: {error}"
        );
    }

    #[test]
    fn binding_plan_validates_cached_resident_input_lengths() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("in", 0, DataType::U32).with_count(4),
                BufferDecl::output("out", 1, DataType::U32).with_count(4),
            ],
            [4, 1, 1],
            vec![],
        );
        let plan = BindingPlan::from_input_lengths(&program, &[16])
            .expect("Fix: resident input length should match the declared u32[4] input");

        plan.validate_input_byte_lengths(&[16])
            .expect("Fix: cached resident plan should accept the same input byte length");
        plan.validate_input_byte_lengths(&[20])
            .expect("Fix: cached resident plan should accept a larger reused allocation");
        let error = plan
            .validate_input_byte_lengths(&[12])
            .expect_err("cached resident plan must reject stale pipeline shape reuse");
        assert!(
            format!("{error}").contains("at least 16 bytes"),
            "wrong resident input length must produce an actionable size mismatch: {error}"
        );
    }
}
