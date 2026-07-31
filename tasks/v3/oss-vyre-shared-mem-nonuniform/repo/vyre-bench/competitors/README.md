# vyre-bench-competitors

CPU competitor adapters for the Vyre release benchmark suite.

```bash
cargo run -p vyre-bench -- run --suite release --backend cuda
```

The crate is not published independently; it pins the conventional CPU libraries used to prove Vyre's GPU release workloads against non-GPU baselines.
