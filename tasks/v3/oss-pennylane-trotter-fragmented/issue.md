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
- Two-body diagonals decompose into `IsingZZ` pairs (one per spin-orbital wire
  pair, with same-orbital pairs driven by the diagonal core entries and
  cross-orbital pairs by the off-diagonal entries, all four spin combinations);
  one-body diagonals decompose into per-wire `RZ` rotations. The linear
  (one-body) corrections arising from the two-body diagonals are folded into
  the trailing global phase, not emitted as extra rotations.
- The accumulated scalar terms (`nuc_constant` plus the diagonal corrections)
  appear once at the end as `GlobalPhase(phi)` with `phi` reduced modulo
  `4*pi`.
- With `control_wires` given (a single wire), the evolution uses the
  double-phase trick: diagonal rotations are sandwiched by `CNOT`s from the
  control, and the global phase becomes `RZ(-phi)` on the control wire.

Acceptance example pinning the conventions. For a single-fragment CDF input on
4 wires with one second-order Trotter step over `evolution_time=1.0`:

```python
def rot(t):
    c, s = np.cos(t), np.sin(t)
    return np.array([[c, -s], [s, c]])

hamiltonian = {
    "nuc_constant": 0.3,
    "core_tensors": np.stack([np.diag([0.3, -0.1]),
                              np.array([[0.25, 0.15], [0.15, -0.2]])]),
    "leaf_tensors": np.stack([rot(0.4), rot(0.9)]),
}
trotter_fragmented(1.0, 1, hamiltonian, wires=[0, 1, 2, 3])
```

the queued circuit must be exactly (angles in radians; BasisRotation shown by
its unitary's first row):

```
BasisRotation(U~[[0.6216, -0.7833], ...], wires=[0, 2])   # rot(0.9): two-body leaf
BasisRotation(U~[[0.6216, -0.7833], ...], wires=[1, 3])
IsingZZ(-0.03125, wires=[0, 1])
IsingZZ(-0.01875, wires=[0, 2])
IsingZZ(-0.01875, wires=[0, 3])
IsingZZ(-0.01875, wires=[1, 2])
IsingZZ(-0.01875, wires=[1, 3])
IsingZZ(0.025,    wires=[2, 3])
BasisRotation(U~[[0.8776, 0.4794], ...], wires=[0, 2])    # rot(0.9)^T @ rot(0.4): merged into one-body
BasisRotation(U~[[0.8776, 0.4794], ...], wires=[1, 3])
RZ(0.15,  wires=[0])
RZ(0.15,  wires=[1])
RZ(-0.05, wires=[2])
RZ(-0.05, wires=[3])
BasisRotation(U~[[0.8776, -0.4794], ...], wires=[0, 2])   # rot(0.4)^T @ rot(0.9): merged back
BasisRotation(U~[[0.8776, -0.4794], ...], wires=[1, 3])
IsingZZ(-0.03125, wires=[0, 1])
IsingZZ(-0.01875, wires=[0, 2])
IsingZZ(-0.01875, wires=[0, 3])
IsingZZ(-0.01875, wires=[1, 2])
IsingZZ(-0.01875, wires=[1, 3])
IsingZZ(0.025,    wires=[2, 3])
BasisRotation(U~[[0.6216, 0.7833], ...], wires=[0, 2])    # rot(0.9)^T: final restore
BasisRotation(U~[[0.6216, 0.7833], ...], wires=[1, 3])
GlobalPhase(0.3375, wires=[])
```

Export `trotter_fragmented` from `pennylane.labs.templates`. Existing labs
templates and core PennyLane behavior must be unchanged.
