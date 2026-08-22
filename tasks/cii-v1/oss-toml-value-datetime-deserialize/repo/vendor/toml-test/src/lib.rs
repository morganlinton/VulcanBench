//! Verify Rust TOML parsers
//!
//! For TOML test cases, see [`toml-test-data`](https://docs.rs/toml-test-data).
//!
//! To run the test cases against your TOML implementation, see [`toml-test-harness`](https://docs.rs/toml-test-harness).

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

mod decoded;
mod error;
mod verify;

pub use decoded::DecodedScalar;
pub use decoded::DecodedValue;
pub use error::Error;
pub use verify::Command;
pub use verify::Decoder;
pub use verify::Encoder;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
