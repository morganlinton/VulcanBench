`middleware.WrapResponseWriter` reports double the actual response size when a Tee writer is set and the response body is written through the `io.ReaderFrom` fast path.

chi's logging and metrics middlewares rely on `WrapResponseWriter.BytesWritten()` for the response size. When a handler sends the body with something that uses the `io.ReaderFrom` fast path (for example `http.ServeContent`, `http.ServeFile`, or an `io.Copy` to the response writer) AND a Tee writer has been attached (as the response-capture middlewares do), `BytesWritten()` comes back exactly doubled: an 11-byte body is reported as 22 bytes.

The body itself is delivered correctly, and the tee buffer receives the correct copy; only the byte accounting is wrong, which corrupts access logs and response-size metrics for any route serving files or streamed content behind a tee-ing middleware.

Fix the byte accounting so `BytesWritten()` reports the true body size exactly once in this case. The `ReadFrom` fast path without a tee, and ordinary `Write`-based responses, must keep behaving exactly as they do today.
