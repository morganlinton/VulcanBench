# escargot

> **Cargo API written in Paris**

[![codecov](https://codecov.io/gh/crate-ci/escargot/branch/master/graph/badge.svg)](https://codecov.io/gh/crate-ci/escargot)
[![Documentation](https://img.shields.io/badge/docs-master-blue.svg)][Documentation]
![License](https://img.shields.io/crates/l/escargot.svg)
[![Crates Status](https://img.shields.io/crates/v/escargot.svg)][Crates.io]

## Why escargot

Compared to depending on `cargo`:
- Faster compile times.
- Simpler API.
- Better interop with projects relying on other cargo versions.
- Probably slower execution, especially on platforms without an optimized `fork` (e.g. Windows).

## Relevant crates

Other related crates:
* [cargo](https://crates.io/crates/cargo) for the real thing
* [cargo-metadata](https://crates.io/crates/cargo_metadata) for a similar project specifically geared to the `metadata` subcommand.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual-licensed as above, without any additional terms or
conditions.

[Crates.io]: https://crates.io/crates/escargot
[Documentation]: https://docs.rs/escargot
