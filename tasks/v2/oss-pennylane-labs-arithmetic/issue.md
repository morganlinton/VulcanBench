Add arithmetic resource operators to `pennylane.labs.estimator_beta`.

The labs resource estimator has QROM-style data-loading operators but no
arithmetic. Add six resource operators to
`pennylane/labs/estimator_beta/templates/arithmetic.py` implementing
Fourier-space (phase-gradient) arithmetic costing:

- `LabsPhaseAdder(num_x_wires, mod=None, wires=None)`: in-place addition of a
  classical constant in the Fourier basis.
- `LabsAdder(num_x_wires, mod=None, wires=None)`: like `LabsPhaseAdder` but
  including the QFT conjugation.
- `LabsOutAdder(num_x_wires, num_y_wires, num_output_wires, mod=None,
  wires=None)`: out-of-place addition of two quantum registers.
- `ClassicalOutMultiplier(num_x_wires, num_output_wires, mod=None,
  wires=None)`: out-of-place multiplication by a classical constant.
- `LabsMultiplier(num_x_wires, mod=None, wires=None)`: in-place modular
  multiplication by a classical constant.
- `LabsModExp(num_x_wires, num_output_wires, mod=None, wires=None)`:
  out-of-place modular exponentiation (`|x>|b> -> |x>|b * base^x mod mod>`),
  the arithmetic core of Shor-style algorithms.

Semantics:

- When `mod` is omitted it defaults to the maximum for the register
  (`2**num_x_wires` or `2**num_output_wires` as appropriate). A `mod` outside
  `(1, 2**n)` raises `ValueError`.
- The power-of-two variants use the plain Fourier-space constructions; general
  `mod` values require the modular-reduction circuitry (extra work wires and
  controlled corrections), which must be reflected in the reported resources.
- Resource reporting goes through the standard `qre.estimate` pipeline:
  algorithmic vs allocated wires and gate counts by type, consistent with how
  the estimator accounts T gates for phase rotations, Toffolis, CNOT ladders,
  and QFT Hadamards.

Export all six from `pennylane.labs.estimator_beta` and from
`pennylane.labs.estimator_beta.templates`. Existing operators (including
`LabsQROM` and `SelectCopyQROM`) must be unchanged.
