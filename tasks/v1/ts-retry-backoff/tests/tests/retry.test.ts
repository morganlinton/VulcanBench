import { test } from "node:test";
import assert from "node:assert/strict";

import { retry, type RetryOptions } from "../src/retry.ts";

// A sleep recorder: resolves immediately, but remembers the delays it was asked
// to wait, so backoff timing is observable without real timers.
function recorder(): { delays: number[]; sleep: RetryOptions["sleep"] } {
  const delays: number[] = [];
  return {
    delays,
    sleep: (ms: number) => {
      delays.push(ms);
      return Promise.resolve();
    },
  };
}

// fn that rejects the first `failures` times, then resolves with `value`.
function flaky<T>(failures: number, value: T): { fn: () => Promise<T>; calls: () => number } {
  let calls = 0;
  return {
    fn: () => {
      calls += 1;
      if (calls <= failures) {
        return Promise.reject(new Error(`fail #${calls}`));
      }
      return Promise.resolve(value);
    },
    calls: () => calls,
  };
}

test("returns immediately on first success", async () => {
  const rec = recorder();
  const { fn } = flaky(0, "ok");
  const result = await retry(fn, { attempts: 3, baseDelayMs: 10, maxDelayMs: 100, sleep: rec.sleep });
  assert.equal(result, "ok");
  assert.deepEqual(rec.delays, []);
});

test("retries then succeeds early", async () => {
  const rec = recorder();
  const { fn, calls } = flaky(1, "ok"); // fails once, succeeds on attempt 2
  const result = await retry(fn, { attempts: 5, baseDelayMs: 10, maxDelayMs: 100, sleep: rec.sleep });
  assert.equal(result, "ok");
  assert.equal(calls(), 2);
  assert.deepEqual(rec.delays, [10]);
});

test("succeeds on the last allowed attempt", async () => {
  const rec = recorder();
  const { fn, calls } = flaky(2, "ok"); // fails twice, succeeds on attempt 3
  const result = await retry(fn, { attempts: 3, baseDelayMs: 10, maxDelayMs: 100, sleep: rec.sleep });
  assert.equal(result, "ok");
  assert.equal(calls(), 3); // the final allowed attempt must run
});

test("caps backoff delay at maxDelayMs", async () => {
  const rec = recorder();
  const { fn } = flaky(99, "never"); // always fails
  await assert.rejects(
    retry(fn, { attempts: 6, baseDelayMs: 10, maxDelayMs: 50, sleep: rec.sleep }),
  );
  // delays before attempts 2..6: 10, 20, 40, then capped at 50, 50
  assert.deepEqual(rec.delays, [10, 20, 40, 50, 50]);
});

test("throws the last error after exhausting all attempts", async () => {
  const rec = recorder();
  const { fn, calls } = flaky(99, "never");
  await assert.rejects(
    retry(fn, { attempts: 4, baseDelayMs: 10, maxDelayMs: 100, sleep: rec.sleep }),
    /fail #4/, // the most recent failure, after exactly 4 calls
  );
  assert.equal(calls(), 4);
});
