use super::{MAGIC, WIRE_FORMAT_VERSION};
use crate::serial::wire::{Reader, MAX_STRING_LEN};

impl<'a> Reader<'a> {
    #[inline]
    pub(crate) fn expect_magic(&mut self) -> Result<(), String> {
        if self.bytes.len() < MAGIC.len() || &self.bytes[..MAGIC.len()] != MAGIC {
            return Err(
                "invalid IR wire-format header. Fix: serialize Program with Program::to_wire()."
                    .to_string(),
            );
        }
        self.pos = MAGIC.len();
        // L.1.47: consume and validate the schema version immediately
        // after the magic. Only the compiled-in version is accepted;
        // newer blobs surface an actionable error pointing the caller
        // at the version mismatch rather than producing an opaque
        // downstream tag failure.
        if self.bytes.len() < self.pos + 2 {
            return Err(
                "IR wire-format truncated before schema version. Fix: serialize Program with Program::to_wire()."
                    .to_string(),
            );
        }
        let version = u16::from_le_bytes([self.bytes[self.pos], self.bytes[self.pos + 1]]);
        if version != WIRE_FORMAT_VERSION {
            return Err(format!(
                "IR wire-format version {version} is not supported by this decoder (expects {WIRE_FORMAT_VERSION}). Fix: upgrade the consumer or re-serialize with a compatible Program::to_wire()."
            ));
        }
        self.pos += 2;
        Ok(())
    }

    #[inline]
    pub(crate) fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(len).ok_or_else(|| {
            "IR wire-format offset overflow. Fix: provide a valid VIR0 Program blob.".to_string()
        })?;
        if end > self.bytes.len() {
            return Err(
                "truncated IR wire format. Fix: provide the complete Program bytes.".to_string(),
            );
        }
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    #[inline]
    pub(crate) fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    #[inline]
    pub(crate) fn u32(&mut self) -> Result<u32, String> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    #[inline]
    pub(crate) fn u64(&mut self) -> Result<u64, String> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    #[inline]
    pub(crate) fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.u32()?.to_le_bytes()))
    }

    #[inline]
    pub(crate) fn bounded_len(&mut self, max: usize, label: &str) -> Result<usize, String> {
        let raw = self.u32()?;
        let value = usize::try_from(raw).map_err(|error| {
            format!("Fix: {label} {raw} cannot fit usize ({error}); decode this wire blob on a supported target or reject it.")
        })?;
        if value > max {
            return Err(format!(
                "Fix: {label} {value} exceeds IR wire-format limit {max}; split the Program or reject this untrusted blob."
            ));
        }
        Ok(value)
    }

    #[inline]
    pub(crate) fn string(&mut self) -> Result<String, String> {
        let len = self.bounded_len(MAX_STRING_LEN, "string length")?;
        let bytes = self.take(len)?;
        // VYRE_IR_HOTSPOTS CRIT (impl_reader.rs:92): previous code did
        // `bytes.to_vec()` then `String::from_utf8(vec)`, allocating an
        // intermediate `Vec<u8>` for every string decode. Using
        // `std::str::from_utf8` validates the borrowed slice directly;
        // `str::to_owned` copies once into the final `String`.
        std::str::from_utf8(bytes).map(str::to_owned).map_err(|err| {
            format!("invalid UTF-8 in IR wire-format string: {err}. Fix: serialize valid Rust String values.")
        })
    }

    /// Take `len` raw bytes from the reader, used by opaque extension
    /// payload decoding.
    #[inline]
    pub(crate) fn bytes(&mut self, len: usize) -> Result<Vec<u8>, String> {
        Ok(self.take(len)?.to_vec())
    }
}
