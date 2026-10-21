# vyre-emit-metal

Metal Shading Language artifact emitter for Vyre.

This crate consumes the shared `vyre_lower::KernelDescriptor`, routes through
`vyre-emit-naga`, validates the `naga::Module`, and emits deterministic
`native_module` JSON artifacts containing MSL source plus ABI metadata.

It owns Metal emission only. Program lowering remains in `vyre-lower`, Naga IR
construction remains in `vyre-emit-naga`, and runtime compilation belongs in a
native Metal driver crate.
