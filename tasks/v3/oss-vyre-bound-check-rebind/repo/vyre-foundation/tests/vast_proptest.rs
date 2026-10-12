//! Random buffers must not panic `vast::validate_vast` / header decode.

use proptest::prelude::*;
use vyre_foundation::vast::{validate_vast, VastHeader, HEADER_LEN, VAST_MAGIC, VAST_VERSION};

proptest! {
    #[test]
    fn random_bytes_validate_or_err(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let validation = validate_vast(&bytes);
        let header = VastHeader::decode(&bytes);

        if bytes.len() < HEADER_LEN {
            prop_assert!(header.is_err());
            prop_assert!(validation.is_err());
        }
        if bytes.len() >= HEADER_LEN
            && bytes[0..4] == VAST_MAGIC
            && u16::from_le_bytes([bytes[4], bytes[5]]) == VAST_VERSION
        {
            prop_assert!(header.is_ok());
        }
    }
}
