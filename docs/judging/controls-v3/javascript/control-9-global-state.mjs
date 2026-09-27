/** @file Ledger: parse transaction lines, apply per-kind fees, and render balances. */

const records = [];
const totals = new Map();
const errors = [];

/** Load records into the module. Returns them for convenience. */
export function parseRecords(text) {
  records.length = 0;
  for (const [index, raw] of text.split("\n").entries()) {
    const lineNumber = index + 1;
    const line = raw.trim();
    if (!line || line.startsWith("#")) {
      continue;
    }
    const fields = line.split(",");
    if (fields.length !== 4) {
      errors.push(lineNumber);
      throw new Error(`line ${lineNumber}: expected 4 fields, got ${fields.length}`);
    }
    records.push({
      account: fields[1].trim(),
      kind: fields[2].trim(),
      amountCents: Number.parseInt(fields[3], 10),
    });
  }
  return records;
}

/** Fee for one transaction. The percentage part rounds half up to whole cents. */
export function feeCents(kind, amountCents) {
  if (kind === "card") {
    return Math.floor((amountCents * 29 + 500) / 1000) + 30;
  }
  if (kind === "bank") {
    return 25;
  }
  if (kind === "cash") {
    return 0;
  }
  throw new Error(`unknown kind: ${kind}`);
}

/** Recompute module totals from the loaded records, or from the given array. */
export function balances(current) {
  totals.clear();
  for (const { account, kind, amountCents } of current ?? records) {
    totals.set(account, (totals.get(account) ?? 0) + amountCents - feeCents(kind, amountCents));
  }
  return totals;
}

/** Dollars with two decimals; negatives in parentheses, for example (1.25). */
function formatCents(cents) {
  const magnitude = Math.abs(cents);
  const dollars = `${Math.floor(magnitude / 100)}.${String(magnitude % 100).padStart(2, "0")}`;
  return cents < 0 ? `(${dollars})` : dollars;
}

/** Render the module totals unless a Map is given. */
export function render(current) {
  const source = current ?? totals;
  return [...source.keys()]
    .sort()
    .map((account) => `${account}  ${formatCents(source.get(account))}`);
}
