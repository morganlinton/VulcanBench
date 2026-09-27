#include <ctype.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define LEDGER_MAX_RECORDS 256
#define LEDGER_NAME_MAX 32
#define FEE_POLICY_REGISTRY_CAPACITY 8

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

struct money {
    long long cents;
};

static struct money money_of(long long cents) {
    struct money money = {cents};
    return money;
}

static struct money money_minus(struct money self, struct money other) {
    return money_of(self.cents - other.cents);
}

static struct money money_plus(struct money self, struct money other) {
    return money_of(self.cents + other.cents);
}

struct fee_policy {
    struct money (*fee)(const struct fee_policy *self, struct money amount);
};

static struct money card_fee_policy_fee(const struct fee_policy *self, struct money amount) {
    (void)self;
    long long scaled = amount.cents * 29 + 500;
    return money_of(scaled / 1000 - (scaled % 1000 < 0) + 30);
}

static struct money bank_fee_policy_fee(const struct fee_policy *self, struct money amount) {
    (void)self;
    (void)amount;
    return money_of(25);
}

static struct money cash_fee_policy_fee(const struct fee_policy *self, struct money amount) {
    (void)self;
    (void)amount;
    return money_of(0);
}

static const struct fee_policy CARD_FEE_POLICY = {card_fee_policy_fee};
static const struct fee_policy BANK_FEE_POLICY = {bank_fee_policy_fee};
static const struct fee_policy CASH_FEE_POLICY = {cash_fee_policy_fee};

struct fee_policy_registry_entry {
    const char *kind;
    const struct fee_policy *policy;
};

struct fee_policy_registry {
    struct fee_policy_registry_entry entries[FEE_POLICY_REGISTRY_CAPACITY];
    size_t count;
};

static struct fee_policy_registry *fee_policy_registry_register(struct fee_policy_registry *self,
                                                                const char *kind,
                                                                const struct fee_policy *policy) {
    if (self->count < FEE_POLICY_REGISTRY_CAPACITY) {
        self->entries[self->count].kind = kind;
        self->entries[self->count].policy = policy;
        self->count++;
    }
    return self;
}

static const struct fee_policy *fee_policy_registry_resolve(const struct fee_policy_registry *self,
                                                            const char *kind) {
    for (size_t i = 0; i < self->count; i++) {
        if (strcmp(self->entries[i].kind, kind) == 0)
            return self->entries[i].policy;
    }
    return NULL;
}

static const struct fee_policy_registry *registry(void) {
    static struct fee_policy_registry instance;
    if (instance.count == 0) {
        fee_policy_registry_register(
            fee_policy_registry_register(
                fee_policy_registry_register(&instance, "card", &CARD_FEE_POLICY), "bank",
                &BANK_FEE_POLICY),
            "cash", &CASH_FEE_POLICY);
    }
    return &instance;
}

struct ledger_entry {
    char account[LEDGER_NAME_MAX];
    char kind[LEDGER_NAME_MAX];
    struct money amount;
};

struct record_parser {
    const char *text;
};

