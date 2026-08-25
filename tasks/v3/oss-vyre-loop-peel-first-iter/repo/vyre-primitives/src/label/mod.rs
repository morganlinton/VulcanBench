//! Tier 2.5 label → NodeSet resolver.
//!
//! Given a node-tags buffer (each word = tag bitmask over a
//! registered TagFamily) and a family-mask constant, emit a NodeSet
//! bitset marking every node whose tag mask intersects the family.
//!
//! Downstream analyzer's `@family` lookup lowers to one dispatch of this
//! primitive. Labels themselves live in TOML and are merged into a
//! single per-node tag bitmap during host-side scan; once that tag
//! buffer is on device, every `@shell_family`, `@network_sink`, …
//! reference reuses the same resolver with a different mask constant.

pub mod resolve_family;
