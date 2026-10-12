# vyre-driver-spirv  -  architecture

SPIR-V backend. Lowers `vyre::ir::Program` to SPIR-V binary,
validates against `spirv-tools`, and dispatches via Vulkan.

## Modules

### `backend.rs`
The `VyreBackend` implementation. Holds the Vulkan instance +
device handles, a SPIR-V module cache keyed on conformance
cert, and the per-pipeline descriptor-set layout cache.

### `lib.rs`
Top-level re-exports + the inventory-driven backend registration
that lets `vyre-driver`'s routing table discover this backend
without explicit linking.

## Public types

- **`SpirvBackend`**  -  backend-trait implementation.
- **`SpirvBackendRegistration`**  -  the inventory token.

## Integration points

- Plugs into `vyre-driver` via inventory.
- Used for non-NVIDIA Vulkan-capable GPUs and the desktop Linux +
  Apple Silicon Metal-via-MoltenVK path on the preflight smoke
  matrix.
