# The symbol solver resolves names that Java does not let them reach

The workspace at `/app` is JavaParser, a Java parser with a symbol solver
that resolves names, method calls and types in parsed Java source. It builds
and tests offline:

```
cd /app
mvn -pl javaparser-symbol-solver-testing,javaparser-core-testing -am test
```

The suite passes as given. Users report four ways the symbol solver's answers
differ from what `javac` does, and the current tests do not notice:

- A qualified method call on a type read from source, such as `Host.ping()`,
  `new Host().ping()` or the method reference `Host::ping`, resolves to a
  method `Host` neither declares nor inherits, found through a static import
  in `Host`'s own file (`import static lib.Tools.*`). Imports are not
  transitive and `javac` rejects all three. The same call on a type loaded
  from a jar is already reported as unsolved.

- A qualified nested type name, such as `Host.Nested` in `new Host.Nested()`
  or in a type use, resolves to a type that `Host`'s file imports (statically,
  or through an ordinary `import other.Nested`) instead of only to the member
  types `Host` declares or inherits.

- When a type's hierarchy contains a superclass or interface that cannot be
  resolved anywhere above its direct ancestors, every method call on the
  type fails with an unsolved-symbol error, even for methods that have
  nothing to do with the missing type. A missing direct ancestor is already
  tolerated. A question whose answer really depends on the missing type must
  still raise the unsolved-symbol error, never return a wrong answer.

- Calls such as `Arrays.copyOf(ints, n)` with an `int[]` argument fail with a
  method ambiguity error: the `int[]` argument is accepted for `long[]`,
  `float[]` and `double[]` parameters, which Java forbids.

Fix the underlying causes so that each of these resolves as `javac` would,
and so that everything that resolves correctly today keeps resolving the
same way. Do not special-case the examples above; the grading cases cover the
same rules in more forms.

Grading copies only your `src/main/java` trees of `javaparser-core` and
`javaparser-symbol-solver-core` (the `com.github.javaparser` packages) into a
clean environment that has the pristine tests, build configuration and
dependencies, then runs both testing modules: held-out tests for these
rules, and every existing test as a regression wall. Do not add
dependencies; nothing outside those two trees is graded.
