Add a `SelectCopyQROM` resource operator to `pennylane.labs.estimator_beta`.

The labs resource estimator has a `LabsQROM` operator for plain QROM costing,
but no operator for the Select-Copy QROM variant of Low et al. (2024),
`arXiv:1812.00954`, which trades auxiliary "dirty" qubits for a lower Toffoli
count by loading data in parallel batches (Figure 1.C and Theorem 1 of the
paper).

Add `SelectCopyQROM(num_bitstrings, size_bitstring, available_dirty_aux=None,
batch_size=None, bits_per_iter=None, wires=None)` to
`pennylane/labs/estimator_beta/templates/subroutines.py`:

- When `available_dirty_aux` is given, the operator must choose the optimal
  batch size and bits-per-iteration for that dirty-qubit budget by optimizing
  the Theorem 1 cost model, and report the resources of the resulting
  decomposition.
- When `batch_size` and `bits_per_iter` are given explicitly (both required
  together, mutually exclusive with `available_dirty_aux`), those parameters
  are used directly and determine the implied dirty-qubit usage.
- When none of the three are given, the operator falls back to the
  no-extra-dirty-qubits configuration.
- Resource reporting goes through the standard `qre.estimate` pipeline:
  algorithmic vs allocated wires, and gate counts broken down by type,
  consistent with how the estimator accounts Toffolis (including the
  left-elbow form used for temporary logical ANDs), CNOT fan-outs, X and
  Hadamard gates.

Export it from `pennylane.labs.estimator_beta` and from
`pennylane.labs.estimator_beta.templates`. Existing operators, including
`LabsQROM`, must be unchanged.

Example from the resource-estimation workflow:

```pycon
>>> import pennylane.labs.estimator_beta as qre
>>> qrom_op = qre.SelectCopyQROM(
...     num_bitstrings = 10**8,
...     size_bitstring = 8,
...     available_dirty_aux = 300,
... )
>>> print(qre.estimate(qrom_op))
--- Resources: ---
 Total wires: 308
   algorithmic wires: 35
   allocated wires: 273
     zero state: 273
     any state: 0
 Total gates : 4.781E+8
   'Toffoli': 3.520E+6,
   'CNOT': 4.570E+8,
   'X': 7.036E+6,
   'Hadamard': 1.055E+7
```

Acceptance example with exact integer counts, pinning the cost model. The
parameter optimization brute-forces the Theorem 1 Toffoli cost

```
cost(N, b, mu, lam) = (ceil(b/mu) + 1) * (ceil(N/lam) + lam - 5)
                      + (lam - 1) * (mu * (floor(b/mu) + 1) + b % mu)
```

over bits-per-iteration `mu in 1..b` and batch sizes `lam` restricted to powers
of two (`lam = 2, 4, 8, ...` up to `2**floor(log2(N-1))`), subject to
`mu * (lam - 1) <= available_dirty_aux`, taking the first minimum in that
iteration order. For `num_bitstrings=2048, size_bitstring=16,
available_dirty_aux=48` the estimate must come out exactly:

```
total wires: 83  (algorithmic 27, allocated zero-state 56)
total gates: 41006
  X:                2048
  CNOT:             34782
  Toffoli (left-elbow logical ANDs): 1020
  Toffoli (plain):  96
  Hadamard:         3060
```
