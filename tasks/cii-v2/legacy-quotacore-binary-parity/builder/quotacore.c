/* QuotaCore legacy API-metering biller. BUILD ARTIFACT ONLY - hidden
 * ground truth; must never ship in the agent-visible repo.
 *
 * Batch-stateful: per-key call consumption accumulates WITHIN the batch
 * (case-insensitively, first-seen spelling reported), so an event's
 * treatment depends on the events before it.
 *
 * Engine quirks (spec says otherwise or says nothing):
 *  Q1 (stateful) burst credits: the FIRST event of a key that stays
 *     entirely within quota banks 10 percent of its calls (integer
 *     floor) as bonus quota, silently extending quota_left.
 *  Q2 (stateful) an event that crosses out of a PARTIALLY CONSUMED
 *     quota (0 < used < limit and used + calls > limit) is billed
 *     entirely at the overage rate, its within-quota portion included.
 *     (Spec: only the overage portion is billed. The fresh-key path,
 *     used == 0, was special-cased by the original authors and bills
 *     only the overage portion, as the spec says.)
 *  Q3 (conjunction) tier P AND region in {BR, IN}: overage rate is 5
 *     cents per 100 calls, not 6 (the growth-market accord).
 *  Q4 rounding: overage units round up EXCEPT when the billed call
 *     count is an exact 50 modulo 100, which rounds DOWN:
 *     units = billed // 100 if billed % 100 == 50 else ceil(billed/100).
 *  Q5 tier upgrades F->S and S->P are accepted mid-batch (the key
 *     adopts the larger quota; consumed calls carry over). Downgrades
 *     and the F->P jump reject TIER. (Spec: any change rejects TIER.)
 *  Q6 region case is folded (lowercase accepted, uppercased inside).
 *  Q7 zero-call events are skipped silently right after field
 *     validation: no output line, not counted anywhere.
 *  Q8 key case folds for accumulation; first-seen spelling is echoed.
 *  Q9 trailing tokens are tolerated.
 *
 * Input:  Q <key:1-8 alnum> <calls:1-7 digits> <tier:F|S|P> <region:2 letters>
 * Output: B <key> <charge_cents> <quota_left>
 *         R <key> <code>          codes FMT CALLS TIER REGION
 * Trailer: X <accepted> <rejected> <sum_charges>
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAXKEY 4096

typedef struct {
    char key[9];  /* lowercased key */
    char seen[9]; /* first-seen spelling */
    char tier;
    long long used;
    long long bank;
} acct_t;

static acct_t accts[MAXKEY];
static int nacct = 0;
static long long count_ok = 0, count_rej = 0, sum_charges = 0;

static long long base_quota(char tier) {
    if (tier == 'F') return 10000LL;
    if (tier == 'S') return 100000LL;
    return 1000000LL;
}

static long long rate_for(char tier, const char *region) {
    if (tier == 'F') return 12;
    if (tier == 'S') return 9;
    if (strcmp(region, "BR") == 0 || strcmp(region, "IN") == 0)
        return 5; /* Q3 */
    return 6;
}

static acct_t *acct_find(const char *id, int *is_new) {
    char key[9];
    size_t n = strlen(id);
    for (size_t i = 0; i <= n; i++) key[i] = (char)tolower((unsigned char)id[i]);
    for (int i = 0; i < nacct; i++)
        if (strcmp(accts[i].key, key) == 0) { *is_new = 0; return &accts[i]; }
    if (nacct >= MAXKEY) return NULL;
    acct_t *a = &accts[nacct++];
    snprintf(a->key, sizeof a->key, "%s", key);
    snprintf(a->seen, sizeof a->seen, "%s", id);
    a->tier = 0;
    a->used = 0;
    a->bank = 0;
    *is_new = 1;
    return a;
}

static int tok_class(const char *s, int lo, int hi, int (*cls)(int)) {
    int n = (int)strlen(s);
    if (n < lo || n > hi) return 0;
    for (int i = 0; i < n; i++)
        if (!cls((unsigned char)s[i])) return 0;
    return 1;
}

static void process(char *line) {
    char kind[8] = "", id[64] = "", ctok[64] = "", ttok[64] = "", rtok[64] = "";
    int n = sscanf(line, "%7s %63s %63s %63s %63s", kind, id, ctok, ttok, rtok);
    int id_ok = tok_class(id, 1, 8, isalnum);
    if (strcmp(kind, "Q") != 0 || n < 5 || !id_ok) { /* Q9: extras tolerated */
        printf("R %s FMT\n", id_ok ? id : "????????");
        count_rej++;
        return;
    }
    if (!tok_class(ctok, 1, 7, isdigit)) { printf("R %s CALLS\n", id); count_rej++; return; }
    long long calls = atoll(ctok);
    if (strlen(ttok) != 1 || !strchr("FSP", ttok[0])) {
        printf("R %s TIER\n", id); count_rej++; return;
    }
    if (!tok_class(rtok, 2, 2, isalpha)) { printf("R %s REGION\n", id); count_rej++; return; }
    char region[3];
    region[0] = (char)toupper((unsigned char)rtok[0]); /* Q6 */
    region[1] = (char)toupper((unsigned char)rtok[1]);
    region[2] = 0;

    if (calls == 0) return; /* Q7: silent skip, no state effect */

    int is_new = 0;
    acct_t *acct = acct_find(id, &is_new); /* Q8 */
    if (!acct) { printf("R %s FMT\n", id); count_rej++; return; }
    char tier = ttok[0];
    int first_event = is_new;
    if (is_new) {
        acct->tier = tier;
    } else if (tier != acct->tier) {
        if ((acct->tier == 'F' && tier == 'S') ||
            (acct->tier == 'S' && tier == 'P')) {
            acct->tier = tier; /* Q5: upgrade adopts the larger quota */
        } else {
            printf("R %s TIER\n", acct->seen);
            count_rej++;
            return;
        }
    }

    long long limit = base_quota(acct->tier) + acct->bank;
    long long used = acct->used;
    long long within = used >= limit ? 0 : (calls < limit - used ? calls : limit - used);
    long long over = calls - within;
    long long billed = over;
    if (used > 0 && used < limit && used + calls > limit)
        billed = calls; /* Q2 */
    long long units;
    if (billed % 100 == 50)
        units = billed / 100; /* Q4 */
    else
        units = (billed + 99) / 100;
    long long charge = units * rate_for(acct->tier, region);

    if (first_event && over == 0)
        acct->bank += calls / 10; /* Q1 */
    acct->used += calls;

    long long left = base_quota(acct->tier) + acct->bank - acct->used;
    if (left < 0) left = 0;
    printf("B %s %lld %lld\n", acct->seen, charge, left);
    count_ok++;
    sum_charges += charge;
}

int main(void) {
    char line[512];
    while (fgets(line, sizeof line, stdin)) {
        line[strcspn(line, "\r\n")] = 0;
        if (!line[0]) continue;
        process(line);
    }
    printf("X %lld %lld %lld\n", count_ok, count_rej, sum_charges);
    return 0;
}
