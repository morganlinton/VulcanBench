# Fraction arithmetic overflows, returns unreduced results, and leaks the wrong exception

The workspace at `/app` is Apache Commons Lang, a pure-Java utility library
built with Maven. The build is configured to run offline against a local
repository that already holds everything it needs, with the static-analysis
plugins switched off. Run the tests the usual way, for example:

```
mvn test -Dtest=FractionTest
mvn test
```

The suite passes as given (the full run takes a few minutes). But users of
`org.apache.commons.lang3.math.Fraction` report wrong behaviour that the
current tests do not cover:

- Arithmetic on fractions that are not in lowest terms fails or gives
  unreduced answers. `Fraction.getFraction(int, int)` deliberately keeps the
  numerator and denominator as given, yet `multiplyBy`, `divideBy`, `pow`,
  `add` and `subtract` can throw `ArithmeticException` reporting an overflow
  even though the exact result fits comfortably in an `int`, as soon as an
  operand carries a common factor; the same values passed in reduced form
  work. `add` and `subtract` are also documented to return the result in
  reduced form, but they do not when an operand had a common factor.
- `add` and `subtract` with a zero operand hand back the other operand as it
  was given (or its negation) instead of the reduced form the method promises,
  and negating a value whose numerator is `Integer.MIN_VALUE` along that path
  must raise `ArithmeticException` rather than silently wrap.
- `Fraction.getReducedFraction(int, int)` mishandles `Integer.MIN_VALUE`. It
  only copes with the one case the comment mentions; other combinations of
  `Integer.MIN_VALUE` with an even or negative partner either throw an
  overflow when the reduced value is representable (for example minus two
  to the thirty-first over minus two is a plain positive integer) or produce
  a wrong sign. When the reduced result genuinely cannot be represented
  (such as `Integer.MIN_VALUE` over minus one) it must still throw
  `ArithmeticException`.
- `Fraction.getFraction(String)` is documented to throw
  `NumberFormatException` when the string cannot be parsed, but for inputs
  whose value is unrepresentable (a decimal too large for an `int`, a zero
  denominator such as `"1/0"` or `"1 2/0"`, or a mixed number whose whole
  part overflows) it throws an undeclared `ArithmeticException` instead. It
  should throw `NumberFormatException` with the arithmetic failure attached
  as the cause, for all three accepted formats.

Fix these so that `Fraction` is correct for arbitrary operands, reduced or
not, including the extreme `int` values, and so that an operation throws
`ArithmeticException` only when the exact result truly cannot be represented.
The public API and the documented contracts do not change. Do not special-case
the examples above; find and fix the underlying causes.

Grading copies only your `src/main` tree into a clean environment that has
the pristine test sources, build configuration and dependencies, compiles it,
runs a held-out set of `Fraction` tests, and then runs the library's entire
existing test suite as a regression wall. A change that makes the held-out
tests pass but breaks any existing test, or that fails to compile against the
existing tests, does not count. Do not add dependencies; nothing outside
`src/main` is graded.
