/* QLite legacy record store. BUILD ARTIFACT ONLY - hidden ground truth.
 *
 * Engine quirks (spec says otherwise or says nothing):
 *  Q1 Scan order is PHYSICAL SLOT order: DEL pushes the slot onto a LIFO
 *     free list, and the next INS reuses the most recently freed slot, so
 *     scans after delete/insert churn do not follow insertion order.
 *     (Spec: rows are returned in insertion order.)
 *  Q2 FIND matching is case-insensitive EXCEPT the first character of the
 *     pattern and name, which are compared raw. (Spec: case-insensitive.)
 *  Q3 RANGE is lo-inclusive, hi-EXCLUSIVE, except lo==hi which matches
 *     exactly. (Spec: both bounds inclusive.)
 *  Q4 AVG divides with C truncation toward zero, so negative averages
 *     differ from floor division. (Spec: rounded down.)
 *  Q5 INS with an existing id updates the SCORE but not the name, and
 *     answers OK. (Spec: reject, ERR DUPKEY.)
 *  Q6 Names are silently truncated to 12 characters on insert.
 *     (Spec: up to 16 characters.)
 *  Q7 SUM accumulates in 32-bit two's complement and wraps.
 *     (Spec: exact sum.)
 *  Q8 In FIND patterns, '*' is a wildcard only as the final character;
 *     interior '*' match literally. (Spec: '*' matches any sequence
 *     anywhere.)
 *
 * Protocol (stdin, one command per line; outputs per command):
 *   INS <id> <name> <score>   -> OK | ERR FULL | ERR FMT
 *   DEL <id>                  -> OK | ERR NOTFOUND
 *   GET <id>                  -> ROW ... | ERR NOTFOUND
 *   FIND <pattern>            -> ROW ... lines, then END <count>
 *   RANGE <lo> <hi>           -> ROW ... lines, then END <count>
 *   LIST                      -> ROW ... lines, then END <count>
 *   SUM                       -> SUM <value>
 *   AVG                       -> AVG <value> | ERR EMPTY
 *   id: 1-8 alnum; name: 1-16 alnum; score: integer -999999..999999.
 */

#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SLOTS 4096
#define NAME_KEEP 12

typedef struct {
    char id[9];
    char name[17];
    long score;
    int used;
} slot_t;

static slot_t table[SLOTS];
static int high_water = 0;       /* slots 0..high_water-1 ever used */
static int freelist[SLOTS];      /* LIFO stack of freed slot indexes */
static int nfree = 0;

static slot_t *find_id(const char *id) {
    for (int i = 0; i < high_water; i++)
        if (table[i].used && strcmp(table[i].id, id) == 0) return &table[i];
    return NULL;
}

static int tok_alnum(const char *s, int lo, int hi) {
    int n = (int)strlen(s);
    if (n < lo || n > hi) return 0;
    for (int i = 0; i < n; i++)
        if (!isalnum((unsigned char)s[i])) return 0;
    return 1;
}

static int parse_score(const char *s, long *out) {
    char *end;
    long v = strtol(s, &end, 10);
    if (*end || end == s) return 0;
    if (v < -999999 || v > 999999) return 0;
    *out = v;
    return 1;
}

static void cmd_ins(const char *id, const char *name, const char *scoretok) {
    long score;
    if (!tok_alnum(id, 1, 8) || !tok_alnum(name, 1, 16) || !parse_score(scoretok, &score)) {
        puts("ERR FMT");
        return;
    }
    slot_t *existing = find_id(id);
    if (existing) { /* Q5: score updated, name kept */
        existing->score = score;
        puts("OK");
        return;
    }
    int idx;
    if (nfree > 0) idx = freelist[--nfree]; /* Q1: LIFO reuse */
    else if (high_water < SLOTS) idx = high_water++;
    else { puts("ERR FULL"); return; }
    slot_t *s = &table[idx];
    snprintf(s->id, sizeof s->id, "%s", id);
    snprintf(s->name, sizeof s->name, "%.*s", NAME_KEEP, name); /* Q6 */
    s->score = score;
    s->used = 1;
    puts("OK");
}

