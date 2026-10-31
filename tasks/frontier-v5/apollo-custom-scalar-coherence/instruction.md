# Custom scalars are only half supported across Apollo Client

The workspace at `/app` is Apollo Client 4.3 (TypeScript, run from source by
jest) with its dependencies installed. Run the test suite the usual way:

```
npm test
```

or a single file, for example
`npm test -- src/cache/inmemory/__tests__/scalars.ts`. The suite passes as
given, apart from a few environment-dependent upstream tests that fail or
flake on this machine with or without any change (the `FinalizationRegistry`
ponyfill tests, which depend on garbage-collection timing, and one
`HttpLink` multipart case over whatwg stream bodies). Those are not graded.

Apollo Client 4.3 introduced custom scalars: a `Scalar` with `parse`,
`serialize` and `is`, registered with `new InMemoryCache({ scalars })` and
attached to fields through a field policy's `scalar` option (and to input
object fields through the cache's `inputObjects` configuration), so that the
cache stores and returns parsed values (a `Date`, a `bigint`, a `Map`) while
the network and `cache.extract()` see serialized ones. The repository also
ships a GraphQL Codegen plugin (`codegen/custom-scalars`) that writes this
configuration from a schema. Users report that the feature breaks as soon as
they leave the simplest path. Fix the underlying causes so that all of the
following holds. Do not special-case the examples; they illustrate classes of
inputs.

## 1. Network results skip `read` functions and scalar parsing

Two watched queries select overlapping parts of the same non-normalized
object (no `id`, or a field policy with `merge: false`). Each write replaces
the object and drops the fields the other query needs, so the queries refetch
each other. To stop that loop the client currently refuses to write a network
result that is identical to the one it last wrote for the query. The
consequence: the query then delivers the raw network result, reported as
`dataState: "complete"`, without its field `read` functions or custom scalar
parsing applied (a `Date` field arrives as a string), and the clobbered data
is never repaired in the cache. In other setups (a `read` function that keeps
a result partial) the same pair of queries refetches forever.

Required behaviour:

- Every network result is written to the cache, and what a cache-backed query
  delivers is read back from the cache, so `read` functions and scalar
  parsing always apply. A refetched result that reads back as partial warns
  about the incomplete cache read again (the existing development warning),
  instead of being silently skipped.
- The endless loop is stopped at the automatic refetch instead. A watched
  query whose cache result becomes incomplete still refetches. If, after
  that, its cache result becomes incomplete again with the same missing
  fields and the same variables, and nothing destructive (`cache.evict`,
  `cache.modify`, `cache.reset`) happened to the cache in between, it does
  not refetch again. In that state a query whose current result is already
  partial (it uses `returnPartialData`) still receives each updated partial
  cache result; a query is never downgraded from a complete result to a
  partial one this way. Queries with the `cache-only` fetch policy never
  fetch and are not affected.
- The stopped state is reset by a complete cache result, a different set of
  missing fields, different variables, a destructive cache operation, and by
  any request the user starts (`refetch`, polling, `reobserve`,
  `setVariables`, and so on).
- When the client stops refetching, it warns once in development through the
  project's `invariant.warn`, with a format string that begins
  `Apollo Client stopped refetching '%s' because the same incomplete cache result was already refetched.`
  and continues with advice on fixing the feud (select an `id` or `_id`, set
  `keyFields`, add a field `merge` function) and a `missing fields: %o`
  line. The two arguments are the operation name (or `(anonymous)`) and the
  missing-field tree of the incomplete cache result. It warns once until the
  stopped state is reset, not on every later overwrite.
- While a deferred (`@defer` or `@stream`) response is still streaming, a
  cache update that leaves the query incomplete is delivered directly, with
  the query's current `networkStatus` and `loading` (for example
  `NetworkStatus.streaming`), without starting another request and without an
  extra emission that reports `NetworkStatus.loading`. Results delivered from
  an optimistic cache transaction likewise keep the query's current
  `networkStatus` and `loading` instead of forcing `ready`.

## 2. `no-cache` queries never parse scalars

With `fetchPolicy: "no-cache"` (through `client.query` and
`client.watchQuery`), results bypass the cache, and their custom scalar
fields arrive serialized. When the cache configures scalars, a `no-cache`
result must have its scalar fields parsed according to the same
configuration the cache would use: through aliases, named and inline
fragments (respecting `possibleTypes`), nested objects and lists, and root
types renamed in the type policies. Parts of the result that need no
conversion keep their identity, and nothing changes when the cache configures
no scalars.

