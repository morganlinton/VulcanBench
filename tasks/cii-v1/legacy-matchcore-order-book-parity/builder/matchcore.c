/* MatchCore legacy matching engine. BUILD ARTIFACT ONLY - hidden ground
 * truth; must never ship in the agent-visible repo.
 *
 * Engine quirks (the published spec says otherwise or says nothing):
 *  Q1 Equal-price priority: resting orders with ORIGINAL quantity >= 1000
 *     form a priority class that matches before smaller orders at the same
 *     price; FIFO within each class. (Spec: strict price-time FIFO.)
 *  Q2 Self-trade prevention cancels the RESTING order and keeps matching
 *     the incoming one. (Spec: the incoming order is rejected, code STP.)
 *  Q3 A new order reusing the client order id of a LIVE resting order from
 *     the same account is a cancel-replace: the old order is cancelled and
 *     the new one processed. (Spec: reject, code DUP.)
 *  Q4 Cancel acknowledgements report the ORIGINAL order quantity, not the
 *     remaining quantity. (Spec: remaining.)
 *  Q5 Volatility band: within one incoming order's matching sweep, once a
 *     candidate fill's price differs from the FIRST fill's price by more
 *     than 500, matching stops; unfilled market/IOC remainder is cancelled
 *     and unfilled limit remainder rests. (Spec: no band.)
 *  Q6 Tick regime: prices >= 100000 must be multiples of 5, else REJ TICK.
 *     (Spec: any positive integer price below 10000000.)
 *  Q7 The end-of-session book hash excludes resting orders whose REMAINING
 *     quantity is below 100. (Spec: all resting orders.)
 *
 * Message formats (one per line, single space separated):
 *   N <acct:4> <clordid:8> <B|S> <L|M|I> <price:1-7 digits> <qty:1-6 digits>
 *   C <acct:4> <clordid:8>
 *   E
 * Outputs:
 *   A <clordid> <restingqty>      accept (restingqty 0 for fully-filled/IOC/M)
 *   F <resting> <incoming> <price> <qty>
 *   X <clordid> <qty>             cancelled (Q4: original qty)
 *   R <clordid> <code>            REJ codes: PRICE TICK QTY UNKNOWN FMT
 *   S <bid> <ask> <hash>          session summary on E
 */

#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAXORD 100000
#define BAND 500
#define PRIO_QTY 1000
#define HASHMOD 1000000007LL

typedef struct {
    char acct[5], id[9];
    int side;      /* 0 buy, 1 sell */
    long price;
    long qty;      /* remaining */
    long orig;
    long seq;
    int live;
} order_t;

static order_t book[MAXORD];
static int nord = 0;
static long seqno = 0;

static void emit_cancel(const char *id, long orig) { printf("X %s %ld\n", id, orig); } /* Q4 */

static order_t *find_live(const char *acct, const char *id) {
    for (int i = 0; i < nord; i++)
        if (book[i].live && strcmp(book[i].acct, acct) == 0 && strcmp(book[i].id, id) == 0)
            return &book[i];
    return NULL;
}

static order_t *find_live_id(const char *id) {
    for (int i = 0; i < nord; i++)
        if (book[i].live && strcmp(book[i].id, id) == 0) return &book[i];
    return NULL;
}

/* Best matchable resting order for an incoming order, honoring Q1. */
static order_t *best_counter(int side, long limit, int is_market) {
    order_t *best = NULL;
    for (int i = 0; i < nord; i++) {
        order_t *o = &book[i];
        if (!o->live || o->side == side) continue;
        if (!is_market) {
            if (side == 0 && o->price > limit) continue; /* buy vs ask */
            if (side == 1 && o->price < limit) continue; /* sell vs bid */
        }
        if (best == NULL) { best = o; continue; }
        int better_price = side == 0 ? (o->price < best->price) : (o->price > best->price);
        int same_price = o->price == best->price;
        if (better_price) { best = o; continue; }
        if (!same_price) continue;
        int o_cls = o->orig >= PRIO_QTY, b_cls = best->orig >= PRIO_QTY; /* Q1 */
        if (o_cls != b_cls) { if (o_cls) best = o; continue; }
        if (o->seq < best->seq) best = o;
    }
    return best;
}

