import { test } from "node:test";
import assert from "node:assert/strict";

import { TransitionTable } from "../src/transitions.ts";
import { InvalidTransitionError, Machine, type TransitionEvent } from "../src/machine.ts";

// An order workflow used across the tests.
function orderTable(): TransitionTable {
  return new TransitionTable([
    { from: "pending", event: "pay", to: "paid" },
    { from: "pending", event: "cancel", to: "cancelled" },
    { from: "paid", event: "ship", to: "shipped" },
    { from: "paid", event: "refund", to: "cancelled" },
    { from: "shipped", event: "deliver", to: "delivered" },
  ]);
}

test("initial state and history", () => {
  const m = new Machine(orderTable(), "pending");
  assert.equal(m.state, "pending");
  assert.deepEqual(m.history, ["pending"]);
});

test("table nextState and allowedEvents", () => {
  const t = orderTable();
  assert.equal(t.nextState("pending", "pay"), "paid");
  assert.equal(t.nextState("paid", "ship"), "shipped");
  assert.equal(t.nextState("pending", "ship"), undefined);
  assert.deepEqual(t.allowedEvents("pending").sort(), ["cancel", "pay"]);
  assert.deepEqual(t.allowedEvents("delivered"), []);
});

test("valid transitions advance state and record history", () => {
  const m = new Machine(orderTable(), "pending");
  m.send("pay");
  m.send("ship");
  m.send("deliver");
  assert.equal(m.state, "delivered");
  assert.deepEqual(m.history, ["pending", "paid", "shipped", "delivered"]);
});

test("invalid transition throws and leaves the machine unchanged", () => {
  const m = new Machine(orderTable(), "pending");
  assert.throws(() => m.send("ship"), InvalidTransitionError);
  assert.equal(m.state, "pending");
  assert.deepEqual(m.history, ["pending"]);
});

test("can reflects the allowed events of the current state", () => {
  const m = new Machine(orderTable(), "pending");
  assert.equal(m.can("pay"), true);
  assert.equal(m.can("ship"), false);
  m.send("pay");
  assert.equal(m.can("ship"), true);
  assert.equal(m.can("pay"), false);
});

test("onTransition notifies listeners with the payload, in order", () => {
  const m = new Machine(orderTable(), "pending");
  const seen: TransitionEvent[] = [];
  m.onTransition((e) => seen.push(e));
  m.onTransition((e) => seen.push({ ...e, event: e.event + "!" }));
  m.send("pay");
  assert.deepEqual(seen, [
    { from: "pending", event: "pay", to: "paid" },
    { from: "pending", event: "pay!", to: "paid" },
  ]);
});
