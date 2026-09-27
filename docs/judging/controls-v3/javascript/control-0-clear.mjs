/** @file Ledger: parse transaction lines, apply per-kind fees, and render balances. */

const CARD_FEE_PERMILLE = 29; // 2.9 percent of the amount
const CARD_FEE_FIXED_CENTS = 30;
const BANK_FEE_CENTS = 25;

/** Return { account, kind, amountCents } records, skipping blank lines and # comments. */
export function parseRecords(text) {
  const records = [];
  for (const [index, raw] of text.split("\n").entries()) {
    const lineNumber = index + 1;
    const line = raw.trim();
    if (!line || line.startsWith("#")) {
      continue;
    }
    const fields = line.split(",");
    if (fields.length !== 4) {
      throw new Error(`line ${lineNumber}: expected 4 fields, got ${fields.length}`);
    }
    const [_date, account, kind, amount] = fields;
    records.push({
      account: account.trim(),
      kind: kind.trim(),
      amountCents: Number.parseInt(amount, 10),
    });
  }
  return records;
}

/** Fee for one transaction. The percentage part rounds half up to whole cents. */
export function feeCents(kind, amountCents) {
  if (kind === "card") {
    const percentage = Math.floor((amountCents * CARD_FEE_PERMILLE + 500) / 1000);
    return percentage + CARD_FEE_FIXED_CENTS;
  }
  if (kind === "bank") {
    return BANK_FEE_CENTS;
  }
  if (kind === "cash") {
    return 0;
  }
  throw new Error(`unknown kind: ${kind}`);
}

/** Sum amount minus fee for each account. */
export function balances(records) {
  const totals = new Map();
  for (const { account, kind, amountCents } of records) {
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

export function render(totals) {
  return [...totals.keys()]
    .sort()
    .map((account) => `${account}  ${formatCents(totals.get(account))}`);
}