static bool record_parser_copy_trimmed(char dst[LEDGER_NAME_MAX], const char *start,
                                       const char *end) {
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

static bool record_parser_keep(const char *line, const char *end) {
    while (line < end && isspace((unsigned char)*line))
        line++;
    return line < end && *line != '#';
}

static bool record_parser_parse_line(const char *line, const char *end,
                                     struct ledger_entry *entry) {
    const char *fields[4] = {line};
    size_t field_count = 1;
    for (const char *p = line; p < end; p++) {
        if (*p != ',')
            continue;
        if (field_count < 4)
            fields[field_count] = p + 1;
        field_count++;
    }
    char digits[LEDGER_NAME_MAX];
    char *rest;
    if (field_count != 4 || !record_parser_copy_trimmed(entry->account, fields[1], fields[2] - 1)
        || !record_parser_copy_trimmed(entry->kind, fields[2], fields[3] - 1)
        || !record_parser_copy_trimmed(digits, fields[3], end))
        return false;
    entry->amount = money_of(strtoll(digits, &rest, 10));
    return rest != digits && *rest == '\0';
}

static enum ledger_status record_parser_parse(const struct record_parser *self,
                                              struct ledger_entry *entries, size_t *count,
                                              int *error_line) {
    size_t n = 0;
    int number = 0;
    for (const char *line = self->text; *line != '\0';) {
        const char *end = line + strcspn(line, "\n");
        number++;
        if (record_parser_keep(line, end)) {
            if (n == LEDGER_MAX_RECORDS || !record_parser_parse_line(line, end, &entries[n])) {
                *error_line = number;
                return LEDGER_E_FIELDS;
            }
            n++;
        }
        line = *end == '\n' ? end + 1 : end;
    }
    *count = n;
    return LEDGER_OK;
}

struct balance_aggregator {
    const struct fee_policy_registry *registry;
    struct balance *totals;
    size_t count;
};

static enum ledger_status balance_aggregator_accept(struct balance_aggregator *self,
                                                    const struct ledger_entry *entry) {
    const struct fee_policy *policy = fee_policy_registry_resolve(self->registry, entry->kind);
    if (policy == NULL)
        return LEDGER_E_KIND;
    struct money fee = policy->fee(policy, entry->amount);
    size_t slot = 0;
    while (slot < self->count && strcmp(self->totals[slot].account, entry->account) != 0)
        slot++;
    if (slot == self->count) {
        snprintf(self->totals[slot].account, sizeof self->totals[slot].account, "%s",
                 entry->account);
        self->totals[self->count++].cents = 0;
    }
    struct money total = money_plus(money_of(self->totals[slot].cents),
                                    money_minus(entry->amount, fee));
    self->totals[slot].cents = total.cents;
    return LEDGER_OK;
}

static size_t balance_aggregator_result(const struct balance_aggregator *self) {
    return self->count;
}

struct renderer {
    char *buf;
    size_t size;
    size_t used;
};

static void renderer_money(long long cents, char *out, size_t size) {
    long long magnitude = llabs(cents);
    snprintf(out, size, cents < 0 ? "(%lld.%02lld)" : "%lld.%02lld", magnitude / 100,
             magnitude % 100);
}

static int renderer_compare(const void *a, const void *b) {
    const struct balance *left = a;
    const struct balance *right = b;
    return strcmp(left->account, right->account);
}

static void renderer_render(struct renderer *self, const struct balance *totals, size_t count) {
    struct balance sorted[LEDGER_MAX_RECORDS];
    if (count > LEDGER_MAX_RECORDS)
        count = LEDGER_MAX_RECORDS;
    if (self->size > 0)
        self->buf[0] = '\0';
    memcpy(sorted, totals, count * sizeof *totals);
    qsort(sorted, count, sizeof *sorted, renderer_compare);
    for (size_t i = 0; i < count && self->used < self->size; i++) {
        char money[32];
        renderer_money(sorted[i].cents, money, sizeof money);
        int written = snprintf(self->buf + self->used, self->size - self->used, "%s%s  %s",
                               i > 0 ? "\n" : "", sorted[i].account, money);
        if (written < 0)
            break;
        self->used += (size_t)written;
    }
}

enum ledger_status parse_records(const char *text, struct record *out, size_t *count,
                                 int *error_line) {
    struct ledger_entry entries[LEDGER_MAX_RECORDS];
    struct record_parser parser = {text};
    size_t n = 0;
    enum ledger_status status = record_parser_parse(&parser, entries, &n, error_line);
    if (status != LEDGER_OK)
        return status;
    for (size_t i = 0; i < n; i++) {
        snprintf(out[i].account, sizeof out[i].account, "%s", entries[i].account);
        snprintf(out[i].kind, sizeof out[i].kind, "%s", entries[i].kind);
        out[i].amount_cents = entries[i].amount.cents;
    }
    *count = n;
    return LEDGER_OK;
}

enum ledger_status fee_cents(const char *kind, long long amount_cents, long long *fee) {
    const struct fee_policy *policy = fee_policy_registry_resolve(registry(), kind);
    if (policy == NULL)
        return LEDGER_E_KIND;
    *fee = policy->fee(policy, money_of(amount_cents)).cents;
    return LEDGER_OK;
}

enum ledger_status balances(const struct record *records, size_t count, struct balance *out,
                            size_t *out_count) {
    struct balance_aggregator aggregator = {registry(), out, 0};
    for (size_t i = 0; i < count; i++) {
        struct ledger_entry entry;
        snprintf(entry.account, sizeof entry.account, "%s", records[i].account);
        snprintf(entry.kind, sizeof entry.kind, "%s", records[i].kind);
        entry.amount = money_of(records[i].amount_cents);
        enum ledger_status status = balance_aggregator_accept(&aggregator, &entry);
        if (status != LEDGER_OK)
            return status;
    }
    *out_count = balance_aggregator_result(&aggregator);
    return LEDGER_OK;
}

void render(const struct balance *totals, size_t count, char *buf, size_t buf_size) {
    struct renderer renderer = {buf, buf_size, 0};
    renderer_render(&renderer, totals, count);
}
