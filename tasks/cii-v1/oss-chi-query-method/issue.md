# Support the HTTP QUERY method (RFC 10008)

The HTTP QUERY method — a safe, idempotent method that conveys a request
body — is not supported by the router. There is no way to register a QUERY
route, and `MethodFunc("QUERY", ...)` rejects the method as unknown.

Wanted:

```go
r := chi.NewRouter()
r.Query("/search", func(w http.ResponseWriter, r *http.Request) { ... })
r.MethodFunc("QUERY", "/reports", handler) // works too
```

- `Query(pattern, handler)` on the router (and the `Router` interface),
  alongside the existing per-method helpers.
- QUERY requests dispatch to QUERY handlers and are distinct from GET on
  the same route; the request body is readable in the handler.
- QUERY participates in method-not-allowed handling: a 405 response's
  `Allow` header lists QUERY alongside the other methods registered on the
  route.
- Existing routing (GET/POST helpers, URL params, 404/405 behavior) is
  unchanged.
