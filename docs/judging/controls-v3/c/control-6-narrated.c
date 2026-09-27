// Include ctype.h.
#include <ctype.h>
// Include stdbool.h.
#include <stdbool.h>
// Include stdio.h.
#include <stdio.h>
// Include stdlib.h.
#include <stdlib.h>
// Include string.h.
#include <string.h>
// Define LEDGER_MAX_RECORDS as 256.
#define LEDGER_MAX_RECORDS 256
// Define LEDGER_NAME_MAX as 32.
#define LEDGER_NAME_MAX 32
// Define the ledger_status enum.
enum ledger_status { LEDGER_OK = 0, LEDGER_E_FIELDS, LEDGER_E_KIND };
// Define struct record.
struct record {
  char account[LEDGER_NAME_MAX];
  char kind[LEDGER_NAME_MAX];
  long long amount_cents;
};
// Define struct balance.
struct balance {
  char account[LEDGER_NAME_MAX];
  long long cents;
};
// Define copy_trimmed.
static bool copy_trimmed(char d[LEDGER_NAME_MAX], const char *s, const char *e) {
  // Advance s while it points at a space.
  while (s < e && isspace((unsigned char)*s))
    s++;
  // Move e back while the character before it is a space.
  while (e > s && isspace((unsigned char)e[-1]))
    e--;
  // Set n to the length from s to e.
  size_t n = (size_t)(e - s);
  // Return false if n is too big.
  if (n >= LEDGER_NAME_MAX)
    return false;
  // Copy n bytes from s to d.
  memcpy(d, s, n);
  // Set d[n] to 0.
  d[n] = 0;
  // Return true.
  return true;
}
// Define parse_amount.
static bool parse_amount(long long *v, const char *s, const char *e) {
  // Declare a buffer b.
  char b[LEDGER_NAME_MAX];
  // Declare a pointer r.
  char *r;
  // Return false if copy_trimmed fails.
  if (!copy_trimmed(b, s, e))
    return false;
  // Set *v to strtoll of b.
  *v = strtoll(b, &r, 10);
  // Return whether r moved and points at the end.
  return r != b && !*r;
}
// Define parse_records.
enum ledger_status parse_records(const char *t, struct record *o, size_t *c, int *l) {
  // Set n to 0.
  size_t n = 0;
  // Set i to 0.
  int i = 0;
  // Loop over the text with x until the end.
  for (const char *x = t; *x;) {
    // Set e to the end of the line and s to x.
    const char *e = x + strcspn(x, "\n"), *s = x;
    // Increment i.
    i++;
    // Advance s while it points at a space.
    while (s < e && isspace((unsigned char)*s))
      s++;
    // If s is before e and not a #.
    if (s < e && *s != '#') {
      // Create an array f of four pointers starting with x.
      const char *f[4] = {x};
      // Set k to 1.
      size_t k = 1;
      // Loop over the line with p.
      for (const char *p = x; p < e; p++) {
        // Continue if p is not a comma.
        if (*p != ',')
          continue;
        // If k is less than 4 set f[k] to the next character.
        if (k < 4)
          f[k] = p + 1;
        // Increment k.
        k++;
      }
      // If k is not 4 or n is full or a copy fails.
      if (k != 4 || n == LEDGER_MAX_RECORDS || !copy_trimmed(o[n].account, f[1], f[2] - 1) ||
          !copy_trimmed(o[n].kind, f[2], f[3] - 1) || !parse_amount(&o[n].amount_cents, f[3], e)) {
        // Set *l to i.
        *l = i;
        // Return LEDGER_E_FIELDS.
        return LEDGER_E_FIELDS;
      }
      // Increment n.
      n++;
    }
    // Set x to the character after e, or e.
    x = *e == '\n' ? e + 1 : e;
  }
  // Set *c to n.
  *c = n;
  // Return LEDGER_OK.
  return LEDGER_OK;
}
// Define fee_cents.
enum ledger_status fee_cents(const char *k, long long a, long long *f) {
  // If k is card.
  if (!strcmp(k, "card")) {
    // Set s to a times 29 plus 500.
    long long s = a * 29 + 500;
    // Set *f to this value.
    *f = s / 1000 - (s % 1000 < 0) + 30;
    // Return LEDGER_OK.
    return LEDGER_OK;
  }
  // If k is bank.
  if (!strcmp(k, "bank")) {
    // Set *f to 25.
    *f = 25;
    // Return LEDGER_OK.
    return LEDGER_OK;
  }
  // If k is cash.
  if (!strcmp(k, "cash")) {
    // Set *f to 0.
    *f = 0;
    // Return LEDGER_OK.
    return LEDGER_OK;
  }
  // Otherwise return LEDGER_E_KIND.
  return LEDGER_E_KIND;
}
// Define balances.
enum ledger_status balances(const struct record *r, size_t c, struct balance *o, size_t *m) {
  // Set n to 0.
  size_t n = 0;
  // Loop over r with i.
  for (size_t i = 0; i < c; i++) {
    // Declare f.
    long long f;
    // Set s to fee_cents of the record.
    enum ledger_status s = fee_cents(r[i].kind, r[i].amount_cents, &f);
    // Return s if it is not LEDGER_OK.
    if (s != LEDGER_OK)
      return s;
    // Set j to 0.
    size_t j = 0;
    // Increment j while the account at j is different.
    while (j < n && strcmp(o[j].account, r[i].account))
      j++;
    // If j is n.
    if (j == n) {
      // Copy the account into o[n].
      snprintf(o[n].account, sizeof o[n].account, "%s", r[i].account);
      // Set o[n].cents to 0 and increment n.
      o[n++].cents = 0;
    }
    // Add the amount minus f to o[j].cents.
    o[j].cents += r[i].amount_cents - f;
  }
  // Set *m to n.
  *m = n;
  // Return LEDGER_OK.
  return LEDGER_OK;
}
// Define format_cents.
static void format_cents(long long c, char *o, size_t z) {
  // Set m to the absolute value of c.
  long long m = llabs(c);
  // Print m into o with parentheses if c is negative.
  snprintf(o, z, c < 0 ? "(%lld.%02lld)" : "%lld.%02lld", m / 100, m % 100);
}
// Define compare_accounts.
static int compare_accounts(const void *a, const void *b) {
  // Return strcmp of the two accounts.
  return strcmp(((const struct balance *)a)->account, ((const struct balance *)b)->account);
}
// Define render.
void render(const struct balance *b, size_t c, char *o, size_t z) {
  // Declare an array s.
  struct balance s[LEDGER_MAX_RECORDS];
  // Set u to 0.
  size_t u = 0;
  // If c is too big set it to LEDGER_MAX_RECORDS.
  if (c > LEDGER_MAX_RECORDS)
    c = LEDGER_MAX_RECORDS;
  // If z is more than 0 set o[0] to 0.
  if (z > 0)
    o[0] = 0;
  // Copy b into s.
  memcpy(s, b, c * sizeof *b);
  // Sort s with compare_accounts.
  qsort(s, c, sizeof *s, compare_accounts);
  // Loop over s with i while there is room.
  for (size_t i = 0; i < c && u < z; i++) {
    // Declare a buffer d.
    char d[32];
    // Format the cents into d.
    format_cents(s[i].cents, d, sizeof d);
    // Print the line into o at u.
    int w = snprintf(o + u, z - u, "%s%s  %s", i > 0 ? "\n" : "", s[i].account, d);
    // Break if w is negative.
    if (w < 0)
      break;
    // Add w to u.
    u += (size_t)w;
  }
}
