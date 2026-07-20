#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    drop(vyre_foundation::serial::wire::decode::from_wire(data));
});
