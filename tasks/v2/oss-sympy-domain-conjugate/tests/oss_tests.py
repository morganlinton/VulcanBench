"""Hidden test for oss-sympy-domain-conjugate.

A ``ConjugateDomain`` abstraction must be added and complex conjugation
implemented consistently across the polynomial domain hierarchy: Gaussian
integers/rationals, the complex field, real/rational/integer domains (where
conjugation is the identity), and algebraic number fields. Expected behavior
captured from the upstream feature (sympy/sympy PR #29877).

Run with PYTHONPATH=. so the workspace sympy is under test.
"""
from __future__ import annotations

from sympy import CC, QQ, QQ_I, RR, ZZ, ZZ_I, I


def test_gaussian_integer_conjugate():
    assert ZZ_I.conjugate(ZZ_I(3, 4)) == ZZ_I(3, -4)


def test_gaussian_rational_conjugate():
    assert QQ_I.conjugate(QQ_I(3, 4)) == QQ_I(3, -4)


def test_real_domains_conjugate_is_identity():
    # On domains with no imaginary part, conjugation returns the value unchanged
    # and stays inside the domain.
    for domain, val in ((ZZ, ZZ(5)), (QQ, QQ(3, 2)), (RR, RR(1.5))):
        c = domain.conjugate(val)
        assert c == val
        assert domain.of_type(c)


def test_complex_field_conjugate():
    # Conjugating a complex-field element flips the imaginary part.
    assert CC.conjugate(CC(1, 2)) == CC(1, -2)
    assert CC.of_type(CC.conjugate(CC(1, 2)))


def test_algebraic_field_conjugate():
    K = QQ.algebraic_field(I)
    a = K.from_sympy(1 + I)
    assert K.to_sympy(K.conjugate(a)) == (1 - I)


def test_base_domain_arithmetic_unaffected():
    # The ordinary domain API must keep working.
    assert ZZ_I(3, 4) * ZZ_I(1, 1) == ZZ_I(-1, 7)
    assert QQ.to_sympy(QQ(3, 2) + QQ(1, 2)) == 2
