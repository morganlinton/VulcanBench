//! Pure IR walks over [`vyre_foundation::ir::Program`] shared by all backends.

mod dispatch_params;
mod grid;
mod indirect;
mod launch_geometry;
mod outputs;

pub use dispatch_params::{
    dispatch_element_count, dispatch_element_count_for_program, dispatch_param_words_into,
    try_dispatch_param_words, try_dispatch_param_words_into,
};
pub use grid::{
    auto_grid, coerce_to_pow2_with_tail_mask, infer_dispatch_grid, infer_dispatch_grid_for_count,
    try_coerce_to_pow2_with_tail_mask, TailMaskPolicy,
};
pub use indirect::{find_indirect_dispatch, IndirectDispatch};
pub(crate) use launch_geometry::program_uses_launch_geometry_ids;
pub use outputs::{
    element_size_bytes, enforce_actual_output_budget, output_binding_layout,
    output_binding_layouts, output_binding_layouts_into, output_layout_from_program,
    OutputBindingLayout, OutputLayout,
};

#[cfg(test)]
mod tests;
