# Duration formatting and fixed-offset zone validity give wrong answers

The workspace at `/app` is Luxon, a JavaScript date and time library. Its dev
dependencies are already installed; run the suite the usual way:

```
npx jest
```

The suite passes as given (two locale-era tests fail on this machine's ICU
build regardless of your changes; ignore those). But users report three
behaviours that are wrong, none of which the current tests cover:

- `Duration#toISO` can produce output that is not valid ISO 8601 and that
  `Duration.fromISO` then refuses to parse. It happens when a unit's value is a
  very small fraction, for example after converting one millisecond into hours:
  the number is written in a form ISO 8601 does not allow, so the duration does
  not round-trip through its own string representation. Values that already
  format correctly today, including the rounding of seconds and milliseconds
  to three decimals, must keep formatting exactly as they do now.
- `Duration#toFormat` with `signMode: "negativeLargestOnly"` loses the sign on
  a negative duration when the largest unit in the format rounds to zero.
  Formatting minus thirty minutes as `h:mm` yields `0:30`, indistinguishable
  from plus thirty minutes; it should yield `-0:30`. The same happens for any
  format whose leading unit is zero. A positive duration whose largest unit is
  zero must stay unsigned, and the other sign modes must behave as they do now.
- A `FixedOffsetZone` constructed with something that is not a numeric offset
  (a string such as `"CDT"` or `"5"`, `NaN`, `undefined`, `null`, an object)
  reports itself as valid. It should be invalid, and a `DateTime` created in
  such a zone should be an invalid DateTime whose `invalidReason` is
  `"unsupported zone"`, the same way an unrecognised zone is handled today.
  Zones constructed with an integer offset, including those obtained through
  the usual factory methods, stay valid.

Fix all three so the behaviour is correct in general, not just for the examples
above. The public API does not change.

Grading copies only your `src/` tree into a clean environment that has the
pristine test suite, jest and babel configuration and dependencies, runs a
held-out set of tests for these three behaviours, and then runs the library's
entire existing test suite as a regression wall. A change that makes the
held-out tests pass but breaks any existing test does not count. Do not add
runtime dependencies; nothing outside `src/` is graded.
