# vyre-driver-reference

`vyre-driver-reference` registers the pure Rust `cpu-ref` backend adapter.
It keeps `vyre-reference` independent from the driver layer while still letting
conformance and parity harnesses acquire a deterministic CPU oracle when this
crate is linked. Production dispatch must not link this crate as an implicit
runtime backend; CUDA and WGPU are the supported execution backends.
