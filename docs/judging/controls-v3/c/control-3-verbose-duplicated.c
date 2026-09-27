/* This file is the ledger file. It handles the ledger. */

#include <ctype.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define LEDGER_MAX_RECORDS 256
#define LEDGER_NAME_MAX 32

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

static bool copy_trimmed(char dst[LEDGER_NAME_MAX], const char *start, const char *end) {
    /* Skip the spaces at the start. */
    while (start < end && isspace((unsigned char)*start))
        start++;
    /* Skip the spaces at the end. */
    while (end > start && isspace((unsigned char)end[-1]))
        end--;
    size_t length = (size_t)(end - start);
    /* Check that it fits. */
    if (length >= LEDGER_NAME_MAX)
        return false;
    memcpy(dst, start, length);
    dst[length] = '\0';
    return true;
}

static bool parse_amount(long long *amount, const char *start, const char *end) {
    char digits[LEDGER_NAME_MAX];
    char *rest;
    if (!copy_trimmed(digits, start, end))
        return false;
    *amount = strtoll(digits, &rest, 10);
    return rest != digits && *rest == '\0';
}

enum ledger_status parse_records(const char *text, struct record *out, size_t *count,
                                 int *error_line) {
    /* First we set the number of records to zero because the list is empty. */
    size_t record_count = 0;
    /* Now we go through every line. */
    int line_number = 0;
    const char *line = text;
    while (*line != '\0') {
        line_number = line_number + 1;
        const char *end = line + strcspn(line, "\n");
        const char *first = line;
        while (first < end && isspace((unsigned char)*first))
            first = first + 1;
        /* Skip the line if it is blank. */
        if (first == end) {
            line = *end == '\n' ? end + 1 : end;
            continue;
        }
        /* Skip the line if it is a comment. */
        if (*first == '#') {
            line = *end == '\n' ? end + 1 : end;
            continue;
        }
        const char *fields[4] = {line};
        size_t field_count = 1;
        for (const char *p = line; p < end; p++) {
            if (*p == ',') {
                if (field_count < 4)
                    fields[field_count] = p + 1;
                field_count = field_count + 1;
            }
        }
        /* Check the number of fields. */
        if (field_count != 4) {
            *error_line = line_number;
            return LEDGER_E_FIELDS;
        }
        /* Check that the record fits. */
        struct record *record = &out[record_count];
        if (record_count == LEDGER_MAX_RECORDS
            || !copy_trimmed(record->account, fields[1], fields[2] - 1)
            || !copy_trimmed(record->kind, fields[2], fields[3] - 1)
            || !parse_amount(&record->amount_cents, fields[3], end)) {
            *error_line = line_number;
            return LEDGER_E_FIELDS;
        }
        /* Validate the kind here so bad kinds are caught early. */
        if (strcmp(record->kind, "card") == 0) {
            /* card is fine */
        } else if (strcmp(record->kind, "bank") == 0) {
            /* bank is fine */
        } else if (strcmp(record->kind, "cash") == 0) {
            /* cash is fine */
        } else {
            return LEDGER_E_KIND;
        }
        record_count = record_count + 1;
        line = *end == '\n' ? end + 1 : end;
    }
    *count = record_count;
    return LEDGER_OK;
}

enum ledger_status fee_cents(const char *kind, long long amount_cents, long long *fee) {
    /* Work out the fee for a card. */
    if (strcmp(kind, "card") == 0) {
        long long scaled = amount_cents * 29 + 500;
        long long percentage = scaled / 1000;
        /* Go down one more when the remainder is negative. */
        if (scaled % 1000 < 0)
            percentage = percentage - 1;
        *fee = percentage + 30;
        return LEDGER_OK;
    }
    /* Work out the fee for a bank transfer. */
    if (strcmp(kind, "bank") == 0) {
        *fee = 25;
        return LEDGER_OK;
    }
    /* Work out the fee for cash. */
    if (strcmp(kind, "cash") == 0) {
        *fee = 0;
        return LEDGER_OK;
    }
    return LEDGER_E_KIND;
}

