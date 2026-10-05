# Typed tagged decoding and transactional bin_prot streams

Complete Tagged.decode, Tagged.reader and Tagged_stream.decode_next in this full
bin_prot v0.17.0 checkout. Keep every supplied .mli, the schema types, encoder,
writer, existing bin_prot interfaces and public/upstream test fixtures unchanged.
The new modules are exported from Bin_prot. No Obj.magic, Obj casts, Marshal,
erased dynamic values or concrete-domain special cases. Record must decode a
heterogeneous tuple with the supplied GADT fields before invoking its constructor.
The public migration client demonstrates old records decoded by a newer schema.

This is an original tagged extension, not an upstream bug or a replacement for
standard bin_prot encodings. The supplied encoder and writer define stable bytes.
Atom stores exactly its Type_class writer's payload. Pair stores left and right,
each prefixed by a u32 byte length. Option stores tag 0 alone or tag 1 plus a
u32-prefixed payload. List stores a u16 count then u32-prefixed elements. Record
stores a u16 count then [u16 field ID, u32 byte length, payload] entries. All
headers are unsigned little endian. Map has its underlying schema's bytes.
The encoder sorts record IDs; decoding accepts any order. IDs are unique within
each record. Unknown fields are opaque, but their lengths and duplicate IDs are
validated. Default applies only to absent fields, never to malformed present
fields. IDs zero and 65536 or larger are invalid in schemas, including nested
schemas. validate runs before any buffer/cursor effect or user reader/constructor.

Tagged.decode consumes exactly [!pos_ref, limit) of Common.buf. Bounds require
0 <= start <= limit <= buffer length; otherwise return Invalid_bounds at start
with empty path. Success sets pos_ref to limit, overriding any callback changes.
Every returned Error and every propagated exception restores its entry value.
This includes exceptions from Atom readers, Record constructors and Map functions,
even when they capture and mutate the caller's cursor. Propagate the same exception
value, including a user-raised Tagged.Decode; do not convert user exceptions into
returned parse errors. User buffer mutations and external side effects are not
rolled back. Reentrant calls with independent cursors are permitted.

Atom readers receive a zero-copy Bigarray subview limited to exactly their payload,
with a fresh local pos_ref initially zero. They cannot see enclosing headers or
bytes past their slice. They must consume the slice exactly: a successful reader
leaving a position from 0 to length-1 returns Trailing at payload-start plus that
position; a successful reader returning a negative or greater-than-length position
returns Invalid_bounds at payload start. Exceptions from the reader propagate
unchanged. Do not translate bin_prot exceptions or attempt to recover Atom values.

Returned structural errors have absolute buffer offsets and paths from outermost
to innermost. Entering a field appends Field_id id, list element Index i (zero
based), pair Left/Right, present option Some_value, and Map's child Mapped.
Invalid_tag is at the option tag. Truncated is at the start of a required header
whose bytes are unavailable. A length past its enclosing slice is Truncated at
that length header, with the child's path. Trailing is at the first unused byte.
Missing_field id is at the enclosing record's end, with Field_id id appended;
missing/known fields are visited in schema declaration order. Duplicate_field id
is at the second ID header with Field_id id appended. For a duplicate, report it
as soon as the ID is available, even if its length header is malformed.

Within each composite, validate its own trailing bytes before its Record constructor
or Map callback. For Record, first scan all entry headers, validate lengths and
duplicates and exact record exhaustion; only then decode known fields in declaration
order and construct the record. For Pair/List/Option, decode children in wire order
and check enclosing exhaustion after children. Map checks its entire underlying
payload before invoking its mapping callback. A later outer error need not undo
side effects of earlier successfully decoded children. Empty records/lists, empty
Atom payloads and absent options remain legal when their schemas allow them.

A record with a 2 MiB unknown field and one small known Atom must allocate less
than 65536 OCaml heap bytes during native decoding. Index offsets/subviews rather
than copying the whole buffer or unknown payload. Allocation before decode is not
counted. Arbitrary user reader allocation is not constrained by this requirement.

Tagged.reader returns a Type_class.reader. Its read decodes from the input cursor
to the buffer's end under the same transaction rules, raises Tagged.Decode for a
returned structural error, and propagates user exceptions unchanged. vtag_read
raises Invalid_argument without changing its supplied cursor. Tagged.writer
and Tagged.reader must work with existing Utils.bin_dump and Utils.bin_read_stream.

Tagged_stream.dump already uses the standard Utils 8-byte signed little-endian
size header followed by a tagged payload. decode_next reads at most one such frame
inside [start, limit), commits only through that frame, and leaves subsequent
frames untouched. It validates the schema first, then rejects negative max_size
with Invalid_argument, then validates bounds. Fewer than 8 header bytes or a
nonnegative, permitted length whose payload is incomplete returns Ok None without
calling readers/constructors or moving the cursor. A negative length or a length
larger than OCaml max_int returns Invalid_frame_size at the header with empty path.
A representable length greater than max_size returns Frame_too_large at the header,
even if its payload is incomplete. Check lengths by subtraction before addition.
A complete frame delegates to the same typed bounded decoder, returns Ok (Some v),
and commits through that frame; errors use absolute offsets in the original buffer.
All returned errors and all exceptions restore the entry cursor exactly.

Run dune build --profile release src/bin_prot.cmxa @runtest for public checks.
The upstream test library needs float_array, which is absent from this pinned
native environment. Preserve it; the public preflight runs the upstream generated
fixture checksum rule and the supplied clients, not every upstream inline test.