static void cmd_del(const char *id) {
    if (!tok_alnum(id, 1, 8)) { puts("ERR FMT"); return; }
    for (int i = 0; i < high_water; i++) {
        if (table[i].used && strcmp(table[i].id, id) == 0) {
            table[i].used = 0;
            freelist[nfree++] = i;
            puts("OK");
            return;
        }
    }
    puts("ERR NOTFOUND");
}

static void print_row(const slot_t *s) {
    printf("ROW %s %s %ld\n", s->id, s->name, s->score);
}

static void cmd_get(const char *id) {
    if (!tok_alnum(id, 1, 8)) { puts("ERR FMT"); return; }
    slot_t *s = find_id(id);
    if (s) print_row(s);
    else puts("ERR NOTFOUND");
}

static int match_name(const char *pattern, const char *name) {
    size_t plen = strlen(pattern);
    int star = plen > 0 && pattern[plen - 1] == '*'; /* Q8 */
    size_t cmplen = star ? plen - 1 : plen;
    if (star ? strlen(name) < cmplen : strlen(name) != cmplen) return 0;
    for (size_t i = 0; i < cmplen; i++) {
        char p = pattern[i], c = name[i];
        if (i == 0) { /* Q2: first char raw */
            if (p != c) return 0;
        } else if (tolower((unsigned char)p) != tolower((unsigned char)c)) {
            return 0;
        }
    }
    return 1;
}

static void cmd_find(const char *pattern) {
    if (strlen(pattern) == 0 || strlen(pattern) > 17) { puts("ERR FMT"); return; }
    int count = 0;
    for (int i = 0; i < high_water; i++) { /* Q1: physical order */
        if (table[i].used && match_name(pattern, table[i].name)) {
            print_row(&table[i]);
            count++;
        }
    }
    printf("END %d\n", count);
}

static void cmd_range(const char *lotok, const char *hitok) {
    long lo, hi;
    if (!parse_score(lotok, &lo) || !parse_score(hitok, &hi)) { puts("ERR FMT"); return; }
    int count = 0;
    for (int i = 0; i < high_water; i++) {
        if (!table[i].used) continue;
        long v = table[i].score;
        int hit = (lo == hi) ? v == lo : (v >= lo && v < hi); /* Q3 */
        if (hit) { print_row(&table[i]); count++; }
    }
    printf("END %d\n", count);
}

static void cmd_list(void) {
    int count = 0;
    for (int i = 0; i < high_water; i++)
        if (table[i].used) { print_row(&table[i]); count++; }
    printf("END %d\n", count);
}

static void cmd_sum(void) {
    int32_t sum = 0; /* Q7 */
    for (int i = 0; i < high_water; i++)
        if (table[i].used) sum = (int32_t)((uint32_t)sum + (uint32_t)(int32_t)table[i].score);
    printf("SUM %d\n", (int)sum);
}

static void cmd_avg(void) {
    long long sum = 0;
    long n = 0;
    for (int i = 0; i < high_water; i++)
        if (table[i].used) { sum += table[i].score; n++; }
    if (n == 0) { puts("ERR EMPTY"); return; }
    printf("AVG %lld\n", sum / n); /* Q4: C truncation */
}

int main(void) {
    char line[256];
    while (fgets(line, sizeof line, stdin)) {
        line[strcspn(line, "\r\n")] = 0;
        if (!line[0]) continue;
        char a[64] = "", b[64] = "", c[64] = "", d[64] = "";
        int n = sscanf(line, "%63s %63s %63s %63s", a, b, c, d);
        if (n >= 1 && strcmp(a, "INS") == 0 && n >= 4) cmd_ins(b, c, d);
        else if (strcmp(a, "DEL") == 0 && n >= 2) cmd_del(b);
        else if (strcmp(a, "GET") == 0 && n >= 2) cmd_get(b);
        else if (strcmp(a, "FIND") == 0 && n >= 2) cmd_find(b);
        else if (strcmp(a, "RANGE") == 0 && n >= 3) cmd_range(b, c);
        else if (strcmp(a, "LIST") == 0) cmd_list();
        else if (strcmp(a, "SUM") == 0) cmd_sum();
        else if (strcmp(a, "AVG") == 0) cmd_avg();
        else puts("ERR FMT");
    }
    return 0;
}