static void process_new(char *acct, char *id, char side_c, char type_c, long price, long qty) {
    if (qty <= 0 || qty > 999999) { printf("R %s QTY\n", id); return; }
    int is_market = type_c == 'M';
    if (!is_market) {
        if (price <= 0 || price >= 10000000) { printf("R %s PRICE\n", id); return; }
        if (price >= 100000 && price % 5 != 0) { printf("R %s TICK\n", id); return; } /* Q6 */
    }
    order_t *dup = find_live(acct, id);
    if (dup) { /* Q3: cancel-replace */
        dup->live = 0;
        emit_cancel(dup->id, dup->orig);
    }

    int side = side_c == 'S';
    long remaining = qty;
    long first_fill_price = -1;

    for (;;) {
        if (remaining == 0) break;
        order_t *counter = best_counter(side, price, is_market);
        if (!counter) break;
        if (strcmp(counter->acct, acct) == 0) { /* Q2 */
            counter->live = 0;
            emit_cancel(counter->id, counter->orig);
            continue;
        }
        if (first_fill_price >= 0 && labs(counter->price - first_fill_price) > BAND) /* Q5 */
            break;
        if (first_fill_price < 0) first_fill_price = counter->price;
        long take = remaining < counter->qty ? remaining : counter->qty;
        printf("F %s %s %ld %ld\n", counter->id, id, counter->price, take);
        counter->qty -= take;
        remaining -= take;
        if (counter->qty == 0) counter->live = 0;
    }

    if (remaining > 0 && type_c == 'L') {
        if (nord >= MAXORD) { printf("R %s FMT\n", id); return; }
        order_t *o = &book[nord++];
        snprintf(o->acct, sizeof o->acct, "%s", acct);
        snprintf(o->id, sizeof o->id, "%s", id);
        o->side = side;
        o->price = price;
        o->qty = remaining;
        o->orig = qty;
        o->seq = ++seqno;
        o->live = 1;
        printf("A %s %ld\n", id, remaining);
    } else {
        printf("A %s 0\n", id);
    }
}

static void process_cancel(char *acct, char *id) {
    order_t *o = find_live(acct, id);
    if (!o) { printf("R %s UNKNOWN\n", id); return; }
    o->live = 0;
    emit_cancel(o->id, o->orig); /* Q4 */
}

static void process_end(void) {
    long bid = 0, ask = 0;
    long long hash = 0;
    for (int i = 0; i < nord; i++) {
        order_t *o = &book[i];
        if (!o->live) continue;
        if (o->side == 0 && o->price > bid) bid = o->price;
        if (o->side == 1 && (ask == 0 || o->price < ask)) ask = o->price;
        if (o->qty >= 100) /* Q7 */
            hash = (hash + (long long)o->price % HASHMOD * (o->qty % HASHMOD)) % HASHMOD;
    }
    printf("S %ld %ld %lld\n", bid, ask, hash);
}

static int tok_ok(const char *s, size_t lo, size_t hi, int digits) {
    size_t n = strlen(s);
    if (n < lo || n > hi) return 0;
    for (size_t i = 0; i < n; i++) {
        if (digits && !isdigit((unsigned char)s[i])) return 0;
        if (!digits && !isalnum((unsigned char)s[i])) return 0;
    }
    return 1;
}

int main(void) {
    char line[256];
    while (fgets(line, sizeof line, stdin)) {
        line[strcspn(line, "\r\n")] = 0;
        if (!line[0]) continue;
        char kind = line[0];
        if (kind == 'E' && line[1] == 0) { process_end(); continue; }
        char acct[64] = "", id[64] = "", side = 0, type = 0;
        char pricebuf[64] = "", qtybuf[64] = "";
        if (kind == 'N') {
            int n = sscanf(line, "N %63s %63s %c %c %63s %63s", acct, id, &side, &type, pricebuf, qtybuf);
            if (n != 6 || !tok_ok(acct, 4, 4, 0) || !tok_ok(id, 1, 8, 0) ||
                (side != 'B' && side != 'S') || (type != 'L' && type != 'M' && type != 'I') ||
                !tok_ok(pricebuf, 1, 7, 1) || !tok_ok(qtybuf, 1, 6, 1)) {
                printf("R %s FMT\n", tok_ok(id, 1, 8, 0) ? id : "????????");
                continue;
            }
            process_new(acct, id, side, type, atol(pricebuf), atol(qtybuf));
        } else if (kind == 'C') {
            int n = sscanf(line, "C %63s %63s", acct, id);
            if (n != 2 || !tok_ok(acct, 4, 4, 0) || !tok_ok(id, 1, 8, 0)) {
                printf("R %s FMT\n", tok_ok(id, 1, 8, 0) ? id : "????????");
                continue;
            }
            process_cancel(acct, id);
        } else {
            printf("R ???????? FMT\n");
        }
    }
    return 0;
}
