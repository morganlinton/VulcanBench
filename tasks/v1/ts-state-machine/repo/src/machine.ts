/**
 * A finite state machine driven by a TransitionTable.
 *
 * NOTE: can/send/onTransition are not implemented — see issue.md. The current
 * state and history are set up in the constructor.
 */

import { TransitionTable } from "./transitions.ts";

export class InvalidTransitionError extends Error {}

export interface TransitionEvent {
  from: string;
  event: string;
  to: string;
}

export type Listener = (e: TransitionEvent) => void;

export class Machine {
  private table: TransitionTable;
  private current: string;
  private trail: string[];
  private listeners: Listener[] = [];

  constructor(table: TransitionTable, initial: string) {
    this.table = table;
    this.current = initial;
    this.trail = [initial];
  }

  get state(): string {
    return this.current;
  }

  get history(): string[] {
    return [...this.trail];
  }

  /** Whether `event` has a valid transition out of the current state. */
  can(event: string): boolean {
    throw new Error("not implemented");
  }

  /** Apply `event`, advancing the state. Throws InvalidTransitionError if the
   *  event is not valid in the current state (leaving the machine unchanged). */
  send(event: string): void {
    throw new Error("not implemented");
  }

  /** Register a listener invoked after every successful transition. */
  onTransition(listener: Listener): void {
    throw new Error("not implemented");
  }
}
