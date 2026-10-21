# vyre-driver-spirv

SPIR-V backend for [vyre](https://crates.io/crates/vyre), via
[naga](https://crates.io/crates/naga).

## What it does

Emits SPIR-V words from validated `naga::Module` values via
`naga::back::spv::write_vec`. The crate owns only SPIR-V serialization;
shared lowering belongs in a backend-neutral layer, and concrete runtime
dispatch belongs in concrete runtime drivers.

## Using it

```rust,no_run
use vyre_driver_spirv::SpirvBackend;

// The caller passes the naga::Module produced by the shared VYRE lowering path.
let module: naga::Module = build_module_for_current_program();
let spirv_words: Vec<u32> = SpirvBackend::emit_spv(&module).expect("spv emit");
// hand `spirv_words` to your Vulkan dispatch stack.
# fn build_module_for_current_program() -> naga::Module { naga::Module::default() }
```

## Relationship to runtime drivers

`vyre-driver-spirv` does not own a device queue or Vulkan dispatch stack. It
emits a SPIR-V blob for consumers that own execution. The registered
`VyreBackend::dispatch` returns a structured refusal pointing the caller at
the intended flow.

## License

MIT OR Apache-2.0.
