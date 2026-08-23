# onBodySent/onRequestSent are dropped by wrapped handlers

Handlers passed to `dispatch()` can observe the outgoing request via
`onBodySent(chunk)` and `onRequestSent()`. On a bare `Client` they fire —
but composing ANY interceptor silences them:

```js
let client = new Client(origin).compose(interceptors.retry())
client.dispatch(opts, {
  onBodySent (chunk) { /* never called */ },
  onRequestSent () { /* never called */ },
  ...
})
```

(Upstream report: nodejs/undici#5695.) The handler wrapper the
interceptors are built on swallows `onBodySent` (an empty method) and
doesn't forward `onRequestSent` at all, and the redirect/retry/cache
handlers have the same gap.

Expected: the two hooks are forwarded to the wrapped handler — through
`DecoratorHandler` itself and through the built-in interceptors (retry,
redirect, cache) — with the same chunks and call counts a bare `Client`
delivers. Response flow through wrapped handlers is unchanged.
