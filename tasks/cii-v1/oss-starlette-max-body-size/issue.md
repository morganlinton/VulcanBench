# Add configurable request body size limits

There is no way to bound the size of an incoming HTTP request body. An
application that reads `await request.body()` or parses a form will happily
buffer whatever a client sends, so a single large upload can exhaust memory.

Add a `max_body_size` option (bytes) configurable at every level of the
application, plus a middleware that provides the same behavior to arbitrary
ASGI apps:

```python
app = Starlette(routes=[...], max_body_size=1_000_000)

Route("/upload", endpoint, methods=["POST"], max_body_size=5_000_000)
Mount("/api", app=sub_app, max_body_size=100_000)

RequestBodyLimitMiddleware(some_asgi_app, max_body_size=1_000)
```

Requirements:

- Exceeding the limit produces a `413` response instead of the endpoint's;
  under-limit requests are unaffected.
- The limit counts the **actual body bytes received from the ASGI server**,
  not a declared `Content-Length` — a streamed/chunked body with no such
  header must still be cut off at the limit.
- Multipart uploads count toward the limit too, and files opened while
  parsing must still be cleaned up when the limit aborts a parse.
- The default (`None`) imposes no limit.

Nothing else may change: requests without a limit still read arbitrarily
large bodies, urlencoded and multipart form parsing behave as before,
streaming request bodies and streaming responses still work, and the
middleware stack keeps its ordering so exception handlers and background
tasks are unaffected.
