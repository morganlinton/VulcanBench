# vyre-driver-metal

Native Metal backend for Vyre.

This crate owns the `metal` backend ID and the Metal.framework runtime boundary.
It deliberately does not register a fake backend on non-Apple hosts.

Execution path:

```text
Program
  -> vyre-lower::pre_emit
  -> vyre-emit-metal
  -> Metal Shading Language
  -> MTLComputePipelineState
  -> command buffer dispatch
  -> byte readback through vyre-driver output layouts
```

Shared contracts used by this crate:

1. `vyre_driver::BindingPlan` owns input/output/shared binding roles.
2. `vyre_driver::output_binding_layouts` owns readback layout and trimming.
3. `vyre_driver::enforce_actual_output_budget` owns output-size policy.
4. `vyre_emit_metal` owns deterministic MSL artifact emission.
5. The global `inventory` registry owns backend discovery.

On Linux and other non-Apple targets, `vyre_driver_metal::acquire()` returns an
actionable unsupported-feature error and the crate submits no backend
registration.
