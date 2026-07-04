"""Hidden test for oss-pennylane-trotter-fragmented.

``pennylane.labs.templates`` must gain ``trotter_fragmented``: a second-order
Trotter time-evolution template for fragmented Hamiltonians, supporting both
electronic CDF (Compressed Double Factorization) and vibrational CGF
(Christiansen Greedy Fragmentation) inputs, auto-detected from tensor shapes.

The emitted circuit is graded EXACTLY against the upstream implementation
(PennyLaneAI/pennylane PR #9459): adjacent fragment basis rotations must be
merged (one rotation per boundary, not an undo/redo pair), rotations that merge
to the identity must be skipped, diagonal fragments must decompose into the
upstream RZ/IsingZZ convention, and the accumulated energy shift must appear as
a trailing ``GlobalPhase`` reduced modulo 4*pi (or, with a control wire, as
``RZ(-phi)`` on the control via the double-phase trick). Many mathematically
equivalent circuits exist; only the upstream one passes.

Run with PYTHONPATH=. so the workspace pennylane is under test.
"""
from __future__ import annotations

import numpy as np
import pennylane as qp
import pytest


def _rot(theta):
    c, s = np.cos(theta), np.sin(theta)
    return np.array([[c, -s], [s, c]])


def _make_cdf():
    # L=2 two-body fragments, N=2 orbitals -> 4 system wires (alpha/beta interleaved).
    leaf = np.stack([_rot(0.3), _rot(0.7), _rot(1.1)])
    core = np.stack([
        np.diag([0.5, -0.25]),
        np.array([[0.2, 0.1], [0.1, -0.3]]),
        np.array([[-0.4, 0.05], [0.05, 0.6]]),
    ])
    return {"nuc_constant": 0.5, "core_tensors": core, "leaf_tensors": leaf}


def _make_cgf():
    # L=1 two-body fragment, M=2 modes, N=2 modals -> 4 system wires (mode-major).
    leaf = np.stack([
        np.stack([_rot(0.2), _rot(0.4)]),
        np.stack([_rot(0.6), _rot(0.8)]),
    ])
    core = np.zeros((2, 2, 2, 2, 2))
    core[0, 0, 0] = np.diag([0.3, -0.2])
    core[0, 1, 1] = np.diag([0.1, 0.25])
    core[1, 0, 1] = np.array([[0.15, -0.05], [0.2, 0.1]])
    core[1, 1, 0] = np.array([[0.15, 0.2], [-0.05, 0.1]])
    return {"nuc_constant": -0.25, "core_tensors": core, "leaf_tensors": leaf}


def _capture(fn):
    with qp.queuing.AnnotatedQueue() as q:
        fn()
    return list(q.queue)


def _sig(op):
    wires = tuple(int(w) for w in op.wires)
    if op.name in ("RZ", "IsingZZ", "PhaseShift", "GlobalPhase"):
        return (op.name, wires, round(float(op.data[0]), 10))
    return (op.name, wires)


CDF_REF = [
    ("BasisRotation", (0, 2)),
    ("BasisRotation", (1, 3)),
    ("IsingZZ", (0, 1), -0.025),
    ("IsingZZ", (0, 2), -0.0125),
    ("IsingZZ", (0, 3), -0.0125),
    ("IsingZZ", (1, 2), -0.0125),
    ("IsingZZ", (1, 3), -0.0125),
    ("IsingZZ", (2, 3), 0.0375),
    ("BasisRotation", (0, 2)),
    ("BasisRotation", (1, 3)),
    ("IsingZZ", (0, 1), 0.05),
    ("IsingZZ", (0, 2), -0.00625),
    ("IsingZZ", (0, 3), -0.00625),
    ("IsingZZ", (1, 2), -0.00625),
    ("IsingZZ", (1, 3), -0.00625),
    ("IsingZZ", (2, 3), -0.075),
    ("BasisRotation", (0, 2)),
    ("BasisRotation", (1, 3)),
    ("RZ", (0,), 0.25),
    ("RZ", (1,), 0.25),
    ("RZ", (2,), -0.125),
    ("RZ", (3,), -0.125),
    ("BasisRotation", (0, 2)),
    ("BasisRotation", (1, 3)),
    ("IsingZZ", (0, 1), 0.05),
    ("IsingZZ", (0, 2), -0.00625),
    ("IsingZZ", (0, 3), -0.00625),
    ("IsingZZ", (1, 2), -0.00625),
    ("IsingZZ", (1, 3), -0.00625),
    ("IsingZZ", (2, 3), -0.075),
    ("BasisRotation", (0, 2)),
    ("BasisRotation", (1, 3)),
    ("IsingZZ", (0, 1), -0.025),
    ("IsingZZ", (0, 2), -0.0125),
    ("IsingZZ", (0, 3), -0.0125),
    ("IsingZZ", (1, 2), -0.0125),
    ("IsingZZ", (1, 3), -0.0125),
    ("IsingZZ", (2, 3), 0.0375),
    ("BasisRotation", (0, 2)),
    ("BasisRotation", (1, 3)),
    ("GlobalPhase", (), 0.575),
]

