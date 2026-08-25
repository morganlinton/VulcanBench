# vyre-runtime

Persistent megakernel + io_uring GPU-ingest runtime for vyre.

## What this crate provides

The execution runtime layer: the bridge between "I have a compiled
`Program`" and "bytes flow through the GPU continuously."

| Module | Purpose |
|--------|---------|
| `megakernel/` | Persistent GPU process: ring buffer, CAS protocol, opcode dispatch, JIT fusion |
| `megakernel/protocol` | Slot layout, control words, opcode constants |
| `megakernel/opcode` | Built-in opcode handlers + `OpcodeHandler` extension |
| `megakernel/builder` | IR `Program` construction (interpreted + JIT variants) |
| `pipeline_cache` | Content-addressed compilation cache (`blake3` fingerprint) |
| `uring/` | Linux `io_uring` ingest: registered GPU-visible reads and native GPUDirect NVMe→BAR1 reads |

## Quick start

```rust
use std::sync::Arc;
use vyre_driver::backend::VyreBackend;
use vyre_runtime::{GpuStream, Megakernel};

fn run(backend: Arc<dyn VyreBackend>) -> Result<(), Box<dyn std::error::Error>> {
    let megakernel = Megakernel::bootstrap(backend)?;
    let mut stream = GpuStream::new();
    loop {
        let control = Megakernel::encode_control(
            stream.is_shutdown_requested(), 1, 16);
        let ring = Megakernel::encode_empty_ring(megakernel.slot_count());
        let debug = Megakernel::encode_empty_debug_log(64);
        let _outputs = megakernel.dispatch(control, ring, debug)?;
        if stream.is_shutdown_requested() { break; }
    }
    Ok(())
}
```

## Formerly `vyre-pipeline`

This crate was renamed from `vyre-pipeline` to `vyre-runtime` to
accurately reflect its role as the execution runtime layer, not a
graphics pipeline or CI/CD pipeline.
