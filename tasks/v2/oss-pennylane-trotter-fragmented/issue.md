Add a `trotter_fragmented` template to `pennylane.labs.templates`.

Add a second-order Trotter time-evolution template for fragmented Hamiltonians
to the labs module, matching the reference construction used in the upstream
resource-estimation work (arXiv:2506.15784 Sec. III A for electronic CDF,
arXiv:2508.11865 Sec. III C for vibrational CGF Hamiltonians).

API:

```python
trotter_fragmented(evolution_time, num_trotter_steps, hamiltonian, wires, control_wires=None)
```

`hamiltonian` is a dict with keys `nuc_constant`, `core_tensors`, `leaf_tensors`.
The Hamiltonian type is auto-detected from tensor shapes:

- CDF: `core_tensors: (L+1, N, N)`, `leaf_tensors: (L+1, N, N)` for N orbitals
  and L two-body fragments. The system uses `2N` wires, alpha/beta interleaved:
  a fragment's basis rotation acts as one `BasisRotation` on the even wires and
  one on the odd wires.
- CGF: `core_tensors: (L+1, M, M, N, N)`, `leaf_tensors: (L+1, M, N, N)` for M
  modes with N modals each. The system uses `M*N` wires, mode-major; per-mode
  `BasisRotation`s receive the transposed leaf (leaves store the
  bare-to-diagonal direction).
- Any other shape combination raises `ValueError`.

Index 0 of the tensors is the one-body fragment; indices 1..L are the two-body
fragments. Each second-order Trotter step sweeps the two-body fragments forward,
applies the one-body fragment, then sweeps the two-body fragments backward, with
each diagonal evolved for half the step time.

Circuit conventions (the emitted gate stream must match the reference
implementation exactly; many mathematically equivalent circuits exist):

- Consecutive basis rotations at fragment boundaries are merged into a single
  rotation per boundary (`U_prev^dagger @ U_curr`), including across the
  one-body turn-around, across adjacent Trotter steps, and for the final
  restoring rotation after the last step. A rotation that merges to the
  identity is not emitted at all.
- Two-body diagonals decompose into `IsingZZ` pairs plus the one-body
  correction absorbed the way the reference does; one-body diagonals decompose
  into per-wire `RZ` rotations.
- The accumulated scalar terms (`nuc_constant` plus the diagonal corrections)
  appear once at the end as `GlobalPhase(phi)` with `phi` reduced modulo
  `4*pi`.
- With `control_wires` given (a single wire), the evolution uses the
  double-phase trick: diagonal rotations are sandwiched by `CNOT`s from the
  control, and the global phase becomes `RZ(-phi)` on the control wire.

Export `trotter_fragmented` from `pennylane.labs.templates`. Existing labs
templates and core PennyLane behavior must be unchanged.
