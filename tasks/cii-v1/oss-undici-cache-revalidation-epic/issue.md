# Cache interceptor: conditional revalidation never engages, and writes leave named entries stale

Two related gaps in the cache interceptor's RFC 9111 conformance.

**1. Responses that demand revalidation are never stored.** The canonical
"cache, but always validate" pattern for APIs — a response carrying an
`ETag` (or `Last-Modified`) together with `Cache-Control: no-cache`, or
`max-age=0` with no other freshness source — is never entered into the
cache. As a result no `If-None-Match` is ever sent, the origin can never
answer `304 Not Modified`, and every request pays for a full `200` with the
complete body. Expected: the response is stored, subsequent requests carry
the validator, and a `304` is answered from the stored body.

**2. Writes do not invalidate the entries they name.** After the classic
`POST /collection` → `201 Created` + `Location: /collection/123` flow, a
previously cached `GET /collection/123` keeps being served stale. RFC 9111
§4.4 says a cache SHOULD also invalidate the URIs in a response's
`Location` and `Content-Location` header fields when a non-error response
to an unsafe method names them — with relative references resolved against
the request URI, and only when the named URI shares the request URI's
origin (a cross-origin `Location` must not touch the other origin's
entries).

Existing behavior must not regress: fresh responses keep being served from
cache, an unsafe method still invalidates its own request URI, `no-store`
responses are never cached, and cross-origin entries are never invalidated
by another origin's responses.
