/* MeterCore legacy engine. BUILD ARTIFACT ONLY - this source is the task's
 * hidden ground truth and must never ship in the agent-visible repo.
 *
 * Behavior is deliberately quirky; the quirks ARE the contract:
 *  Q1  Winter night relief: band N in months {12, 01, 02} uses multiplier
 *      0.60 (not the documented 0.70), but ONLY for kwh landing in tier 1.
 *      Kwh from the same reading that lands in tier 2/3 still uses 0.70.
 *  Q2  Meter L (legacy analog) carries a 3% line-loss uplift: the reading's
 *      kwh is billed at 103% inside each tier slice, but the uplift does
 *      NOT advance the per-account tier counter and does NOT appear in the
 *      reported total_kwh. Meter L readings therefore bill ~3% more energy
 *      than they advance the tiers. The spec documents no uplift at all.
 *  Q3  The summer (months 06-09) +10% energy surcharge applies to tier 2
 *      AND tier 3 slices; the spec says tier 3 only.
 *  Q4  Rounding: the engine computes each tier slice exactly (integer
 *      arithmetic; per-cent denominator 10^7 covering rate tenths, uplift
 *      hundredths, season hundredths, band hundredths), sums the slices,
 *      and rounds ONCE per reading, half-even, to whole cents. The spec
 *      says "rounded to the nearest cent per reading"; the naive rewrite
 *      rounds each slice half-up and then sums.
 *  Q5  Accounts fold case-insensitively for accumulation; the first-seen
 *      spelling is what the B line reports. The spec says case-sensitive.
 *  Q6  Month 00 is rejected (MONTH), but month 13 is ACCEPTED and treated
 *      as month 01 (year-wrap convention from the mainframe feed). Month 13
 *      is therefore winter for Q1 purposes. The spec rejects 13.
 *  Q7  kwh 0 readings are skipped entirely: no accumulation, no first-seen
 *      registration, no effect on total_kwh. An account whose only
 *      readings are 0 kwh never appears in the output. The spec calls them
 *      valid no-ops (the naive rewrite emits such accounts with a 0 bill).
 *  Q8  Lines with MORE than 6 whitespace-separated tokens are accepted
 *      (trailing tokens ignored). Fewer than 6 is FMT. The spec demands
 *      exactly 6.
 *
 *  Reject lines (R <acct> <code>) are emitted immediately in input order;
 *  the echoed acct is the line's second token, or "?" when absent. Codes:
 *  FMT, MONTH, KWH, BAND, METER. At EOF: one B line per account in
 *  first-seen order, then the X trailer.
 */

#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAXACCTS 8192
#define T1_LIMIT 500LL
#define T2_LIMIT 2000LL

/* rate in tenths of a cent per kwh: tier1 14c, tier2 19c, tier3 26c */
static const long long RATE_TENTHS[3] = {140, 190, 260};

typedef struct {
    char key[9];   /* uppercased */
    char shown[9]; /* first-seen spelling */
    long long counter;    /* raw kwh, advances tiers (Q2: no uplift here) */
    long long total_kwh;  /* reported raw kwh */
    long long bill;       /* cents */
} acct_t;

static acct_t accts[MAXACCTS];
static size_t naccts = 0;

static acct_t *find_or_add(const char *acct) {
    char key[9];
    size_t n = strlen(acct);
    for (size_t i = 0; i < n; i++) key[i] = (char)toupper((unsigned char)acct[i]);
    key[n] = 0; /* Q5 */
    for (size_t i = 0; i < naccts; i++)
        if (strcmp(accts[i].key, key) == 0) return &accts[i];
    if (naccts == MAXACCTS) { fprintf(stderr, "too many accounts\n"); exit(2); }
    acct_t *a = &accts[naccts++];
    strcpy(a->key, key);
    strcpy(a->shown, acct);
    a->counter = a->total_kwh = a->bill = 0;
    return a;
}

static long long half_even(long long n, long long d) { /* n >= 0, d > 0 */
    long long q = n / d, r = n % d;
    if (2 * r > d) return q + 1;
    if (2 * r < d) return q;
    return (q % 2 == 0) ? q : q + 1; /* Q4 */
}