# (Re[U[0,0]], Re[U[0,1]]) of each BasisRotation in CDF_REF order. These pin the
# MERGED rotation products (e.g. rot(0.7)^T @ rot(1.1)), not the raw leaves.
CDF_BR_FPS = [
    (0.76484221, -0.64421767),
    (0.76484221, -0.64421767),
    (0.92106098, -0.38941836),
    (0.92106098, -0.38941836),
    (0.69670671, 0.71735609),
    (0.69670671, 0.71735609),
    (0.69670671, -0.71735609),
    (0.69670671, -0.71735609),
    (0.92106098, 0.38941836),
    (0.92106098, 0.38941836),
    (0.76484219, 0.64421769),
    (0.76484219, 0.64421769),
]

CDF_CTRL_COUNTS = {"BasisRotation": 12, "CNOT": 32, "IsingZZ": 24, "RZ": 5}
CDF_CTRL_N = 73
CDF_CTRL_LAST = ("RZ", (4,), -0.575)

CGF_REF = [
    ("BasisRotation", (0, 1)),
    ("BasisRotation", (2, 3)),
    ("IsingZZ", (2, 0), 0.009375),
    ("IsingZZ", (2, 1), 0.0125),
    ("IsingZZ", (3, 0), -0.003125),
    ("IsingZZ", (3, 1), 0.00625),
    ("BasisRotation", (0, 1)),
    ("BasisRotation", (2, 3)),
    ("RZ", (0,), -0.075),
    ("RZ", (1,), 0.05),
    ("RZ", (2,), -0.025),
    ("RZ", (3,), -0.0625),
    ("BasisRotation", (0, 1)),
    ("BasisRotation", (2, 3)),
    ("IsingZZ", (2, 0), 0.009375),
    ("IsingZZ", (2, 1), 0.0125),
    ("IsingZZ", (3, 0), -0.003125),
    ("IsingZZ", (3, 1), 0.00625),
    ("BasisRotation", (0, 1)),
    ("IsingZZ", (2, 0), 0.009375),
    ("IsingZZ", (2, 1), 0.0125),
    ("IsingZZ", (3, 0), -0.003125),
    ("IsingZZ", (3, 1), 0.00625),
    ("BasisRotation", (0, 1)),
    ("BasisRotation", (2, 3)),
    ("RZ", (0,), -0.075),
    ("RZ", (1,), 0.05),
    ("RZ", (2,), -0.025),
    ("RZ", (3,), -0.0625),
    ("BasisRotation", (0, 1)),
    ("BasisRotation", (2, 3)),
    ("IsingZZ", (2, 0), 0.009375),
    ("IsingZZ", (2, 1), 0.0125),
    ("IsingZZ", (3, 0), -0.003125),
    ("IsingZZ", (3, 1), 0.00625),
    ("BasisRotation", (0, 1)),
    ("BasisRotation", (2, 3)),
    ("GlobalPhase", (), 12.5538706144),
]


def _trotter_fragmented():
    from pennylane.labs.templates import trotter_fragmented

    return trotter_fragmented


def test_cdf_exact_gate_stream():
    tf = _trotter_fragmented()
    ops = _capture(lambda: tf(1.0, 1, _make_cdf(), wires=list(range(4))))
    assert [_sig(op) for op in ops] == CDF_REF


def test_cdf_merged_basis_rotation_matrices():
    tf = _trotter_fragmented()
    ops = _capture(lambda: tf(1.0, 1, _make_cdf(), wires=list(range(4))))
    fps = [
        (
            round(float(np.real(np.asarray(op.parameters[0])[0, 0])), 8),
            round(float(np.real(np.asarray(op.parameters[0])[0, 1])), 8),
        )
        for op in ops
        if op.name == "BasisRotation"
    ]
    assert fps == CDF_BR_FPS


def test_cdf_controlled_double_phase():
    tf = _trotter_fragmented()
    ops = _capture(
        lambda: tf(1.0, 1, _make_cdf(), wires=list(range(4)), control_wires=[4])
    )
    counts: dict[str, int] = {}
    for op in ops:
        counts[op.name] = counts.get(op.name, 0) + 1
    assert counts == CDF_CTRL_COUNTS
    assert len(ops) == CDF_CTRL_N
    # The controlled global phase becomes RZ(-phi) on the control wire.
    assert _sig(ops[-1]) == CDF_CTRL_LAST


def test_cgf_exact_gate_stream():
    tf = _trotter_fragmented()
    ops = _capture(lambda: tf(0.5, 2, _make_cgf(), wires=list(range(4))))
    assert [_sig(op) for op in ops] == CGF_REF


def test_shape_autodetection_rejects_bad_input():
    tf = _trotter_fragmented()
    bad = {
        "nuc_constant": 0.0,
        "core_tensors": np.zeros((2, 2)),
        "leaf_tensors": np.zeros((2, 2)),
    }
    with pytest.raises(ValueError):
        _capture(lambda: tf(1.0, 1, bad, wires=[0, 1]))


def test_core_pennylane_unaffected():
    # Regression guard: adding the labs template must not disturb core behavior.
    with qp.queuing.AnnotatedQueue() as q:
        qp.RZ(0.5, wires=0)
        qp.IsingZZ(0.25, wires=[0, 1])
    assert [op.name for op in q.queue] == ["RZ", "IsingZZ"]
    from pennylane.labs.templates import LeftClassicalComparator  # noqa: F401
