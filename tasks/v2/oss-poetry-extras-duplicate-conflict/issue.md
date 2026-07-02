Dependency resolution fails with a spurious conflict when the same package appears in more than one place in a project's dependency configuration.

Concretely: a project declares package `A` as an optional dependency in two different extras (say `alchemy` and `databases`), and additionally requires `A` with one of `A`'s own extras (say `A[mypy]`) in a dependency group such as `test`. Locking such a project fails with an error of the form:

```
Because root depends on both A (*) and A (<empty>), version solving failed.
```

There is no real conflict here: every declared requirement on `A` is satisfiable by the same version, and resolution should succeed and install `A` once. The `A (<empty>)` constraint in the error message does not correspond to anything the user wrote.

Fix the dependency solver so that this configuration (duplicate optional dependencies of the same package across extras, combined with a group dependency on that package with extras) resolves successfully. Ordinary dependency resolution must be unaffected.
