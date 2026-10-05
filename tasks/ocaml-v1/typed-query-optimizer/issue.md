# Implement capture-avoiding normalization for the typed query DSL

The query engine is used by callers with open environments as well as closed
plans. Its GADT encodes both environment shape and result type. Implement the
missing Optimizer functions without unsafe casts, Obj operations, Marshal,
external identity primitives, weakening Expr, or changing any supplied .mli.
Preserve existing interpreter and diagnostic behavior.

## Scope and substitution

Z denotes the nearest binding; S moves one environment slot outward. A renaming
maps every free variable to a variable of the same value type. A simultaneous
substitution maps every free variable to a term of the same value type. Neither
mapping applies to a Let-bound Z inside its body. Under a binder, mappings of
outer variables must be lifted into the larger target environment. Do not apply
the substitution again to terms returned by replace. Handle all constructors,
including pairs, projections, predicates, and arbitrarily nested bindings.

## Observable semantics

Eval is eager and left to right. Pair evaluates both fields before projection;
Let evaluates its value once, even if unused. If evaluates only its selected
branch. Read calls are observable: preserve their sequence, multiplicity, and
exceptions, as well as results for every well-typed environment and reader.
Never execute a Read or inspect a free variable during normalization. Keep
nonliteral Let values shared rather than duplicating their evaluation.

## Required normal form

Normalize every child recursively, then apply these rules until no rule applies:

* Fold Add, Mul, and Equal when both operands are integer literals.
* Remove additive zero and multiplicative one on either side. Multiplication by
  zero may drop the other operand only if that operand is pure.
* A Boolean-literal If selects its corresponding normalized branch.
* Fst(Pair(a,b)) becomes a only if b is pure; Snd(Pair(a,b)) becomes b only if a
  is pure. An unused Read in the other field must still run.
* A literal is Int, Bool, or a Pair recursively containing only literals.
  Eliminate a Let with a literal value by capture-avoiding substitution and
  normalize the result again. Literal pairs may contain different types.
* Eliminate a Let with a pure value if its body does not refer to the bound
  variable, removing that environment slot from free indices. Occurrences
  underneath another Let refer to this slot at the corresponding S depth.
* Keep all other Lets, particularly used nonliteral values and unused effectful
  values. Do not inline a nonliteral merely because it is pure or used once.

Purity means absence of Read anywhere in the normalized subtree. No extra
algebraic rewrites, reassociation, CSE, or distributivity are required. Results
must be structurally idempotent under Measure.fingerprint. Normalize must be
total on finite well-typed trees, with the ordinary OCaml recursion limits.

Example: Let(Read "price", Add(Var Z, Int 0)) keeps the Let and one Read.
Fst(Pair(Int 4, Read "audit")) keeps the projection and the audit read.
Let(Int 4, Let(Read "fee", Add(Var(S Z), Var Z))) substitutes the outer 4 but
preserves the inner fee binding. Open variables retain their types and scope.

No financial knowledge is assumed. Implementations are graded by independent
type clients, scope transitions, effect traces, normalization requirements,
and deterministic generated programs. Public tests remain runnable. The tests
do not require a particular internal representation or helper API.
