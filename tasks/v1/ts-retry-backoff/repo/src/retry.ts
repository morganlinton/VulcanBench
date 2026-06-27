/**
 * Retry an async operation with exponential backoff.
 *
 * NOTE: this implementation has two bugs — it runs one fewer attempt than
 * requested (so an operation that would succeed on its final allowed attempt is
 * reported as failed), and it does not cap the backoff delay at `maxDelayMs`
 * (delays grow without bound). See issue.md.
 */

export interface RetryOptions {
  /** Total number of times to call the operation (first call plus retries). */
  attempts: number;
  /** Base backoff delay in milliseconds. */
  baseDelayMs: number;
  /** Upper bound on any single backoff delay, in milliseconds. */
  maxDelayMs: number;
  /** Injectable sleep (defaults to a real timer); tests pass a recorder. */
  sleep?: (ms: number) => Promise<void>;
}

const realSleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));

export async function retry<T>(
  fn: () => Promise<T>,
  options: RetryOptions,
): Promise<T> {
  const { attempts, baseDelayMs, maxDelayMs } = options;
  const sleep = options.sleep ?? realSleep;
  let lastError: unknown;
  for (let attempt = 1; attempt <= attempts - 1; attempt++) {
    try {
      return await fn();
    } catch (err) {
      lastError = err;
      if (attempt < attempts) {
        const delay = baseDelayMs * 2 ** (attempt - 1);
        await sleep(delay);
      }
    }
  }
  throw lastError;
}
