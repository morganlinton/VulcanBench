/* PayrollCore legacy withholding engine. BUILD ARTIFACT ONLY - hidden
 * ground truth; must never ship in the agent-visible repo.
 *
 * Batch-stateful: year-to-date gross accumulates per employee WITHIN the
 * batch (case-insensitively, first-seen spelling reported), so a record's
 * treatment depends on the records before it.
 *
 * Engine quirks (spec says otherwise or says nothing):
 *  Q1 (conjunction) status H AND state in {CA, NY} AND per-period gross
 *     strictly over 250000: the top federal marginal rate is 32%, not 30%.
 *  Q2 (stateful) the social levy's 1,600,000 YTD cap is checked BEFORE
 *     the record: the record that crosses the cap pays the levy on its
 *     FULL gross, then the levy stops. (Spec: prorate the crossing
 *     record so the levy applies only under the cap.)
 *  Q3 status M relief: engine computes bracket federal, rounds half-even,
 *     then applies the 8% discount and rounds half-even again. (Spec
 *     reads as a rate discount; the naive multiplies rates by 0.92 and
 *     rounds half-up once.)
 *  Q4 zero-tax states are {TX, FL, WA, NV, SD}: SD is undocumented.
 *  Q5 (conjunction) flags containing both 'X' and 'B': +1% of gross added
 *     to the levy, cap or no cap.
 *  Q6 period 27 is accepted (bonus run; spec allows only 01-26) and
 *     computes federal on HALVED bracket thresholds.
 *  Q7 gross 0 is rejected GROSS (spec: valid, net 0).
 *  Q8 employee ids fold case for YTD accumulation.
 *  Q9 trailing tokens are tolerated.
 *
 * Input:  P <empid:1-8 alnum> <period:01-27> <gross_cents:1-9 digits>
 *           <status:S|M|H> <state:2 upper> <flags:3 of A-Z, or 000>
 * Output: W <empid> <net_cents>   (net = gross - federal - state - levy)
 *         R <empid> <code>        FMT PERIOD GROSS STATUS STATE FLAGS
 * Trailer: X <accepted> <rejected> <sum_net>
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAXEMP 4096
#define LEVY_CAP 1600000LL
#define LEVY_BP 620
#define STATE_BP 500

typedef struct {
    char key[9];   /* lowercased id */
    char seen[9];  /* first-seen spelling */
    long long ytd;
} emp_t;

static emp_t emps[MAXEMP];
static int nemp = 0;
static long long count_ok = 0, count_rej = 0, sum_net = 0;

typedef enum { R_HALF_EVEN, R_HALF_UP } rmode_t;

static long long div_round(long long n, long long d, rmode_t mode) {
    long long q = n / d, r = n % d;
    if (r == 0) return q;
    if (mode == R_HALF_UP) return (2 * r >= d) ? q + 1 : q;
    if (2 * r > d) return q + 1;
    if (2 * r < d) return q;
    return (q % 2 == 0) ? q : q + 1;
}

static emp_t *emp_find(const char *id) {
    char key[9];
    size_t n = strlen(id);
    for (size_t i = 0; i <= n; i++) key[i] = (char)tolower((unsigned char)id[i]);
    for (int i = 0; i < nemp; i++)
        if (strcmp(emps[i].key, key) == 0) return &emps[i];
    if (nemp >= MAXEMP) return NULL;
    emp_t *e = &emps[nemp++];
    snprintf(e->key, sizeof e->key, "%s", key);
    snprintf(e->seen, sizeof e->seen, "%s", id);
    e->ytd = 0;
    return e;
}

static int tok_class(const char *s, int lo, int hi, int (*cls)(int)) {
    int n = (int)strlen(s);
    if (n < lo || n > hi) return 0;
    for (int i = 0; i < n; i++)
        if (!cls((unsigned char)s[i])) return 0;
    return 1;
}

