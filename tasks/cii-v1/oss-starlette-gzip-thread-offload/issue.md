# Offload large GZip compression to a worker thread

Compressing a large response body blocks the event loop: `GZipMiddleware`
does the whole compression inline, so a single multi-hundred-kilobyte
response stalls every other request on that worker for the duration.

Move large compression work off the event loop:

- Bodies at or above a threshold are compressed in a worker thread; smaller
  bodies keep being compressed inline, where the thread hand-off would cost
  more than it saves.
- The threshold is configurable as a `thread_minimum_size` setting on
  `GZipMiddleware`, defaulting to 128 KiB.
- Use a dedicated, private, per-event-loop capacity limiter (40 tokens,
  matching AnyIO's default worker-thread concurrency) so compression does
  not compete with the shared thread pool that application code uses.

Compression behavior itself must not change: bodies round-trip byte for
byte at every size, streaming responses still stream, `minimum_size` still
decides what gets compressed at all, clients that do not accept gzip get
identity responses, and the `Vary` header is still set.
