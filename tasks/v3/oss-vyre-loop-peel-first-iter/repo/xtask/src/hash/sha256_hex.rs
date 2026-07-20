use super::sha256;

/// Return a full 256-bit SHA-256 digest as lowercase hex.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha256(bytes);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out
}
