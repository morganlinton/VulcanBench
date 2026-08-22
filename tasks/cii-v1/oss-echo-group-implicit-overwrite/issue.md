# Group.Use panics when the router disallows route overwriting

Adding middleware to a group registers implicit "route not found" routes for
the group's prefix (so the middleware also runs for unmatched paths). Those
implicit registrations collide with themselves: with a router configured as
`RouterConfig{AllowOverwritingRoute: false}`, calling `Use` on a group more
than once — or creating the group with middleware and calling `Use` later —
panics, because the second registration tries to overwrite the first's
implicit routes.

```go
e := echo.NewWithConfig(echo.Config{
    Router: echo.NewRouter(echo.RouterConfig{AllowOverwritingRoute: false}),
})
g := e.Group("/api")
g.Use(mw1)
g.Use(mw2) // panics
```

Expected: the implicitly registered group routes are always allowed to be
overwritten — they are bookkeeping, not user routes — so repeated `Use`
calls (including on nested groups, and on groups created with initial
middleware) work under `AllowOverwritingRoute: false`, and all added
middlewares apply to requests.

Explicit user routes keep their protection: re-registering an explicit
route with overwriting disallowed still fails. Group middleware still runs
for unmatched paths inside the prefix, and group routing is unchanged.
