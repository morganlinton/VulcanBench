"""Hidden test for oss-sympy-ground-eigenvects.

``DomainMatrix`` must gain ``ground_eigenvals`` and ``ground_eigenvects``:
the eigenvalues that live in the matrix's ground domain (with algebraic
multiplicities) and, for ``ground_eigenvects``, a basis of eigenvectors for
each. The eigenvalue set is determined, but the *representation* of the result
is implementation-defined: the eigenvalues are returned in the order the
characteristic polynomial factors, not sorted; and each eigenvector basis is a
specific null-space basis (scaled a particular way). Expected values captured
from the upstream feature (sympy/sympy PR #29984).

Run with PYTHONPATH=. so the workspace sympy is under test.
"""
from __future__ import annotations

from sympy import ZZ, QQ, QQ_I
from sympy.polys.matrices.domainmatrix import DomainMatrix, DM
from sympy.polys.matrices.exceptions import DMNonSquareMatrixError
from sympy.testing.pytest import raises


def test_ground_eigenvals_basic():
    A = DomainMatrix([], (0, 0), ZZ)
    assert A.ground_eigenvals() == {}

    A = DomainMatrix.zeros((3, 3), QQ)
    assert A.ground_eigenvals() == {QQ(0): 3}

    A = DM([[0, 1], [1, 0]], ZZ)
    assert A.ground_eigenvals() == {ZZ(1): 1, ZZ(-1): 1}

    # A rotation has no eigenvalue in QQ, but does in the Gaussian rationals.
    A = DM([[0, 1], [-1, 0]], QQ)
    assert A.ground_eigenvals() == {}
    A = DM([[0, 1], [-1, 0]], QQ_I)
    assert A.ground_eigenvals() == {QQ_I(0, 1): 1, QQ_I(0, -1): 1}

    A = DM([[4, 1, 0, 0],
            [-3, QQ(1, 2), 0, 0],
            [0, 0, 2, QQ(3, 2)],
            [0, 0, 0, 2]], QQ)
    assert A.ground_eigenvals() == {QQ(2): 3, QQ(5, 2): 1}


def test_ground_eigenvects_symmetric_swap_exact():
    A = DM([[0, 1], [1, 0]], ZZ)
    assert A.ground_eigenvects() == [
        (1, 1, DM([[1], [1]], ZZ)),
        (-1, 1, DM([[-1], [1]], ZZ)),
    ]


def test_ground_eigenvects_zeros_identity_basis():
    A = DomainMatrix.zeros((3, 3), QQ)
    assert A.ground_eigenvects() == [(QQ(0), 3, DomainMatrix.eye(3, QQ))]


def test_ground_eigenvects_block_matrix_exact():
    # The discriminating case: eigenvalues come back in factorization order
    # (5/2 before 2, NOT sorted), and each eigenvector basis is scaled the
    # upstream way. A plausible implementation that sorts eigenvalues or scales
    # the null-space basis differently produces a valid-but-different result.
    A = DM([[4, 1, 0, 0],
            [-3, QQ(1, 2), 0, 0],
            [0, 0, 2, QQ(3, 2)],
            [0, 0, 0, 2]], QQ)
    assert A.ground_eigenvects() == [
        (QQ(5, 2), 1, DM([[-QQ(2, 3)], [1], [0], [0]], QQ)),
        (QQ(2), 3, DM([[-QQ(1, 2), 0], [1, 0], [0, 1], [0, 0]], QQ)),
    ]


def test_ground_eigenvects_empty_and_no_ground_eigenvalue():
    assert DomainMatrix([], (0, 0), ZZ).ground_eigenvects() == []
    # Rotation over QQ: no ground eigenvalue, hence no eigenvectors.
    assert DM([[0, 1], [-1, 0]], QQ).ground_eigenvects() == []


def test_non_square_raises():
    A = DM([[1, 2]], QQ)
    raises(DMNonSquareMatrixError, lambda: A.ground_eigenvals())
    raises(DMNonSquareMatrixError, lambda: A.ground_eigenvects())


def test_core_domainmatrix_unaffected():
    # Regression guard: existing DomainMatrix behavior is untouched.
    A = DM([[1, 2], [3, 4]], ZZ)
    assert A.transpose().to_list() == [[ZZ(1), ZZ(3)], [ZZ(2), ZZ(4)]]
    assert (A + A).to_list() == [[ZZ(2), ZZ(4)], [ZZ(6), ZZ(8)]]
