"""Hidden test for oss-pennylane-labs-arithmetic.

``pennylane.labs.estimator_beta`` must gain six arithmetic resource operators:
``LabsPhaseAdder``, ``LabsAdder``, ``LabsOutAdder``, ``ClassicalOutMultiplier``,
``LabsMultiplier`` and ``LabsModExp`` (PennyLaneAI/pennylane PR #9390). Each
reports, through ``qre.estimate``, the resources of the corresponding
Fourier-space arithmetic construction, in both the plain power-of-two and the
general modular variant where applicable.

Grading is on EXACT wire and gate counts captured from the upstream
implementation. The counts encode the full cost model: which building blocks
each operator decomposes into, how modular reduction is handled (extra work
wires, controlled corrections), the T-count of the phase rotations, and the
auxiliary wire bookkeeping. A from-scratch implementation faithful to the
textbook constructions but differing in any internal choice produces different
counts and fails.

Run with PYTHONPATH=. so the workspace pennylane is under test.
"""
from __future__ import annotations

import pytest


def _qre():
    import pennylane.labs.estimator_beta as qre

    return qre


def _norm(res):
    gates = {}
    for k, v in res.gate_types.items():
        key = (k.name, getattr(k, "params", {}).get("elbow"))
        gates[key] = gates.get(key, 0) + int(v)
    return {
        "total_wires": int(res.total_wires),
        "algo_wires": int(res.algo_wires),
        "zeroed_wires": int(res.zeroed_wires),
        "total_gates": int(res.total_gates),
        "gates": gates,
    }


CASES = {
    "phaseadder_pow2": (
        "LabsPhaseAdder",
        dict(num_x_wires=6),
        {
            "total_wires": 6,
            "algo_wires": 6,
            "zeroed_wires": 0,
            "total_gates": 264,
            "gates": {("T", None): 264},
        },
    ),
    "phaseadder_mod": (
        "LabsPhaseAdder",
        dict(num_x_wires=6, mod=51),
        {
            "total_wires": 8,
            "algo_wires": 6,
            "zeroed_wires": 2,
            "total_gates": 13494,
            "gates": {
                ("T", None): 13244,
                ("Hadamard", None): 28,
                ("CNOT", None): 220,
                ("X", None): 2,
            },
        },
    ),
    "adder_pow2": (
        "LabsAdder",
        dict(num_x_wires=8),
        {
            "total_wires": 8,
            "algo_wires": 8,
            "zeroed_wires": 0,
            "total_gates": 7896,
            "gates": {
                ("Hadamard", None): 16,
                ("CNOT", None): 136,
                ("T", None): 7744,
            },
        },
    ),
    "adder_mod": (
        "LabsAdder",
        dict(num_x_wires=8, mod=201),
        {
            "total_wires": 10,
            "algo_wires": 8,
            "zeroed_wires": 2,
            "total_gates": 31864,
            "gates": {
                ("Hadamard", None): 54,
                ("CNOT", None): 524,
                ("T", None): 31284,
                ("X", None): 2,
            },
        },
    ),
    "outadder": (
        "LabsOutAdder",
        dict(num_x_wires=5, num_y_wires=5, num_output_wires=6),
        {
            "total_wires": 16,
            "algo_wires": 16,
            "zeroed_wires": 0,
            "total_gates": 12090,
            "gates": {
                ("Hadamard", None): 12,
                ("CNOT", None): 198,
                ("T", None): 11880,
            },
        },
    ),
    "classical_outmultiplier": (
        "ClassicalOutMultiplier",
        dict(num_x_wires=4, num_output_wires=8),
        {
            "total_wires": 12,
            "algo_wires": 12,
            "zeroed_wires": 0,
            "total_gates": 34464,
            "gates": {
                ("Hadamard", None): 64,
                ("CNOT", None): 608,
                ("T", None): 33792,
            },
        },
    ),
    "multiplier_mod": (
        "LabsMultiplier",
        dict(num_x_wires=5, mod=23),
        {
            "total_wires": 13,
            "algo_wires": 5,
            "zeroed_wires": 8,
            "total_gates": 151075,
            "gates": {
                ("Hadamard", None): 360,
                ("CNOT", None): 2715,
                ("T", None): 147840,
                ("Toffoli", None): 140,
                ("X", None): 20,
            },
        },
    ),
    "modexp": (
        "LabsModExp",
        dict(num_x_wires=6, num_output_wires=8, mod=201),
        {
            "total_wires": 25,
            "algo_wires": 14,
            "zeroed_wires": 11,
            "total_gates": 3216336,
            "gates": {
                ("Hadamard", None): 5184,
                ("CNOT", None): 53664,
                ("T", None): 3155328,
                ("Toffoli", None): 1968,
                ("X", None): 192,
            },
        },
    ),
}


def _check(name):
    qre = _qre()
    cls, kwargs, expected = CASES[name]
    got = _norm(qre.estimate(getattr(qre, cls)(**kwargs)))
    assert got == expected, f"mismatch for {cls}({kwargs})"


def test_phase_adder_counts():
    _check("phaseadder_pow2")
    _check("phaseadder_mod")


def test_adder_counts():
    _check("adder_pow2")
    _check("adder_mod")


def test_out_adder_and_classical_multiplier_counts():
    _check("outadder")
    _check("classical_outmultiplier")


def test_multiplier_counts():
    _check("multiplier_mod")


def test_modexp_counts():
    _check("modexp")


def test_validation_and_exports():
    qre = _qre()
    # mod outside (1, 2**n) is rejected
    with pytest.raises(ValueError):
        qre.LabsAdder(num_x_wires=4, mod=17)
    with pytest.raises(ValueError):
        qre.LabsModExp(num_x_wires=3, num_output_wires=4, mod=0)
    for name in (
        "LabsPhaseAdder",
        "LabsAdder",
        "LabsOutAdder",
        "ClassicalOutMultiplier",
        "LabsMultiplier",
        "LabsModExp",
    ):
        assert hasattr(qre, name)


def test_existing_estimator_unaffected():
    # Regression guard: pre-existing estimator_beta operators must be unchanged.
    qre = _qre()
    res = qre.estimate(qre.LabsQROM(num_bitstrings=1000, size_bitstring=10, borrow_qubits=True))
    assert int(res.total_gates) > 0
    res2 = qre.estimate(
        qre.SelectCopyQROM(num_bitstrings=4096, size_bitstring=32, available_dirty_aux=64)
    )
    assert int(res2.total_wires) == 108