static long long federal(long long gross, char status, const char *state, int bonus) {
    long long b1 = 100000, b2 = 400000;
    if (bonus) { b1 /= 2; b2 /= 2; } /* Q6 */
    long long top_bp = 3000;
    if (status == 'H' && (strcmp(state, "CA") == 0 || strcmp(state, "NY") == 0) &&
        gross > 250000)
        top_bp = 3200; /* Q1 */
    long long tax_tenth_bp = 0; /* accumulate in basis points x gross */
    long long t1 = gross < b1 ? gross : b1;
    long long t2 = gross < b2 ? (gross > b1 ? gross - b1 : 0) : b2 - b1;
    long long t3 = gross > b2 ? gross - b2 : 0;
    long long raw = t1 * 1000 + t2 * 2000 + t3 * top_bp;
    (void)tax_tenth_bp;
    long long fed = div_round(raw, 10000, R_HALF_EVEN);
    if (status == 'M') { /* Q3: discount after rounding, rounded again */
        fed = div_round(fed * 92, 100, R_HALF_EVEN);
    }
    return fed;
}

static void process(char *line) {
    char kind[8] = "", id[64] = "", ptok[64] = "", gtok[64] = "",
         stok[64] = "", state[64] = "", flags[64] = "";
    int n = sscanf(line, "%7s %63s %63s %63s %63s %63s %63s",
                   kind, id, ptok, gtok, stok, state, flags);
    int id_ok = tok_class(id, 1, 8, isalnum);
    if (strcmp(kind, "P") != 0 || n < 7 || !id_ok) {
        printf("R %s FMT\n", id_ok ? id : "????????");
        count_rej++;
        return;
    }
    if (!tok_class(ptok, 2, 2, isdigit)) { printf("R %s PERIOD\n", id); count_rej++; return; }
    int period = atoi(ptok);
    if (period < 1 || period > 27) { printf("R %s PERIOD\n", id); count_rej++; return; } /* Q6 */
    if (!tok_class(gtok, 1, 9, isdigit)) { printf("R %s GROSS\n", id); count_rej++; return; }
    long long gross = atoll(gtok);
    if (gross == 0) { printf("R %s GROSS\n", id); count_rej++; return; } /* Q7 */
    if (strlen(stok) != 1 || !strchr("SMH", stok[0])) {
        printf("R %s STATUS\n", id); count_rej++; return;
    }
    if (!tok_class(state, 2, 2, isupper)) { printf("R %s STATE\n", id); count_rej++; return; }
    int flags_ok = strcmp(flags, "000") == 0 || tok_class(flags, 3, 3, isupper);
    if (!flags_ok) { printf("R %s FLAGS\n", id); count_rej++; return; }

    emp_t *emp = emp_find(id); /* Q8 */
    if (!emp) { printf("R %s FMT\n", id); count_rej++; return; }

    long long fed = federal(gross, stok[0], state, period == 27);

    long long st = 0;
    if (strcmp(state, "TX") && strcmp(state, "FL") && strcmp(state, "WA") &&
        strcmp(state, "NV") && strcmp(state, "SD")) /* Q4 */
        st = div_round(gross * STATE_BP, 10000, R_HALF_EVEN);

    long long levy = 0;
    if (emp->ytd < LEVY_CAP) /* Q2: full levy on the crossing record */
        levy = div_round(gross * LEVY_BP, 10000, R_HALF_UP);
    if (strchr(flags, 'X') && strchr(flags, 'B')) /* Q5 */
        levy += div_round(gross * 100, 10000, R_HALF_UP);

    emp->ytd += gross;

    long long net = gross - fed - st - levy;
    printf("W %s %lld\n", emp->seen, net);
    count_ok++;
    sum_net += net;
}

int main(void) {
    char line[512];
    while (fgets(line, sizeof line, stdin)) {
        line[strcspn(line, "\r\n")] = 0;
        if (!line[0]) continue;
        process(line);
    }
    printf("X %lld %lld %lld\n", count_ok, count_rej, sum_net);
    return 0;
}
