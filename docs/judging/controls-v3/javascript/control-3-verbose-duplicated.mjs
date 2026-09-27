// This module is the ledger module. It handles the ledger.

export function parseRecords(text) {
  // First we make an empty array to hold the records.
  const records = [];
  // Now we go through every line.
  let lineNumber = 0;
  for (const raw of text.split("\n")) {
    lineNumber = lineNumber + 1;
    const line = raw.trim();
    // Skip the line if it is blank.
    if (line === "") {
      continue;
    }
    // Skip the line if it is a comment.
    if (line.startsWith("#")) {
      continue;
    }
    const fields = line.split(",");
    // Check the number of fields.
    if (fields.length !== 4) {
      throw new Error(`line ${lineNumber}: expected 4 fields, got ${fields.length}`);
    }
    const account = fields[1].trim();
    const kind = fields[2].trim();
    const amount = Number.parseInt(fields[3], 10);
    // Validate the kind here so bad kinds are caught early.
    if (kind === "card") {
      // Nothing to do.
    } else if (kind === "bank") {
      // Nothing to do.
    } else if (kind === "cash") {
      // Nothing to do.
    } else {
      throw new Error(`unknown kind: ${kind}`);
    }
    records.push({ account: account, kind: kind, amountCents: amount });
  }
  return records;
}

export function feeCents(kind, amountCents) {
  // Work out the fee for one transaction.
  let fee = 0;
  if (kind === "card") {
    fee = Math.floor((amountCents * 29 + 500) / 1000);
    fee = fee + 30;
  } else if (kind === "bank") {
    fee = 25;
  } else if (kind === "cash") {
    fee = 0;
  } else {
    throw new Error(`unknown kind: ${kind}`);
  }
  return fee;
}

export function balances(records) {
  // Start with an empty Map of totals.
  const totals = new Map();
  for (const record of records) {
    const account = record.account;
    const kind = record.kind;
    const amount = record.amountCents;
    let fee = 0;
    let net = 0;
    // Work out the fee for a card.
    if (kind === "card") {
      fee = Math.floor((amount * 29 + 500) / 1000);
      fee = fee + 30;
      net = amount - fee;
    }
    // Work out the fee for a bank transfer.
    else if (kind === "bank") {
      fee = 25;
      net = amount - fee;
    }
    // Work out the fee for cash.
    else if (kind === "cash") {
      fee = 0;
      net = amount - fee;
    } else {
      throw new Error(`unknown kind: ${kind}`);
    }
    // Add the net amount to the account.
    if (totals.has(account)) {
      totals.set(account, totals.get(account) + net);
    } else {
      totals.set(account, net);
    }
  }
  return totals;
}

export function totalFees(records) {
  // Sum all the fees. Uses the same rules as balances.
  let fees = 0;
  for (const record of records) {
    const kind = record.kind;
    const amount = record.amountCents;
    if (kind === "card") {
      fees = fees + Math.floor((amount * 29 + 500) / 1000) + 30;
    } else if (kind === "bank") {
      fees = fees + 25;
    } else if (kind === "cash") {
      fees = fees + 0;
    } else {
      throw new Error(`unknown kind: ${kind}`);
    }
  }
  return fees;
}

export function render(totals) {
  // Build the output lines.
  const lines = [];
  const accounts = Array.from(totals.keys());
  accounts.sort();
  for (const account of accounts) {
    let cents = totals.get(account);
    // Format negative numbers with parentheses.
    if (cents < 0) {
      cents = -cents;
      const dollars = String(Math.floor(cents / 100)) + "." + String(cents % 100).padStart(2, "0");
      lines.push(account + "  (" + dollars + ")");
    } else {
      const dollars = String(Math.floor(cents / 100)) + "." + String(cents % 100).padStart(2, "0");
      lines.push(account + "  " + dollars);
    }
  }
  return lines;
}
