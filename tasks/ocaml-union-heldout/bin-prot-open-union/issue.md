# Typed open union codecs

Implement Union in src/union.ml using its supplied GADT interface, and integrate
it with ordinary bin_prot writers/readers. The complete Tagged and Tagged_stream
implementation is supplied as a dependency. Add helpers as needed. Do not change
supplied interfaces, exports, Tagged/Tagged_stream/Utils, build definitions or
public/upstream fixtures. No unsafe casts or global mutable caches.

Wire format is exactly a nonzero u16 tag, a u32 payload byte count, then payload,
all unsigned little endian. Known payloads use Tagged.encode/decode for that
case's existential codec. Unknown payloads are opaque. The tag range is 1..65535.
Closed rejects unknown tags; Open forwards them through its unknown callbacks.

validate checks all top-level tag ranges/duplicates first, then every nested
Tagged schema, in declaration order. Every encode/decode/reader/writer operation
validates before arguments, bounds, projectors or any user callback. Empty case
lists are valid. Constructing reader/writer/type_class need not validate.

Encoding runs each known projector exactly once, in declaration order, even
after one matches. Only after all projectors return, decide the selection.
Exactly one match is encoded, then no unknown projector runs. Multiple matches
raise Invalid_argument before any payload encoder. With no match, Closed raises
Invalid_argument; Open invokes project_unknown exactly once. None, a zero or
out-of-range tag, or collision with ANY known tag raises Invalid_argument.
An unknown payload is copied into the output; later input mutations cannot
change the encoded result. Payloads exceeding u32 length raise Invalid_argument.
Do not sort projectors. User exceptions propagate by physical identity.

Direct decode consumes exactly [entry cursor, limit). Validate first, then
reject negative cursor, limit before cursor or limit beyond buffer dimension as
Error {at=entry; reason=Invalid_bounds}. Bounds precede wire inspection.
A slice shorter than the complete 6-byte header returns Truncated at entry.
For a full header, tag zero returns Invalid_tag at entry before payload bounds.
A payload length greater than the remaining slice returns Truncated at entry+6.
A smaller payload than remaining returns Trailing at entry+6+length. These wire
checks precede all payload readers/injectors and unknown dispatch. A closed
unknown tag returns Unknown_tag tag at entry after structural checks.

For known cases, call Tagged.decode with the ORIGINAL buffer, an independent
local cursor initially entry+6 and limit entry+6+length. Wrap returned Tagged
errors as Payload e at e.at, retaining the complete nested error/path. Only
successful exact decoding calls that case's injector. Open unknown decoding
passes inject_unknown an exact zero-copy Bigarray subview of the payload.
No payload copy, string materialization or allocation proportional to an opaque
unknown payload is allowed: decoding a 2 MiB unknown must allocate less than
65536 bytes on the OCaml heap. The returned view aliases the input.

On success commit the caller cursor to the slice end, overriding changes by
callbacks. On EVERY returned error or user exception restore the entry cursor;
propagate every exception unchanged by physical identity, including Union.Decode
and Tagged.Decode. Do not reinterpret callback-raised public parser exceptions
as returned parser errors. No rollback of buffer/external side effects is required.
This transaction also applies to reader.read.

writer.size reports exact encoded length. writer.write writes the same bytes
at its supplied offset, checks capacity before copying, returns the first byte
after output and leaves bytes outside output untouched. Negative position or
insufficient capacity raises Invalid_argument. Size/write are independent calls,
so callbacks may run once per operation. No cross-call memoization.

reader.read decodes ONE frame at its caller position, allowing subsequent frames
in the same buffer. It checks header/tag/payload truncation using buffer extent,
then invokes the exact decoder on that frame. It translates returned errors to
Union.Decode, preserving cursor rollback; callbacks raising exceptions propagate
unchanged. reader.vtag_read validates schema then raises Invalid_argument without
changing cursor or invoking callbacks. type_class uses the supplied shape verbatim
with matching writer and reader. These must compose as Tagged.Atom values and
work with Utils.bin_dump, Utils.bin_read_stream, and Tagged_stream.dump/decode_next,
including empty unknown payloads and trailing concatenated frames.

The native preflight builds the library, upstream fixture checksums and public
clients. The upstream test library requires float_array, absent from the pinned
environment, so it is preserved but not built. Keep native toolchain unchanged.
