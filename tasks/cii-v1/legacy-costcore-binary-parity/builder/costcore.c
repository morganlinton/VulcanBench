/* CostCore legacy inventory cost-layer engine. BUILD ARTIFACT ONLY -
 * hidden ground truth; must never ship in the agent-visible repo.
 *
 * Batch-stateful at DEPTH: each SKU carries an ordered list of cost
 * layers (case-insensitive key, first-seen spelling reported), and every
 * issue's cost depends on the exact layer list built by everything
 * before it.
 *
 * Engine quirks (spec says otherwise or says nothing):
 *  Q1 (stateful, dark) "display stock": while a SKU has more than one
 *     layer, the newest layer is off-limits to issues; an issue draws
 *     oldest-first from the older layers only, and touches the newest
 *     only once it is the sole remaining layer. Availability for the
 *     STOCK check and for Q5 is therefore on-hand MINUS the newest
 *     layer whenever two or more layers exist. (Spec: plain FIFO over
 *     everything on hand.)
 *  Q2 (stateful) a receipt whose unit cost equals the NEWEST existing
 *     layer's cost merges into that layer (quantity added) instead of
 *     appending a new layer. Interacts with Q1: the merged layer is
 *     still the one protected newest layer. (Spec: every receipt is its
 *     own layer.)
 *  Q3 (conjunction) an issue that empties a SKU to exactly 0 on-hand
 *     with a consumed quantity of 100 or more adds a 2% shrinkage
 *     surcharge to that issue's COGS, rounded half-up (stocktake
 *     writeoff convention).
 *  Q4 unit cost is silently capped at 9,000,000 cents on receipt; the
 *     spec's 7-digit field reads up to 9,999,999.
 *  Q5 an issue whose quantity exceeds availability PARTIALLY fills
 *     (consumes everything available, reports the partial COGS) when at
 *     least half the requested quantity is available; only below half is
 *     it rejected STOCK. (Spec: any shortfall rejects STOCK.)
 *     Availability composes with Q1's display-stock exclusion.
 *  Q6 SKUs fold case for state; the first-seen spelling is echoed.
 *  Q7 quantity 0 lines are skipped silently (no output, not counted).
 *     (Spec: quantities are 1-999999, so 0 rejects QTY.)
 *  Q8 trailing tokens are tolerated.
 *
 * Input:  R <sku:1-8 alnum> <qty:1-6 digits> <unit_cost:1-7 digits>
 *         I <sku:1-8 alnum> <qty:1-6 digits>
 * Output: A <sku> <onhand_after>            (receipt accepted)
 *         C <sku> <cogs_cents> <onhand_after>  (issue accepted)
 *         E <sku> <code>                    codes FMT QTY COST STOCK
 * Trailer: X <receipts> <issues> <rejected> <sum_cogs>
 * Validation order: FMT, QTY, COST (R only), STOCK (I only).
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAXSKU 4096
#define MAXLAYER 4096
#define COST_CAP 9000000LL

typedef struct {
    long long qty, cost;
} layer_t;

typedef struct {
    char key[9];  /* lowercased sku */
    char seen[9]; /* first-seen spelling */
    layer_t layers[MAXLAYER];
    int nlayers;
} sku_t;

static sku_t skus[MAXSKU];
static int nsku = 0;
static long long count_rcpt = 0, count_iss = 0, count_rej = 0, sum_cogs = 0;

static long long round_half_up(long long n, long long d) {
    long long q = n / d, r = n % d;
    if (r == 0) return q;
    return (2 * r >= d) ? q + 1 : q;
}

static sku_t *sku_find(const char *id) {
    char key[9];
    size_t n = strlen(id);
    for (size_t i = 0; i <= n; i++) key[i] = (char)tolower((unsigned char)id[i]);
    for (int i = 0; i < nsku; i++)
        if (strcmp(skus[i].key, key) == 0) return &skus[i];
    if (nsku >= MAXSKU) return NULL;
    sku_t *s = &skus[nsku++];
    snprintf(s->key, sizeof s->key, "%s", key);
    snprintf(s->seen, sizeof s->seen, "%s", id);
    s->nlayers = 0;
    return s;
}

static long long onhand(const sku_t *s) {
    long long t = 0;
    for (int i = 0; i < s->nlayers; i++) t += s->layers[i].qty;
    return t;
}

