"""Check healthy typed clients before accepting specific invalid-client failures."""

from pathlib import Path
import subprocess
import tempfile

subprocess.run(["dune", "build", "lib/query_engine.cma"], check=True)
include = str(Path("_build/default/lib/.query_engine.objs/byte").resolve())
valid = """
let plan : (int * unit, int * bool) Expr.t =
  Expr.Let (Expr.Bool true, Expr.Pair (Expr.Var (Expr.S Expr.Z), Expr.Var Expr.Z))
let result = Optimizer.normalize plan
let identity : ('env, 'env) Optimizer.renaming = {apply = (fun v -> v)}
let result = Optimizer.rename identity result
"""
invalid = {
    "result_type": "let bad : (unit, bool) Expr.t = Expr.Add (Expr.Int 1, Expr.Int 2)",
    "capture_type": "let bad : (bool * unit, int) Expr.t = Expr.Var Expr.Z",
    "mapping_type": """
let bad : (int * unit, bool * unit) Optimizer.renaming =
  {apply = (fun v -> match v with Expr.Z -> Expr.Z | Expr.S _ -> .)}
""",
}
with tempfile.TemporaryDirectory() as temporary:
    directory = Path(temporary)
    for name, source in {"valid": valid, **invalid}.items():
        path = directory / f"{name}.ml"
        path.write_text(source)
        result = subprocess.run(
            ["ocamlc", "-I", include, "-c", str(path)],
            cwd=directory,
            capture_output=True,
            text=True,
        )
        if name == "valid":
            assert result.returncode == 0, result.stderr
        else:
            assert result.returncode != 0, f"invalid client accepted: {name}"
            assert "Error:" in result.stderr, result.stderr
            assert "int" in result.stderr and "bool" in result.stderr, result.stderr
            assert "Unbound" not in result.stderr and "Syntax error" not in result.stderr
