# Add typed framing with an incremental stream decoder

Preserve the GADT API in Wire and implement Stream. A frame is a two-byte
unsigned big-endian body length, followed by tag (one byte), sequence (four-byte
unsigned big-endian), then payload. Tag 0 is Ping with no payload; tag 1 is Data
with 0 to 64 arbitrary bytes. Sequence accepts 0 to 4294967295 inclusive.
`encode` returns `Error "bad sequence"` outside that range or `Error "bad payload"`
for oversized Data, otherwise canonical bytes. Ping at sequence 1 encodes as
hex 00050000000001. Data "A" at sequence 2 encodes as 0006010000000241.

`decode_body` parses a body without the length prefix. Too-short bodies or invalid
payload lengths return `Error "bad payload"`; an unrecognized tag in a body of
at least five bytes returns `Error "unknown tag"`. Known sequence bytes decode
as unsigned int64. Preserve the typed Ping and Data results, not unsafe casts.

`feed` accepts arbitrary chunk boundaries and emits every complete frame in order,
retaining only an unfinished suffix. Body lengths outside 5 to 69 cause one
`Error "bad length"`, discard all pending bytes and poison the stream: further
feeds emit nothing until reset. A legal length with an invalid tag or payload
emits the body error, consumes exactly that frame, then continues with subsequent
frames. Empty feeds are allowed. `finish` returns `Error "bad length"` when
poisoned, `Error "truncated frame"` with an unfinished suffix, or `Ok ()` when
clean. `reset` clears both states. Keep supplied interfaces and public tests.

The supplied `.mli` files are fixed contracts for this implementation task.
Keep their contents unchanged; implement the change in `.ml` files.
