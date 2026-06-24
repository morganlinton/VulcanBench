# `evaluate` ignores operator precedence and associativity

`calc.evaluate(text)` tokenizes, parses, and evaluates an arithmetic
expression. The tokenizer (`calc/tokens.py`) and the tree-walking evaluator
(`calc/evaluator.py`) are correct, but the parser (`calc/parser.py`) folds every
operator at a single precedence level, left to right. As a result mixed
expressions evaluate wrongly:

```
evaluate("2 + 3 * 4")    # returns 20, should be 14
evaluate("2 ** 3 ** 2")  # returns 64, should be 512
evaluate("-2 ** 2")      # returns 4, should be -4
```

Fix the parser so it builds a tree with the following precedence, lowest to
highest:

| Level | Operators | Associativity |
|-------|-----------|---------------|
| 1 (lowest)  | `+` `-`            | left |
| 2           | `*` `/` `//` `%`   | left |
| 3           | unary `+` `-`      | binds tighter than the binary operators above |
| 4 (highest) | `**`               | **right**-associative |

Required semantics:

- `**` is right-associative: `2 ** 3 ** 2 == 2 ** (3 ** 2) == 512`.
- Unary minus binds *looser* than `**`, so `-2 ** 2 == -(2 ** 2) == -4`, but the
  exponent itself is a unary factor, so `2 ** -2 == 0.25` is valid.
- `/` is true division (`1 + 6 / 2 == 4.0`), `//` is floor division, `%` is
  modulo. Division, floor-division, or modulo by zero raises `calc.CalcError`.
- Parentheses override precedence as usual.
- Malformed input (empty, unbalanced parentheses, a trailing or unexpected
  token) raises `calc.CalcError`.

Only `calc/parser.py` needs to change. The AST node types live in
`calc/ast_nodes.py`.
