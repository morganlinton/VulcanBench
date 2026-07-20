# vyre-foundation-fuzz

Fuzz targets for the Vyre foundation decoder, program wire format, and registry TOML loader.

```bash
cargo fuzz run decoder
cargo fuzz run program_wire
cargo fuzz run registry_toml
```

The package is publish=false because it is release verification infrastructure, not a crates.io runtime crate.
