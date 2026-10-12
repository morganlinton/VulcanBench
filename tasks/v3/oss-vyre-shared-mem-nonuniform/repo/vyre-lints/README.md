# vyre-lints

`vyre-lints` enforces architectural boundaries inside the Vyre workspace.

It catches raw IR construction where dialect crates should use the shared
builder patterns instead. The crate ships as a library plus a small CLI so CI
can run the same checks locally and in release gates.

## Usage

```sh
cargo run -p vyre-lints -- path/to/crate
```

## Release role

This crate is publishable because downstream Vyre crates and CI jobs need the
same lint contract without depending on a private workspace checkout.
