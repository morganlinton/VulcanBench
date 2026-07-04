TypeVarTuple defaults are mishandled in several ways when checking generic classes.

Given type variables with defaults, e.g.:

```python
from typing import Generic, Tuple, TypeVar
from typing_extensions import TypeVarTuple, Unpack

T1 = TypeVar("T1")
T3 = TypeVar("T3", default=str)
Ts1 = TypeVarTuple("Ts1")
Ts2 = TypeVarTuple("Ts2", default=Unpack[Tuple[int, str]])
Ts4 = TypeVarTuple("Ts4", default=Unpack[Tuple[()]])
```

1. Referencing a generic class bare (without type arguments) does not apply a TypeVarTuple's default properly. `a: ClassC1` where `class ClassC1(Generic[Unpack[Ts2]])` reveals the mangled type `ClassC1[*tuple[*tuple[int, str], ...]]` instead of `ClassC1[int, str]`. The default is being treated like a homogeneous-tuple fallback instead of being substituted as the concrete default.

2. The same goes for an empty-tuple default: `a: ClassC3` where `class ClassC3(Generic[T3, Unpack[Ts4]])` reveals `ClassC3[str, *tuple[*tuple[()], ...]]` instead of `ClassC3[str]`.

3. A TypeVar with a default is silently accepted after a TypeVarTuple, e.g. `class ClassC4(Generic[T1, Unpack[Ts1], T3])`. Since the arguments consumed by the TypeVarTuple are unbounded, a defaulted TypeVar after it can never be resolved positionally; this declaration must be rejected with an error ("A type variable with default cannot follow TypeVarTuple"), and the class treated as having erased parameters so downstream use does not produce bogus types.

Explicitly provided type arguments must keep resolving exactly as they do today, and instance creation (`ClassC1()`, `ClassC1[float]()`) must behave consistently with annotations.
