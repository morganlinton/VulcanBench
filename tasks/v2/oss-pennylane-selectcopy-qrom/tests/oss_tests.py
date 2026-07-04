"""Hidden test for oss-pennylane-selectcopy-qrom.

``pennylane.labs.estimator_beta`` must gain ``SelectCopyQROM``: a resource
operator for the Select-Copy QROM variant of Low et al. (2024),
arXiv:1812.00954. Given ``num_bitstrings``, ``size_bitstring`` and either
``available_dirty_aux`` or an explicit ``(batch_size, bits_per_iter)`` pair, it
must report the resources of the optimal select-copy decomposition through
``qre.estimate``.

Grading is on EXACT wire and gate counts captured from the upstream
implementation (PennyLaneAI/pennylane PR #9500). The counts encode the full
cost model: the brute-force parameter optimization over batch size and
bits-per-iteration (including its eligibility sets and tie-breaking), the
left-elbow vs plain Toffoli split, the CNOT fan-out/copy accounting, and the
auxiliary wire bookkeeping. A from-scratch implementation that is faithful to
the paper but differs in any of these internal choices produces different
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


CASES = [
    (
        dict(num_bitstrings=10**8, size_bitstring=8, available_dirty_aux=300),
        {
            "total_wires": 308,
            "algo_wires": 35,
            "zeroed_wires": 273,
            "total_gates": 478148282,
            "gates": {
                ("X", None): 7035786,
                ("CNOT", None): 457037936,
                ("Toffoli", "left"): 3518130,
                ("Hadamard", None): 10554390,
                ("Toffoli", None): 2040,
            },
        },
    ),
    (
        dict(num_bitstrings=10**9, size_bitstring=16, available_dirty_aux=256),
        {
            "total_wires": 322,
            "algo_wires": 46,
            "zeroed_wires": 276,
            "total_gates": 9031293325,
            "gates": {
                ("X", None): 132821034,
                ("CNOT", None): 8632825259,
                ("Toffoli", "left"): 66410738,
                ("Hadamard", None): 199232214,
                ("Toffoli", None): 4080,
            },
        },
    ),
    (
        dict(num_bitstrings=4096, size_bitstring=32, available_dirty_aux=64),
        {
            "total_wires": 108,
            "algo_wires": 44,
            "zeroed_wires": 64,
            "total_gates": 103154,
            "gates": {
                ("X", None): 5150,
                ("CNOT", None): 87296,
                ("Toffoli", "left"): 2621,
                ("Hadamard", None): 7863,
                ("Toffoli", None): 224,
            },
        },
    ),
    (
        dict(num_bitstrings=1000, size_bitstring=10, available_dirty_aux=None),
        {
            "total_wires": 29,
            "algo_wires": 20,
            "zeroed_wires": 9,
            "total_gates": 49305,
            "gates": {
                ("X", None): 10956,
                ("CNOT", None): 16467,
                ("Toffoli", "left"): 5468,
                ("Hadamard", None): 16404,
                ("Toffoli", None): 10,
            },
        },
    ),
    (
        dict(num_bitstrings=2**20, size_bitstring=24, batch_size=8, bits_per_iter=6),
        {
            "total_wires": 102,
            "algo_wires": 44,
            "zeroed_wires": 58,
            "total_gates": 20972108,
            "gates": {
                ("X", None): 1310750,
                ("CNOT", None): 17039562,
                ("Toffoli", "left"): 655407,
                ("Hadamard", None): 1966221,
                ("Toffoli", None): 168,
            },
        },
    ),
]


def test_dirty_aux_optimized_cases():
    qre = _qre()
    for kwargs, expected in CASES[:3]:
        got = _norm(qre.estimate(qre.SelectCopyQROM(**kwargs)))
        assert got == expected, f"mismatch for {kwargs}"


def test_no_dirty_aux_default():
    qre = _qre()
    kwargs, expected = CASES[3]
    got = _norm(qre.estimate(qre.SelectCopyQROM(**kwargs)))
    assert got == expected


def test_explicit_batch_and_bits_per_iter():
    qre = _qre()
    kwargs, expected = CASES[4]
    got = _norm(qre.estimate(qre.SelectCopyQROM(**kwargs)))
    assert got == expected


def test_exported_from_estimator_beta():
    qre = _qre()
    assert hasattr(qre, "SelectCopyQROM")
    from pennylane.labs.estimator_beta.templates import SelectCopyQROM  # noqa: F401


def test_existing_labs_qrom_unaffected():
    # Regression guard: the pre-existing LabsQROM resource op must be unchanged.
    qre = _qre()
    res = qre.estimate(qre.LabsQROM(num_bitstrings=1000, size_bitstring=10, borrow_qubits=True))
    assert int(res.total_gates) > 0
    assert int(res.total_wires) > 0
