/* TariffCore legacy duty calculator. BUILD ARTIFACT ONLY - hidden ground
 * truth; must never ship in the agent-visible repo.
 *
 * Designed under the wave-2 anatomy law: difficulty lives in ARBITRARY
 * CONJUNCTIONS of unrelated dimensions, at thresholds no spec or domain
 * folklore hints at. Every conjunction is inferable by systematic 2-D
 * sweeps; none is announced by structure.
 *
 * Documented model (docs/SPEC.md): duty = value * rate(hs_class) with
 * rate by 2-digit HS prefix band, plus weight fee = ceil(weight_kg) * 40
 * cents, rounded per spec, capped at 900000.
 *
 * Engine quirks:
 *  D1 (conjunction) mode 'A' (air) AND 50 < weight_kg <= 100: the weight
 *     fee uses 55 cents/kg instead of 40 (dense-cargo air band).
 *  D2 (conjunction) origin in {KP, IR, SY, CU} AND hs prefix 28..38
 *     (chemicals span): +130bp of value, added after rounding, half-up.
 *  D3 (conjunction) declared value > 2,000,000 AND day-of-month >= 25:
 *     duty rounding switches to truncation (month-end audit mode).
 *  D4 (conjunction) currency JPY AND mode 'S' (sea): weight fee waived
 *     entirely (a 1987 treaty remnant).
 *  D5 duty rounding is half-even (spec says "nearest"); the D2 levy and
 *     weight fee always round half-up.
 *  D6 the 900000 cap applies to duty+levy but NOT the weight fee (spec:
 *     total).
 *  D7 HS codes starting "99" are documented as exempt; the engine also
 *     exempts "98" (legacy grandfather clause) - single-dimension but
 *     undocumented.
 *  D8 weight is parsed as integer decigrams (input format), and
 *     ceil(kg) is computed as (dg + 9999) / 10000 in integer math; the
 *     spec's example table implies rounding to nearest kg.
 *
 * Input (one declaration per line, space separated):
 *   D <declid:1-10 alnum> <origin:2 alpha> <hs:6 digits> <mode:A|S|R>
 *     <currency:USD|EUR|JPY> <value_cents:1-9 digits> <weight_dg:1-8 digits>
 * Output:
 *   T <declid> <duty> <weightfee> <levy> <total>
 *   R <declid> <code>   codes: FMT ORIGIN HS MODE CUR VALUE WEIGHT
 * Trailer on EOF: X <count_ok> <count_rej> <sum_total>
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const struct { int lo, hi, bp; } RATE_BANDS[] = {
    {1, 15, 250}, {16, 27, 400}, {28, 38, 650}, {39, 49, 500},
    {50, 63, 800}, {64, 83, 300}, {84, 90, 150}, {91, 97, 550},
    {98, 99, 0},
};

#define CAP 900000LL

typedef enum { R_HALF_EVEN, R_HALF_UP, R_TRUNC } rmode_t;

static long long div_round(long long n, long long d, rmode_t mode) {
    long long q = n / d, r = n % d;
    if (r == 0) return q;
    switch (mode) {
        case R_TRUNC: return q;
        case R_HALF_UP: return (2 * r >= d) ? q + 1 : q;
        case R_HALF_EVEN:
            if (2 * r > d) return q + 1;
            if (2 * r < d) return q;
            return (q % 2 == 0) ? q : q + 1;
    }
    return q;
}

static int tok_len_class(const char *s, int lo, int hi, int (*cls)(int)) {
    int n = (int)strlen(s);
    if (n < lo || n > hi) return 0;
    for (int i = 0; i < n; i++)
        if (!cls((unsigned char)s[i])) return 0;
    return 1;
}

static long long count_ok = 0, count_rej = 0, sum_total = 0;

static void reject(const char *id, const char *code) {
    printf("R %s %s\n", id, code);
    count_rej++;
}

static void process(char *line) {
    char kind[8] = "", id[64] = "", origin[64] = "", hs[64] = "", mode[64] = "",
         cur[64] = "", valtok[64] = "", wtok[64] = "";
    int n = sscanf(line, "%7s %63s %63s %63s %63s %63s %63s %63s",
                   kind, id, origin, hs, mode, cur, valtok, wtok);
    if (strcmp(kind, "D") != 0 || n < 8 || !tok_len_class(id, 1, 10, isalnum)) {
        printf("R %s FMT\n", tok_len_class(id, 1, 10, isalnum) ? id : "??????????");
        count_rej++;
        return;
    }
    if (!tok_len_class(origin, 2, 2, isupper)) { reject(id, "ORIGIN"); return; }
    if (!tok_len_class(hs, 6, 6, isdigit)) { reject(id, "HS"); return; }
    if (strlen(mode) != 1 || !strchr("ASR", mode[0])) { reject(id, "MODE"); return; }
    if (strcmp(cur, "USD") && strcmp(cur, "EUR") && strcmp(cur, "JPY")) {
        reject(id, "CUR"); return;
    }
    if (!tok_len_class(valtok, 1, 9, isdigit)) { reject(id, "VALUE"); return; }
    if (!tok_len_class(wtok, 1, 8, isdigit)) { reject(id, "WEIGHT"); return; }
    long long value = atoll(valtok), weight_dg = atoll(wtok);
    int prefix = (hs[0] - '0') * 10 + (hs[1] - '0');
    if (prefix < 1) { reject(id, "HS"); return; }

    /* D7: 98 and 99 both exempt */
    long long duty = 0, levy = 0, wfee = 0;
    if (prefix < 98) {
        int bp = 0;
        for (size_t i = 0; i < sizeof RATE_BANDS / sizeof *RATE_BANDS; i++)
            if (prefix >= RATE_BANDS[i].lo && prefix <= RATE_BANDS[i].hi) {
                bp = RATE_BANDS[i].bp;
                break;
            }
        /* D3: month-end audit mode for big declarations. The date lives in
         * the declaration id's last two characters when they are digits
         * (legacy convention: ids end with the day of month). */
        int day = 0;
        size_t idlen = strlen(id);
        if (idlen >= 2 && isdigit((unsigned char)id[idlen - 2]) &&
            isdigit((unsigned char)id[idlen - 1]))
            day = (id[idlen - 2] - '0') * 10 + (id[idlen - 1] - '0');
        rmode_t duty_mode = R_HALF_EVEN; /* D5 */
        if (value > 2000000LL && day >= 25) duty_mode = R_TRUNC; /* D3 */
        duty = div_round(value * bp, 10000, duty_mode);
        /* D2: origin x chemicals levy */
        if ((strcmp(origin, "KP") == 0 || strcmp(origin, "IR") == 0 ||
             strcmp(origin, "SY") == 0 || strcmp(origin, "CU") == 0) &&
            prefix >= 28 && prefix <= 38)
            levy = div_round(value * 130, 10000, R_HALF_UP);
        long long capped = duty + levy;
        if (capped > CAP) { /* D6: cap excludes the weight fee */
            long long over = capped - CAP;
            if (levy >= over) levy -= over;
            else { duty -= (over - levy); levy = 0; }
        }
    }

    /* weight fee: D8 ceil via integer math; D1 air dense band; D4 waiver */
    long long kg = (weight_dg + 9999) / 10000;
    long long rate_c = 40;
    if (mode[0] == 'A' && kg > 50 && kg <= 100) rate_c = 55; /* D1 */
    wfee = kg * rate_c;
    if (strcmp(cur, "JPY") == 0 && mode[0] == 'S') wfee = 0; /* D4 */

    long long total = duty + levy + wfee;
    printf("T %s %lld %lld %lld %lld\n", id, duty, wfee, levy, total);
    count_ok++;
    sum_total += total;
}

int main(void) {
    char line[512];
    while (fgets(line, sizeof line, stdin)) {
        line[strcspn(line, "\r\n")] = 0;
        if (!line[0]) continue;
        process(line);
    }
    printf("X %lld %lld %lld\n", count_ok, count_rej, sum_total);
    return 0;
}
