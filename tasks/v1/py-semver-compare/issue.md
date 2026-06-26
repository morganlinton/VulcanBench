# Implement Semantic Versioning precedence comparison

`semver.compare(a, b)` should return `-1`, `0`, or `1` depending on whether
version `a` has lower, equal, or higher precedence than `b`, following the
Semantic Versioning 2.0.0 spec.

A version is `MAJOR.MINOR.PATCH`, optionally followed by `-prerelease` and/or
`+build` metadata. Rules:

- Compare `MAJOR`, then `MINOR`, then `PATCH` numerically.
- **Build metadata is ignored** for precedence (`1.0.0+a` equals `1.0.0+b`).
- A version **with** a prerelease has **lower** precedence than the same version
  without one: `1.0.0-alpha` < `1.0.0`.
- Compare prerelease versions identifier by identifier (split on `.`):
  - identifiers made of only digits are compared **numerically** (`2` < `11`);
  - identifiers with letters are compared lexically in ASCII order;
  - a **numeric** identifier is always lower precedence than an **alphanumeric**
    one (`alpha.1` < `alpha.beta`);
  - if all shared identifiers are equal, the version with **more** identifiers
    has higher precedence (`alpha` < `alpha.1`).
- `compare` must be antisymmetric: `compare(a, b) == -compare(b, a)`.
- A string that is not a valid semver (wrong field count, leading zeros in a
  numeric field such as `01.0.0` or `1.0.0-01`, an empty prerelease, stray
  characters) raises `ValueError`.

Implement `compare` in `semver/compare.py`.
