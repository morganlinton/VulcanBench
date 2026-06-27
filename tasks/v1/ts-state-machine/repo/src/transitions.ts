/**
 * A transition table for a finite state machine: which event moves the machine
 * from which state to which next state.
 *
 * NOTE: the lookup methods are not implemented — see issue.md. Build an index in
 * the constructor so nextState/allowedEvents are O(1)/O(events).
 */

export interface Transition {
  from: string;
  event: string;
  to: string;
}

export class TransitionTable {
  private index = new Map<string, Map<string, string>>();

  constructor(transitions: Transition[]) {
    void transitions;
  }

  /** The state reached by applying `event` in `from`, or undefined if none. */
  nextState(from: string, event: string): string | undefined {
    throw new Error("not implemented");
  }

  /** The events that have a transition defined out of `from`. */
  allowedEvents(from: string): string[] {
    throw new Error("not implemented");
  }
}
