"""A bytecode compiler and stack VM whose only coupling is a shared instruction set.

``vmlang/compiler.py`` lowers an AST to a flat instruction list; ``vmlang/vm.py``
executes that list. The two files share no code, only the documented opcode
contract, so they must agree exactly. Short-circuit ``and``/``or`` and ``if`` need
forward jumps with backpatched absolute targets; getting an offset wrong shows up
only when the VM runs the code.
"""

import pytest

from vmlang import compile_expr, run


def _eval(ast, env=None):
    return run(compile_expr(ast), env)


def test_construct():
    # passes pre-fix: importable, no compilation/execution yet
    assert callable(compile_expr) and callable(run)


def test_arithmetic():
    ast = ("+", ("*", ("lit", 2), ("lit", 3)), ("lit", 4))  # 2*3 + 4
    assert _eval(ast) == 10


def test_variables_and_compare():
    ast = (">", ("var", "x"), ("lit", 5))
    assert _eval(ast, {"x": 9}) is True
    assert _eval(ast, {"x": 1}) is False


def test_unknown_variable_raises():
    with pytest.raises(NameError):
        _eval(("var", "missing"))


def test_not():
    assert _eval(("not", ("lit", 0))) is True
    assert _eval(("not", ("lit", 7))) is False


def test_if_selects_branch_by_condition():
    ast = ("if", ("var", "c"), ("lit", "yes"), ("lit", "no"))
    assert _eval(ast, {"c": True}) == "yes"
    assert _eval(ast, {"c": False}) == "no"


def test_nested_if_jump_targets():
    # nested ifs stress that backpatched jump targets stay correct as code grows
    inner = ("if", ("var", "b"), ("lit", 2), ("lit", 3))
    ast = ("if", ("var", "a"), inner, ("lit", 9))
    assert _eval(ast, {"a": True, "b": True}) == 2
    assert _eval(ast, {"a": True, "b": False}) == 3
    assert _eval(ast, {"a": False, "b": True}) == 9


def test_and_or_values():
    assert _eval(("and", ("lit", 1), ("lit", 2))) == 2     # both truthy -> right
    assert _eval(("and", ("lit", 0), ("lit", 2))) == 0     # left falsy -> left
    assert _eval(("or", ("lit", 0), ("lit", 5))) == 5      # left falsy -> right
    assert _eval(("or", ("lit", 7), ("lit", 5))) == 7      # left truthy -> left


def test_short_circuit_skips_faulty_branch():
    # if the skipped operand were executed it would divide by zero / load a missing var
    boom_div = ("/", ("lit", 1), ("lit", 0))
    boom_var = ("var", "nope")
    assert _eval(("or", ("lit", 1), boom_div)) == 1    # right never runs
    assert _eval(("and", ("lit", 0), boom_var)) == 0   # right never runs


def test_compiles_to_flat_isa_instructions():
    code = compile_expr(("if", ("var", "c"), ("lit", 1), ("+", ("lit", 2), ("lit", 3))))
    isa = {"PUSH", "LOAD", "ADD", "SUB", "MUL", "DIV", "EQ", "LT", "GT",
           "NOT", "DUP", "POP", "JUMP", "JUMP_IF_FALSE"}
    assert isinstance(code, list) and code, "compile_expr must return a non-empty list"
    for instr in code:
        assert isinstance(instr, tuple) and instr, "each instruction is a non-empty tuple"
        assert instr[0] in isa, f"opcode {instr[0]!r} is not in the documented ISA"
    # jump targets must be valid absolute indices into the flat list
    for instr in code:
        if instr[0] in {"JUMP", "JUMP_IF_FALSE"}:
            assert isinstance(instr[1], int) and 0 <= instr[1] <= len(code)
