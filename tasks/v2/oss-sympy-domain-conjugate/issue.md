The polynomial domains have no notion of complex conjugation, so there is no uniform way to conjugate an element regardless of which domain it lives in.

Add a `ConjugateDomain` abstraction and implement conjugation across the domain hierarchy so that `domain.conjugate(a)` works consistently:

- Introduce a `ConjugateDomain` base (in `sympy/polys/domains/conjugatedomain.py`) declaring the `conjugate` interface, and expose it so domains can mix it in.
- Domains with an imaginary part conjugate properly: the Gaussian integer and Gaussian rational domains flip the sign of the imaginary component (conjugate of `3 + 4*I` is `3 - 4*I`), and the complex field conjugates its floating-point elements.
- Domains that are subsets of the reals (integers, rationals, reals) define conjugation as the identity, returning the same element, still typed as a member of the domain.
- Algebraic number fields conjugate elements using the conjugate of the field's generator (the conjugate of a primitive element shares the same minimal polynomial).

Every element returned by `conjugate` must belong to the same domain, and all existing domain arithmetic must be unaffected.
