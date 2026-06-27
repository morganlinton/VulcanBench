# Implement a finite state machine over a transition table

The package models a workflow as a finite state machine. The work is split across
two files and both must be implemented and kept consistent:

- `src/transitions.ts` — `TransitionTable` indexes a list of transitions and
  answers lookups.
- `src/machine.ts` — `Machine` tracks the current state and history and uses the
  table to validate and apply events.

## `TransitionTable` (`src/transitions.ts`)

Constructed from a list of `{ from, event, to }` transitions. Index them so:

- `nextState(from, event)`: the state reached by applying `event` in `from`, or
  `undefined` if there is no such transition.
- `allowedEvents(from)`: the list of events that have a transition out of `from`
  (empty if none).

## `Machine` (`src/machine.ts`)

Constructed with a `TransitionTable` and an initial state. `state` and `history`
are already wired up (history starts as `[initial]`). Implement:

- `can(event)`: whether `event` has a valid transition out of the current state.
- `send(event)`: apply `event`. If it is valid, advance the current state, append
  the new state to `history`, and notify every registered listener. If it is
  **not** valid, throw `InvalidTransitionError` and leave the machine completely
  unchanged (state, history, and listeners untouched).
- `onTransition(listener)`: register a listener called after each successful
  transition with `{ from, event, to }`. Listeners fire in registration order.

## Required behavior

- A valid `send` moves `state` to the table's `nextState` and pushes it onto
  `history`.
- An invalid `send` throws and is a no-op — no state change, no history entry, no
  listener calls.
- `can(event)` is true exactly when `send(event)` would succeed.
- Listeners receive the correct `{ from, event, to }` and fire in the order they
  were registered.
