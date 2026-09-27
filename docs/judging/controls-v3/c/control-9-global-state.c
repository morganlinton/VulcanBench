/* Ledger: parse transaction lines, apply per-kind fees, and render balances. */

#include <ctype.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define LEDGER_MAX_RECORDS 256
#define LEDGER_NAME_MAX 32 /* account and kind, including the terminator */

enum ledger_status { LEDGER_OK = 0, LEDGER_E_FIELDS, LEDGER_E_KIND };

struct record {
    char account[LEDGER_NAME_MAX];
    char kind[LEDGER_NAME_MAX];
    long long amount_cents;
};

struct balance {
    char account[LEDGER_NAME_MAX];
    long long cents;
};

static struct record records[LEDGER_MAX_RECORDS];
static size_t record_count;
static struct balance totals[LEDGER_MAX_RECORDS];
static size_t total_count;
static int errors[LEDGER_MAX_RECORDS];
static size_t error_count;

/** Copy [start, end) into dst without surrounding whitespace; false if it does not fit. */
static bool copy_trimmed(char dst[LEDGER_NAME_MAX], const char *start, const char *end) {
    while (start < end && isspace((unsigned char)*start))
        start++;
    while (end > start && isspace((unsigned char)end[-1]))
        end--;
    size_t length = (size_t)(end - start);
    if (length >= LEDGER_NAME_MAX)
        return false;
    memcpy(dst, start, length);
    dst[length] = '\0';
    return true;
}

/** Parse [start, end) as a whole decimal integer; false if it is not one. */
static bool parse_amount(long long *amount, const char *start, const char *end) {
    char digits[LEDGER_NAME_MAX];
    char *rest;
    if (!copy_trimmed(digits, start, end))
        return false;
    *amount = strtoll(digits, &rest, 10);
    return rest != digits && *rest == '\0';
}

/** Load records into the module. Copies them to out for convenience. */
enum ledger_status parse_records(const char *text, struct record *out, size_t *count,
                                 int *error_line) {
    record_count = 0;
    int line_number = 0;
    for (const char *line = text; *line != '\0';) {
        const char *end = line + strcspn(line, "\n");
        const char *first = line;
        line_number++;
        while (first < end && isspace((unsigned char)*first))
            first++;
        if (first < end && *first != '#') {
            const char *fields[4] = {line};
            size_t field_count = 1;
            for (const char *p = line; p < end; p++) {
                if (*p != ',')
                    continue;
                if (field_count < 4)
                    fields[field_count] = p + 1;
                field_count++;
            }
            struct record *next = &records[record_count];
            if (field_count != 4 || record_count == LEDGER_MAX_RECORDS
                || !copy_trimmed(next->account, fields[1], fields[2] - 1)
                || !copy_trimmed(next->kind, fields[2], fields[3] - 1)
                || !parse_amount(&next->amount_cents, fields[3], end)) {
                if (error_count < LEDGER_MAX_RECORDS)
                    errors[error_count++] = line_number;
                *error_line = line_number;
                return LEDGER_E_FIELDS;
            }
            record_count++;
        }
        line = *end == '\n' ? end + 1 : end;
    }
    memcpy(out, records, record_count * sizeof *records);
    *count = record_count;
    return LEDGER_OK;
}

/** Fee for one transaction. The percentage part rounds half up to whole cents. */
enum ledger_status fee_cents(const char *kind, long long amount_cents, long long *fee) {
    if (strcmp(kind, "card") == 0) {
        long long scaled = amount_cents * 29 + 500;
        /* Floor division: C's / truncates toward zero, which is wrong for refunds. */
        *fee = scaled / 1000 - (scaled % 1000 < 0) + 30;
        return LEDGER_OK;
    }
    if (strcmp(kind, "bank") == 0) {
        *fee = 25;
        return LEDGER_OK;
    }
    if (strcmp(kind, "cash") == 0) {
        *fee = 0;
        return LEDGER_OK;
    }
    return LEDGER_E_KIND;
}

/** Recompute module totals from the loaded records, or from current when it is not NULL. */
enum ledger_status balances(const struct record *current, size_t count, struct balance *out,
                            size_t *out_count) {
    if (current == NULL) {
        current = records;
        count = record_count;
    }
    if (count > LEDGER_MAX_RECORDS)
        count = LEDGER_MAX_RECORDS;
    total_count = 0;
    for (size_t i = 0; i < count; i++) {
        long long fee;
        enum ledger_status status = fee_cents(current[i].kind, current[i].amount_cents, &fee);
        if (status != LEDGER_OK)
            return status;
        size_t slot = 0;
        while (slot < total_count && strcmp(totals[slot].account, current[i].account) != 0)
            slot++;
        if (slot == total_count) {
            snprintf(totals[slot].account, sizeof totals[slot].account, "%s", current[i].account);
            totals[total_count++].cents = 0;
        }
        totals[slot].cents += current[i].amount_cents - fee;
    }
    memcpy(out, totals, total_count * sizeof *totals);
    *out_count = total_count;
    return LEDGER_OK;
}

/** Dollars with two decimals; negatives in parentheses, for example (1.25). */
static void format_cents(long long cents, char *out, size_t size) {
    long long magnitude = llabs(cents);
    snprintf(out, size, cents < 0 ? "(%lld.%02lld)" : "%lld.%02lld", magnitude / 100,
             magnitude % 100);
}

/** qsort comparator: order balances by account name. */
static int compare_accounts(const void *a, const void *b) {
    const struct balance *left = a;
    const struct balance *right = b;
    return strcmp(left->account, right->account);
}

/** Render the module totals unless totals are given (count is not 0). */
void render(const struct balance *current, size_t count, char *buf, size_t buf_size) {
    struct balance sorted[LEDGER_MAX_RECORDS];
    size_t used = 0;
    if (count == 0) {
        current = totals;
        count = total_count;
    }
    if (count > LEDGER_MAX_RECORDS)
        count = LEDGER_MAX_RECORDS;
    if (buf_size > 0)
        buf[0] = '\0';
    memcpy(sorted, current, count * sizeof *current);
    qsort(sorted, count, sizeof *sorted, compare_accounts);
    for (size_t i = 0; i < count && used < buf_size; i++) {
        char amount[32];
        format_cents(sorted[i].cents, amount, sizeof amount);
        int written = snprintf(buf + used, buf_size - used, "%s%s  %s", i > 0 ? "\n" : "",
                               sorted[i].account, amount);
        if (written < 0)
            break;
        used += (size_t)written;
    }
}