static int all_digits(const char *s) {
    if (!*s) return 0;
    for (; *s; s++)
        if (!isdigit((unsigned char)*s)) return 0;
    return 1;
}

int main(void) {
    char buf[1024];
    long long rejected = 0;

    while (fgets(buf, sizeof buf, stdin)) {
        char line[1024];
        strcpy(line, buf);
        char *tok[16];
        int ntok = 0;
        for (char *p = strtok(line, " \t\r\n"); p && ntok < 16; p = strtok(NULL, " \t\r\n"))
            tok[ntok++] = p;
        if (ntok == 0) continue; /* blank line */

        const char *echo = ntok >= 2 ? tok[1] : "?";
        const char *err = NULL;

        char band = 0, meter = 0;
        long long month = 0, kwh = 0;

        if (ntok < 6 || strcmp(tok[0], "M") != 0) err = "FMT"; /* Q8: >6 ok */
        if (!err) {
            size_t alen = strlen(tok[1]);
            if (alen < 1 || alen > 8) err = "FMT";
            else
                for (size_t i = 0; i < alen; i++)
                    if (!isalnum((unsigned char)tok[1][i])) { err = "FMT"; break; }
        }
        if (!err) {
            if (strlen(tok[2]) != 2 || !all_digits(tok[2])) err = "MONTH";
            else {
                month = atoll(tok[2]);
                if (month == 13) month = 1; /* Q6 */
                else if (month < 1 || month > 12) err = "MONTH";
            }
        }
        if (!err) {
            size_t klen = strlen(tok[3]);
            if (klen < 1 || klen > 6 || !all_digits(tok[3])) err = "KWH";
            else kwh = atoll(tok[3]);
        }
        if (!err) {
            if (strlen(tok[4]) != 1 || !strchr("DNW", tok[4][0])) err = "BAND";
            else band = tok[4][0];
        }
        if (!err) {
            if (strlen(tok[5]) != 1 || !strchr("SL", tok[5][0])) err = "METER";
            else meter = tok[5][0];
        }
        if (err) {
            printf("R %s %s\n", echo, err);
            rejected++;
            continue;
        }
        if (kwh == 0) continue; /* Q7: skipped entirely */

        acct_t *a = find_or_add(tok[1]);
        long long prev = a->counter;

        long long s1 = T1_LIMIT - prev;
        if (s1 < 0) s1 = 0;
        if (s1 > kwh) s1 = kwh;
        long long over1 = prev > T1_LIMIT ? prev : T1_LIMIT;
        long long s2 = T2_LIMIT - over1;
        if (s2 < 0) s2 = 0;
        if (s2 > kwh - s1) s2 = kwh - s1;
        long long s3 = kwh - s1 - s2;
        long long slices[3] = {s1, s2, s3};

        long long uplift = meter == 'L' ? 103 : 100; /* Q2 */
        int summer = month >= 6 && month <= 9;
        int winter = month == 12 || month == 1 || month == 2;
        long long band_base = band == 'D' ? 100 : band == 'N' ? 70 : 85;

        long long units = 0; /* per-cent denominator 10*100*100*100 = 10^7 */
        for (int t = 0; t < 3; t++) {
            if (slices[t] == 0) continue;
            long long season = (summer && t >= 1) ? 110 : 100; /* Q3 */
            long long bandm = band_base;
            if (band == 'N' && winter && t == 0) bandm = 60; /* Q1 */
            units += slices[t] * RATE_TENTHS[t] * uplift * season * bandm;
        }
        long long fee = half_even(units, 10000000LL); /* Q4 */

        a->counter += kwh; /* Q2: raw kwh, no uplift */
        a->total_kwh += kwh;
        a->bill += fee;
    }

    long long grand = 0;
    for (size_t i = 0; i < naccts; i++) {
        printf("B %s %lld %lld\n", accts[i].shown, accts[i].total_kwh, accts[i].bill);
        grand += accts[i].bill;
    }
    printf("X %zu %lld %lld\n", naccts, rejected, grand);
    return 0;
}