## 3. Array-shaped scalars are impossible

Coercion walks into every array before it looks for a scalar, so a scalar
whose serialized form is itself an array (a `DateTimeRange` serialized as
`["2026-01-01T00:00:00.000Z", "2026-06-01T00:00:00.000Z"]`) never receives the
whole value. The fix is a change of contract: the `scalar` option of a field
policy and the field types in the `inputObjects` configuration use GraphQL
list syntax.

- `scalar: "DateTimeRange"` means the field's value is one scalar value, even
  when it is an array: the whole value is passed to `parse`, `serialize` and
  `is`.
- `scalar: "[DateTime]"` means a list of `DateTime` values, `"[[DateTime]]"`
  a list of lists, and so on; `"[DateTimeRange]"` is a list of array-shaped
  values, each passed whole. Non-null markers are not part of this syntax.
- Variables are coerced by the type in the operation's variable definition
  in the same way: a variable of type `[Date!]` is a list of `Date` values,
  a variable of type `DateTimeRange` is one value even when it is an array.
- This applies everywhere scalars are coerced: writes, reads, `cache.modify`,
  `cache.extract()`, `cache.restore()`, variable serialization (including
  input objects, lists of input objects and nested lists) and the `no-cache`
  path above.
- When a list type is configured but the value is not an array, the value is
  coerced as the inner scalar anyway and a development warning is issued
  through `invariant.warn` with the format string
  `The custom scalar configuration for '%s' uses list type '%s', but the value is not an array. The value was coerced as '%s' anyway.`
  and the arguments `"<Typename>.<fieldName>"`, the configured type (for
  example `"[DateTime]"`) and the inner scalar name (`"DateTime"`). For a
  variable the format string is
  `The variable '%s' has list type '%s', but the value is not an array. The value was coerced as '%s' anyway.`
  with the variable name, its list type (without non-null markers) and the
  inner scalar name.
- The codegen plugin writes the new syntax: for a schema field or input
  object field of type `[DateTime!]!` it emits `"[DateTime]"` (list wrappers
  kept, non-null markers stripped), and for a field of type `DateTime!` it
  emits `"DateTime"`. The type of the option is exported as `ScalarType` from
  `@apollo/client/cache` and `@apollo/client`.

## 4. Non-JSON-serializable scalars break identity and watching

A scalar whose parsed value cannot go through `JSON.stringify` (a `bigint`)
breaks two paths that stringify parsed values directly:

- `cache.identify` builds the entity id from key fields with a bare
  `JSON.stringify`, so an object whose key field (configured `keyFields`, or
  the default `id` or `_id`) holds a parsed `bigint` cannot be identified,
  and `cache.modify({ id: cache.identify(obj) })` silently does nothing. Key
  fields that hold scalar values must be serialized with their scalar before
  the id is built, so that the parsed and the serialized form of the same
  object identify the same entity. This covers nested key fields, key fields
  configured on a supertype, list-typed scalar key fields and `_id`. Only key
  fields are serialized, and every scalar key field is identified by its
  serialized value, including JSON-serializable ones (a `Date`) and
  object-valued ones (a `Map`). Writing an object stores the instance
  that was written, even when the scalar serializes with less precision than
  the parsed value holds.
- `cache.watch` keys its watches on raw variables and throws for such
  variables, while `read`, `write` and `diff` serialize them first. Scalar
  variables must be serialized before anything is keyed on them, including
  `keyArgs` built from field arguments and input objects, and variables
  passed to `refetch`.

## Grading

Grading copies only library source from your `src` and `codegen` trees
(tests, snapshots, `src/testing` and `src/config`, which hold the test
helpers, custom matchers and jest setup, are taken from a clean copy) into a
clean environment with the pristine configuration, lockfile and dependencies,
then runs the project's entire jest suite (every project: core, RxJS minimum
version, GraphQL 16, React 17, 18 and 19, and the codegen plugins). A held-out
set of tests exercises the behaviour above in many more combinations, and the
whole existing suite is the regression wall: a change that fixes the new
cases but breaks an existing test does not count. Do not add dependencies.
