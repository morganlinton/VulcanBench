Add `ground_eigenvals` and `ground_eigenvects` to `DomainMatrix`.

`DomainMatrix` (in `sympy.polys.matrices.domainmatrix`) can compute a
characteristic polynomial but has no way to report eigenvalues and eigenvectors
that live in the matrix's ground domain. Add two methods:

- `ground_eigenvals(self)` returns the eigenvalues that lie in the ground domain,
  as a mapping from eigenvalue to algebraic multiplicity. Only eigenvalues that
  actually belong to the ground domain are reported (for example, a rational
  rotation matrix has no eigenvalues in `QQ`, but does in the Gaussian rationals
  `QQ_I`).
- `ground_eigenvects(self)` returns a list of `(eigenvalue, multiplicity,
  eigenvectors)` triples, where `eigenvectors` is a `DomainMatrix` whose columns
  are a basis for the corresponding eigenspace.

Both methods require a square matrix and should raise `DMNonSquareMatrixError`
otherwise. The empty `0x0` matrix has no eigenvalues. Add the methods with
docstrings and doctests in the style of the surrounding `DomainMatrix` code.
