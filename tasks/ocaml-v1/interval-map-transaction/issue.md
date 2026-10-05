# Preserve a persistent interval map through overlapping batch edits

Implement interval assignment and transactional batch application. All ordering
must use the supplied Base comparator, not polymorphic comparison. Intervals are
half-open in that ordering: [lower, upper). A reversed interval returns
`Error "reversed interval"`; equal endpoints are a no-op. `None` erases a range.
Assignments preserve values outside the assigned interval and prior map versions.
`boundaries` returns sorted change points: no leading None, no consecutive values
considered equal by the supplied equality function, and a None boundary where a
finite range ends. Equal adjacent values coalesce. Examples: assign [0,10)=A,
then [3,7)=B yields 0:A,3:B,7:A,10:None; erasing [2,8) retains 0:A,2:None,8:A,10:None.

`Batch.apply` applies edits in order, returns the first error, and never exposes
partial success. Preserve existing `.mli` contracts, comparator witness typing,
and persistence. Do not use unsafe casts. Public tests run with `dune runtest`.

The supplied `.mli` files are fixed contracts for this implementation task.
Keep their contents unchanged; implement the change in `.ml` files.


Local edits must also preserve practical work and allocation bounds. In the native
build, a lookup in a map with 4096 canonical boundaries may call the supplied key
comparator at most 64 times. Replacing or erasing one unit range in that map may
call it at most 512 times and allocate less than 32768 bytes per edit. The checker
constructs 2048 disjoint ranges [3*i,3*i+1), then measures local edits after
construction and a full GC. Construction, boundary enumeration, and post-edit
queries are outside the allocation measurement. These are comparator-work and
allocation contracts, not wall-clock limits. Preserve generic comparator ordering
and persistence; do not special-case benchmark inputs or bypass the comparator.
