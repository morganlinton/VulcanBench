# Typed schema evolution with bounded transactional decoding

Complete Decoder for this schema registry, wire encoder, archive adapter and
versioned quote clients. Keep all supplied .mli files unchanged and retain the
encoder's existing bytes. No Obj.magic, unsafe casts, Marshal, or erased dynamic
values. Generic Record decoding must produce its heterogeneous typed field tuple
before calling its constructor, rather than special-casing Domain.quote.

The wire payload is fully specified: Bool is exactly one byte 0 or 1. Int64 is
exactly eight little-endian bytes (bin_prot int64_bits). Text is arbitrary opaque
string bytes. Pair is two u32-length-prefixed payloads. Option is tag 0 alone or
tag 1 followed by one u32-length-prefixed payload. List is a u16 count followed
by that many u32-length-prefixed payloads. Record is a u16 count followed by
[u16 field-id, u32 length, payload] entries. All headers are unsigned little
endian. The encoder sorts record IDs; the decoder accepts any entry order.
IDs need only be unique within each record; unknown IDs are skipped as opaque
payloads, while duplicate IDs (including unknown ones) are errors. Required
fields missing from a record are errors. Default values apply only when a field
is absent, never when its present payload is malformed. Nested records, lists,
pairs and options obey exactly the same rules. A supplied record constructor can raise; cursor rollback still applies.
Known payloads must be consumed
exactly; extra bytes produce Trailing at the first unused byte. A bad Option
tag or Bool byte produces Invalid_scalar. Schema.validate must run before any
cursor effect; duplicate, zero or out-of-range schema IDs raise Invalid_argument.

decode consumes exactly the slice [!pos_ref, limit). Success commits pos_ref
to limit. Every returned Error and every exception leaves pos_ref unchanged.
Bounds must satisfy 0 <= start <= limit <= String.length data, otherwise return
Invalid_bounds at start with an empty path. Never read beyond the supplied slice.
Errors carry absolute string offsets, a path from outermost to innermost, and
the stated reason. Entering a record field appends Field id; list elements append
Index (zero-based), pairs Left/Right, and a present option Some_value. A missing
field is reported at the end of its enclosing record with Field id appended,
and fields are checked in schema declaration order. Duplicate_field id is at
the second entry's ID header with Field id appended. An invalid scalar is at
its payload start. Truncated is at the start of the header or scalar whose
required bytes are unavailable, using the path to that value. A frame length
past its enclosing slice produces Truncated at its length header, with that
field/element/component path. Text accepts any bytes, including an empty slice.

A record containing a 2 MiB unknown field and one known Int64 must decode
without copying or materializing the unknown payload: less than 65536 bytes
allocated during decoding on native OCaml. All counted headers and nested
boundaries still require validation. Public migration.ml shows the legacy quote
default; run it after implementing the decoder. This is an original wire
format using public bin_prot primitives, not a claim about an upstream defect.
