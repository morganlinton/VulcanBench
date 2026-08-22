# Add an RFC 9457 "Problem Details" HTTP error handler

APIs increasingly standardize on RFC 9457 (Problem Details for HTTP APIs,
`application/problem+json`). Echo should ship an opt-in error handler for it:

```go
e := echo.New()
e.HTTPErrorHandler = echo.ProblemDetailsHTTPErrorHandler(exposeError)
```

Wanted:

- A `ProblemError` type carrying the RFC 9457 members (`type`, `title`,
  `status`, `detail`, `instance`; the last two omitted from JSON when empty).
  It must satisfy `error`, and expose its status code the way other Echo
  errors do. Zero values are defaulted when the response is built: `type` to
  `"about:blank"`, `status` to 500, `title` to the standard status text.
- A `ProblemErrorer` interface (`ProblemError() *ProblemError`) so custom
  error types can convert themselves; a nil conversion falls back to the
  defaults.
- `ProblemDetailsHTTPErrorHandler(exposeError bool)` returning an
  `HTTPErrorHandler` that renders every error as a problem-details JSON
  response with the `application/problem+json` content type:
  - a `*ProblemError` anywhere in the error chain is used as-is;
  - otherwise a `ProblemErrorer` in the chain is converted;
  - otherwise the response is built from the error's status code (500 when
    it has none); an `*HTTPError`'s message becomes `detail`, and
    `exposeError` decides whether wrapped/underlying error text is appended
    (or, for plain errors, included at all).
- `HEAD` requests get the status code with no body. An already-committed
  response is left alone.
- A `MIMEApplicationProblemJSON` constant alongside the existing MIME types.

The default error handler stays the default; existing routing and JSON
responses are unchanged.
