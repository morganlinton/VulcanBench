# RegExpRouter: wildcard middleware is not associated with matching routes

`RegExpRouter` fails to associate wildcard middleware with registered routes
in two situations where the other routers do associate them:

1. A suffix wildcard without a preceding slash. Middleware registered as
   `/assets*` is not invoked for a route registered as `/assets/app.js` —
   the `*` appears to be treated as a literal character when the router
   decides which routes the middleware covers.

```ts
const router = new RegExpRouter<string>()
router.add('POST', '/assets*', 'middleware')
router.add('POST', '/assets/app.js', 'handler')
router.match('POST', '/assets/app.js') // only 'handler'; 'middleware' missing
```

2. A trailing wildcard after a parameter, when another route uses a
   different parameter name at the same position. Middleware registered as
   `/:name/*` should be associated with a route registered as `/:id` (and
   each handler should still receive parameters under its own registered
   name). The same holds for custom patterns, including nested braces and
   regexp metacharacters:

   - `/:year{[0-9]{4}}/*` alongside `/:yr{[0-9]{4}}/comments`
   - `/:kind{(?:foo|bar)}/*` alongside `/:type{(?:foo|bar)}/detail`
   - `/:userId{[^/]+}/*` alongside `/:id/profile`

In both cases the association must hold regardless of registration order,
with handlers returned in registration order and each receiving its own
parameter names.

Existing matching behavior must not change: `/assets/*` style middleware,
wildcard matching of sub-paths, static and param routes, and non-matching
paths all behave as today.
