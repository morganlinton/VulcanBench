# Atomic edits and dynamic dependencies in an incremental graph

Implement Graph using the installed Incremental library. A graph begins with
named integer sources; duplicate initial names raise Invalid_argument. Formulas
use Const, Ref, Add, or Select. Select reads a named integer: zero selects its
first expression, nonzero its second. Only the selected branch is evaluated.

`transaction` updates existing sources and adds/replaces named formulas. Validate
against the complete prospective formula set, so forward references in a batch
are legal. No mutation occurs on errors. Duplicate names within either edit list
return `Error "duplicate edit"`. Editing a nonexistent source, using a source name
as a formula name, or referencing any unknown name returns `Error "unknown reference"`.
Cycles return `Error "cycle"`. For validation, all references in both Select
branches count as edges, even when a branch is inactive. Error precedence is
duplicate edit, then unknown reference, then cycle. Sources cannot be added by
transaction and formulas cannot replace sources.

Subscriptions remain attached to their named node when its formula changes.
After the first stabilization, value returns the most recently stabilized value:
transaction must not change that value until the next stabilize. Repeated
stabilization without changes does no work. `additions` is a cumulative count of
actual Add evaluations. Changes to inactive branches do not evaluate additions
in the active output; assigning the same source/formula again does no work.
If an upstream computed value stays equal, unchanged downstream additions do not
recompute. An unobserved formula should remain unevaluated. Do not merely hide
work from the counter: increment it on each evaluation of Add.

Example: sources flag=0,a=2,b=9, formula selected=Select(flag,Ref a,Ref b),
formula result=Add(Ref selected,Const 1). Observing result gives 3. Updating b
while flag=0 leaves result at 3 with no new Add evaluation. Switching flag to 1
uses b's latest value. Preserve interfaces and type safety; no unsafe casts.

The supplied `.mli` files are fixed contracts for this implementation task.
Keep their contents unchanged; implement the change in `.ml` files.