enum ledger_status balances(const struct record *records, size_t count, struct balance *out,
                            size_t *out_count) {
    /* Start with no totals. */
    size_t total_count = 0;
    for (size_t i = 0; i < count; i++) {
        const char *account = records[i].account;
        const char *kind = records[i].kind;
        long long amount = records[i].amount_cents;
        long long fee;
        long long net;
        /* Work out the fee for a card. */
        if (strcmp(kind, "card") == 0) {
            long long scaled = amount * 29 + 500;
            fee = scaled / 1000;
            if (scaled % 1000 < 0)
                fee = fee - 1;
            fee = fee + 30;
            net = amount - fee;
        }
        /* Work out the fee for a bank transfer. */
        else if (strcmp(kind, "bank") == 0) {
            fee = 25;
            net = amount - fee;
        }
        /* Work out the fee for cash. */
        else if (strcmp(kind, "cash") == 0) {
            fee = 0;
            net = amount - fee;
        } else {
            return LEDGER_E_KIND;
        }
        /* Add the net amount to the account. */
        size_t slot = 0;
        bool found = false;
        for (size_t j = 0; j < total_count; j++) {
            if (strcmp(out[j].account, account) == 0) {
                slot = j;
                found = true;
            }
        }
        if (found) {
            out[slot].cents = out[slot].cents + net;
        } else {
            snprintf(out[total_count].account, sizeof out[total_count].account, "%s", account);
            out[total_count].cents = net;
            total_count = total_count + 1;
        }
    }
    *out_count = total_count;
    return LEDGER_OK;
}

enum ledger_status total_fees(const struct record *records, size_t count, long long *fees) {
    /* Sum all the fees. Uses the same rules as balances. */
    long long sum = 0;
    for (size_t i = 0; i < count; i++) {
        const char *kind = records[i].kind;
        long long amount = records[i].amount_cents;
        if (strcmp(kind, "card") == 0) {
            long long scaled = amount * 29 + 500;
            long long percentage = scaled / 1000;
            if (scaled % 1000 < 0)
                percentage = percentage - 1;
            sum = sum + percentage + 30;
        } else if (strcmp(kind, "bank") == 0) {
            sum = sum + 25;
        } else if (strcmp(kind, "cash") == 0) {
            sum = sum + 0;
        } else {
            return LEDGER_E_KIND;
        }
    }
    *fees = sum;
    return LEDGER_OK;
}

static int compare_accounts(const void *a, const void *b) {
    const struct balance *left = a;
    const struct balance *right = b;
    return strcmp(left->account, right->account);
}

void render(const struct balance *totals, size_t count, char *buf, size_t buf_size) {
    /* Build the output lines. */
    struct balance accounts[LEDGER_MAX_RECORDS];
    if (count > LEDGER_MAX_RECORDS)
        count = LEDGER_MAX_RECORDS;
    memcpy(accounts, totals, count * sizeof *totals);
    qsort(accounts, count, sizeof *accounts, compare_accounts);
    size_t used = 0;
    if (buf_size > 0)
        buf[0] = '\0';
    for (size_t i = 0; i < count && used < buf_size; i++) {
        const char *separator = i > 0 ? "\n" : "";
        long long cents = accounts[i].cents;
        int written;
        /* Format negative numbers with parentheses. */
        if (cents < 0) {
            cents = -cents;
            char dollars[32];
            snprintf(dollars, sizeof dollars, "%lld.%02lld", cents / 100, cents % 100);
            written = snprintf(buf + used, buf_size - used, "%s%s  (%s)", separator,
                               accounts[i].account, dollars);
        } else {
            char dollars[32];
            snprintf(dollars, sizeof dollars, "%lld.%02lld", cents / 100, cents % 100);
            written = snprintf(buf + used, buf_size - used, "%s%s  %s", separator,
                               accounts[i].account, dollars);
        }
        if (written < 0)
            break;
        used = used + (size_t)written;
    }
}
