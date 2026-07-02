Reading the body of a protocol-upgrade request is broken: the body reads back empty, and everything sent after the request headers is treated as data for the upgraded protocol.

A client is allowed to send an HTTP/1.1 request that both asks for a protocol upgrade and carries a body, for example:

```
GET /ws HTTP/1.1
Host: example
Connection: Upgrade
Upgrade: websocket
Content-Length: 13

foobarbaz
```

Per RFC 9110 section 7.8, a server that accepts such an upgrade switches protocols only after the complete request body has been read. Today the parser flips into upgraded ("tunnel") mode as soon as it sees the upgrade headers: the declared body is never delivered to the payload reader (reading it returns nothing), and the body bytes themselves leak into the opaque post-upgrade data stream. The same happens for chunked bodies.

Expected behavior: for an upgrade request with a body (Content-Length or chunked), the body must be fully readable through the normal payload interface, and only the bytes after the end of the body belong to the upgraded protocol. An upgrade request without a body must keep switching immediately, and ordinary (non-upgrade) request parsing must be unchanged.

Note that the HTTP parser has two implementations that must stay in sync: the pure-Python one and the Cython one (`_http_parser.pyx`). Fix both.
