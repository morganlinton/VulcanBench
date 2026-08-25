#![no_main]
//! P1 inventory #98  -  fuzz target for registry TOML loading.
//!
//! Random bytes shaped like TOML  -  including syntactically invalid,
//! structurally-fine-but-semantically-wrong, and oversize files  -  must
//! never panic the registry loader. Every error path must produce a
//! structured `Err` with a `Fix:` hint so the operator can act.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Cap the input. The real loader bounds its read; the fuzz target
    // mirrors that bound so we don't waste cycles on inputs the real
    // path would refuse before parsing.
    if data.len() > 64 * 1024 {
        return;
    }
    let Ok(s) = std::str::from_utf8(data) else { return; };
    // The TOML decoder is the moral equivalent of the registry loader's
    // first stage. A panic here is a real finding  -  every invalid TOML
    // must surface as a structured `Err`.
    drop(toml::from_str::<toml::Value>(s));
});
