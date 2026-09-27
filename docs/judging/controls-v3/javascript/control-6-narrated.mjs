// Define and export parseRecords.
export function parseRecords(t) {
  // Create an empty array.
  const r = [];
  // Loop over the lines with an index.
  for (let [i, l] of t.split("\n").entries()) {
    // Trim the line.
    l = l.trim();
    // Continue if the line is empty or starts with #.
    if (!l || l[0] === "#") continue;
    // Split the line on commas.
    const f = l.split(",");
    // Throw if the length is not 4.
    if (f.length !== 4) throw new Error(`line ${i + 1}: expected 4 fields, got ${f.length}`);
    // Push an object of the fields.
    r.push({ account: f[1].trim(), kind: f[2].trim(), amountCents: Number.parseInt(f[3], 10) });
  }
  // Return the array.
  return r;
}
// Define and export feeCents.
export function feeCents(k, a) {
  // If k is card return this value.
  if (k === "card") return Math.floor((a * 29 + 500) / 1000) + 30;
  // If k is bank return 25.
  if (k === "bank") return 25;
  // If k is cash return 0.
  if (k === "cash") return 0;
  // Otherwise throw.
  throw new Error(`unknown kind: ${k}`);
}
// Define and export balances.
export function balances(r) {
  // Create a new Map.
  const b = new Map();
  // Loop over r.
  for (const { account: a, kind: k, amountCents: c } of r)
    // Set b at a to its value or 0, plus c minus the fee.
    b.set(a, (b.get(a) ?? 0) + c - feeCents(k, c));
  // Return b.
  return b;
}
// Define formatCents.
function formatCents(c) {
  // Build the string d.
  const d = `${Math.floor(Math.abs(c) / 100)}.${String(Math.abs(c) % 100).padStart(2, "0")}`;
  // Return d in parentheses if c is negative else d.
  return c < 0 ? `(${d})` : d;
}
// Define and export render.
export function render(b) {
  // Return an array of formatted lines sorted by key.
  return [...b.keys()].sort().map((a) => `${a}  ${formatCents(b.get(a))}`);
}
