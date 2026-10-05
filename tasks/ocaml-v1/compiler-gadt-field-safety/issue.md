# Safe and precise field classification in pattern matching

This checkout is the upstream OCaml compiler at revision
`364874344779d2e410e99eb634bea8e7e2b70159` (5.6.0+dev0). Repair a native-code
GC safety bug in pattern matching and make field classification consistent
across records, tuples, ordinary constructors, and extension constructors.

A field access can be shared by multiple rows of a pattern matrix. The typing
environment on a row can contain equations introduced by that row's GADT
patterns. For example, a locally abstract record payload can be an integer in
one row and a string in another. Classifying the shared payload as immediate
using the integer row's equations omits a native GC root in the string row.
After a minor collection moves the string, the local reference is stale.

Field classification must use only equations valid independently of individual
pattern rows. Preserve the normal typing environment for type checking.
Apply this safety rule to record fields, tuple components, constructor
arguments, and extension arguments, in either row order and under nesting.
If a tuple shape is visible only through a row-local equation, use conservative
pointer classification rather than assuming the shape remains available.

Retain precision where types justify it without row-local equations: known
integer, boolean, character, and immediate-annotated types must use immediate
field reads, including inside manifest aliases. Heap values must remain pointer
reads. Ordinary constructor arguments start at offset zero; extension arguments
start after the extension descriptor. Preserve record mutability, inlined-record
and unboxed handling, evaluation order, and match side effects. Blanket pointer
classification is an incomplete solution.

Preserve the supplied interfaces. `typing/env.mli` may gain helper declarations
while keeping existing declarations intact. Update dependencies and existing
public test expectations as needed, and add a focused regression test.

The environment contains a public build of this exact pre-fix revision.
Generated artifacts are installed once; original source files are never seeded
or overwritten. Run `make -j1 coreall opt-core ocamlc.opt ocamlopt.opt testsuite/tools/expect` after changes to rebuild both compiler
frontends. For standalone clients use `OCAMLLIB=$PWD/stdlib ./ocamlopt.opt ...`
or `OCAMLLIB=$PWD/stdlib ./ocamlc.opt ...`. `-dlambda` exposes `field_int` and
`field_imm` classifications. Public tests live in `testsuite/tests`; individual
tests can be run with
`OCAMLSRCDIR=$PWD make -C testsuite one TEST=tests/<directory>/<file>.ml`.
The source-directory override is required because the public baseline build
was produced at a different absolute path.
No network is needed to build or run tests. Do not change the runtime or discard
GADT typing constraints as a workaround.