static int tok_class(const char *s, int lo, int hi, int (*cls)(int)) {
    int n = (int)strlen(s);
    if (n < lo || n > hi) return 0;
    for (int i = 0; i < n; i++)
        if (!cls((unsigned char)s[i])) return 0;
    return 1;
}

static void drop_layer(sku_t *s, int idx) {
    for (int i = idx; i < s->nlayers - 1; i++) s->layers[i] = s->layers[i + 1];
    s->nlayers--;
}

/* Q1: the newest layer is "display stock": while more than one layer
 * exists it is off-limits to issues, so an issue draws oldest-first from
 * the older layers only. The newest layer is touched only once it is the
 * sole remaining layer. `avail_q1` is what an issue may draw right now. */
static long long avail_q1(const sku_t *s) {
    if (s->nlayers <= 1) return onhand(s);
    return onhand(s) - s->layers[s->nlayers - 1].qty;
}

static long long consume(sku_t *s, long long take) {
    long long cogs = 0;
    while (take > 0 && s->nlayers > 0) {
        layer_t *ly = &s->layers[0]; /* never the newest while >1 exist */
        long long got = ly->qty < take ? ly->qty : take;
        cogs += got * ly->cost;
        ly->qty -= got;
        take -= got;
        if (ly->qty == 0) drop_layer(s, 0);
    }
    return cogs;
}

static void process(char *line) {
    char kind[8] = "", sku[64] = "", qtok[64] = "", ctok[64] = "";
    int n = sscanf(line, "%7s %63s %63s %63s", kind, sku, qtok, ctok);
    int sku_ok = tok_class(sku, 1, 8, isalnum);
    int is_r = strcmp(kind, "R") == 0, is_i = strcmp(kind, "I") == 0;
    int need = is_r ? 4 : is_i ? 3 : 99;
    if ((!is_r && !is_i) || !sku_ok || n < need) { /* Q8: extra tokens ok */
        printf("E %s FMT\n", sku_ok ? sku : "????????");
        count_rej++;
        return;
    }
    if (!tok_class(qtok, 1, 6, isdigit)) {
        printf("E %s QTY\n", sku);
        count_rej++;
        return;
    }
    long long qty = atoll(qtok);
    if (qty == 0) return; /* Q7: silent skip */
    if (is_r) {
        if (!tok_class(ctok, 1, 7, isdigit)) {
            printf("E %s COST\n", sku);
            count_rej++;
            return;
        }
        long long cost = atoll(ctok);
        if (cost > COST_CAP) cost = COST_CAP; /* Q4 */
        sku_t *s = sku_find(sku); /* Q6 */
        if (!s) { printf("E %s FMT\n", sku); count_rej++; return; }
        if (s->nlayers > 0 && s->layers[s->nlayers - 1].cost == cost) {
            s->layers[s->nlayers - 1].qty += qty; /* Q2: merge */
        } else if (s->nlayers < MAXLAYER) {
            s->layers[s->nlayers].qty = qty;
            s->layers[s->nlayers].cost = cost;
            s->nlayers++;
        }
        printf("A %s %lld\n", s->seen, onhand(s));
        count_rcpt++;
        return;
    }
    /* issue */
    sku_t *s = sku_find(sku); /* Q6 */
    if (!s) { printf("E %s FMT\n", sku); count_rej++; return; }
    long long avail = avail_q1(s); /* Q1: newest layer excluded */
    long long take = qty;
    if (qty > avail) {
        if (2 * avail >= qty) { /* Q5: partial fill at half or more */
            take = avail;
        } else {
            printf("E %s STOCK\n", s->seen);
            count_rej++;
            return;
        }
    }
    long long cogs = consume(s, take);
    long long after = onhand(s);
    if (after == 0 && take >= 100) /* Q3 */
        cogs += round_half_up(cogs * 2, 100);
    printf("C %s %lld %lld\n", s->seen, cogs, after);
    count_iss++;
    sum_cogs += cogs;
}

int main(void) {
    char line[512];
    while (fgets(line, sizeof line, stdin)) {
        line[strcspn(line, "\r\n")] = 0;
        if (!line[0]) continue;
        process(line);
    }
    printf("X %lld %lld %lld %lld\n", count_rcpt, count_iss, count_rej, sum_cogs);
    return 0;
}
