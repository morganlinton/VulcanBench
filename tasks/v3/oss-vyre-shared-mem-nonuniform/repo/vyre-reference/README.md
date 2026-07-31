# vyre-reference

Pure-Rust CPU reference interpreter for vyre IR.

```
cargo add vyre-reference
```

## Three consumption modes

### 1. Parity oracle: proving a backend is byte-identical

Used by backend parity tests to run every input through CPU reference and compare byte-for-byte with GPU backend output:

```rust
use vyre::ir::Program;
use vyre_reference::execute;

let program: Program = /* ... */;
let inputs: &[&[u8]] = &[b"input 0", b"input 1"];

let cpu_outputs: Vec<Vec<u8>> = execute(&program, inputs)?;
// compare cpu_outputs against a backend's dispatch() output
```

### 2. Explicit host oracle for tests and diagnostics

Use the reference interpreter when you intentionally need host-side oracle output for tests, diagnostics, or backend development. This is not a production fallback path for GPU-required execution.

```rust
use vyre_reference::execute;

let outputs = execute(&program, inputs)?;
```

### 3. Property-test double

Downstream libraries consuming vyre can assert their GPU pipeline matches this crate:

```rust
#[test]
fn gpu_matches_reference() {
    let program = my_pipeline();
    let inputs = vec![generate_test_input()];

    let cpu = vyre_reference::execute(&program, &inputs).unwrap();
    let gpu = my_backend.dispatch(&program, &inputs, &config).unwrap();

    assert_eq!(cpu, gpu, "Fix: your backend diverged from the reference");
}
```

## Supported operations

Every operation in `vyre::ops::discovered` has a CPU implementation here:

- primitive: arithmetic, bitwise, comparison, float
- hashing: md5, sha1, sha256, sha384, sha512, sha3_256, sha3_512, blake2b, blake2s, blake3, ripemd160, xxhash64, xxhash3_64, siphash13, hmac_*, argon2id
- crypto: chacha20_block
- compression: deflate_decompress, gzip_decompress, zlib_decompress, zstd_decompress, lz4_decompress
- encoding/decoding: base64, hex, utf-8 validation, url_encode/decode
- byte/text scan engines: aho-corasick, glob, kmp, rabin-karp, wildcard, nfa, dfa scan
- string similarity, tokenization, statistics, graph algorithms, workgroup primitives

## MSRV

Rust 1.85.

## License

MIT OR Apache-2.0.
