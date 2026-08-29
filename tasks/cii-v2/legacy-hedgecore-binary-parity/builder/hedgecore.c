/* HedgeCore legacy FX position ledger. BUILD ARTIFACT ONLY - this source
 * is the task's hidden ground truth and must never ship in the
 * agent-visible repo.
 *
 * Stateful per-book, per-pair position ledger with two internal
 * computation paths (live trades and revaluations). Only COMPOSITE net
 * values are ever printed: per-pair values never appear, so divergences
 * cannot be isolated by column. The quirks ARE the contract, and they
 * live in the ASYMMETRY between the two paths composed with unanchored
 * stateful mechanics:
 *  Q1 (path asymmetry, the core): the LIVE path (reply to a T trade)
 *     rounds each pair's value half-even to cents and sums; the REVAL
 *     path (replies to a V revaluation) computes each pair in integer
 *     tenths of a cent (floored), sums ALL pairs, and rounds ONCE
 *     half-up at the end. A trade-print and a reval-print of identical
 *     state can differ by a cent, buried in the composite.
 *  Q2 (stateful, unanchored): a trade that flips a pair's position SIGN
 *     (strictly positive to strictly negative or vice versa; NOT exact
 *     flat) stores the crossing trade's rate MINUS 5 as the pair's rate
 *     (an ancient washout adjustment); every later composite carries the
 *     phantom offset. The constant 5 appears nowhere; it is discoverable
 *     only via constructed cross-flat position chains.
 *  Q3 (conjunction): pairs quoted in yen (chars 3-5 == "JPY") divide by
 *     100 instead of 10000 in BOTH paths (quote convention), but the
 *     REVAL path applies the 100 divisor only when the rate list of THAT
 *     V command explicitly mentions the pair; a JPY pair absent from the
 *     list revalues at the 10000 divisor (a stale branch never fixed).
 *  Q4 a trade that takes a pair exactly FLAT (new position zero) drops
 *     the pair from the book entirely, its stored rate forgotten; a
 *     later trade re-adds it fresh at that trade's rate. No washout
 *     fires on exact flat (Q2 needs a strict sign flip). The spec keeps
 *     flat pairs with rate memory.
 *  Q5 amount 0 passes field validation and is then skipped SILENTLY: no
 *     reply line, no state change, not counted anywhere. The spec
 *     rejects it AMT.
 *  Q6 book ids fold case; the first-seen spelling is echoed on every
 *     reply. The spec is case-sensitive.
 *  Q7 extra trailing tokens on T are tolerated and ignored; the V
 *     argument is line[2:] verbatim (never token-split), as the spec
 *     says, in all implementations.
 *
 * T validation order: FMT (command shape, token count, book id), PAIR
 * (6 uppercase letters), SIDE (B|S), AMT (1-8 digits), RATE (1-7
 * digits, nonzero). The FMT echo for an unusable book token is
 * '????????'. V validation (identical in all implementations): entries
 * comma-joined PAIR=rate, each pair 6 uppercase letters, each rate 1-7
 * digits nonzero; any damage rejects "R ???????? FMT" and leaves all
 * state untouched.
 *
 * All division is mathematical FLOOR division (matched exactly by the
 * gold Python); half-even and half-up are defined on the floored
 * quotient and remainder.
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAXBOOK 256
#define MAXPAIR 128
#define BOOKMAX 8

typedef struct {
    char pair[7];
    long long pos;
    long long rate;
} pos_t;

typedef struct {
    char fold[BOOKMAX + 1];
    char seen[BOOKMAX + 1];
    pos_t p[MAXPAIR];
    int np;
} book_t;

static book_t books[MAXBOOK];
static int nbook = 0;
static long long n_trade = 0, n_reval = 0, n_rej = 0;

/* floor division and remainder (C '/' truncates; we need floor) */
static long long fdiv(long long n, long long d) {
    long long q = n / d, r = n % d;
    if (r != 0 && ((r < 0) != (d < 0))) q--;
    return q;
}
static long long fmod_(long long n, long long d) { return n - fdiv(n, d) * d; }

static long long round_half_even(long long n, long long d) {
    long long q = fdiv(n, d), r = fmod_(n, d), doubled = 2 * r;
    if (doubled > d) return q + 1;
    if (doubled < d) return q;
    return (q % 2 == 0) ? q : q + 1;
}

static long long round_half_up(long long n, long long d) {
    long long q = fdiv(n, d), r = fmod_(n, d);
    return (2 * r >= d) ? q + 1 : q;
}

static int is_jpy(const char *pair) { return strncmp(pair + 3, "JPY", 3) == 0; }

static int span_alnum(const char *s, size_t n, size_t lo, size_t hi) {
    if (n < lo || n > hi) return 0;
    for (size_t i = 0; i < n; i++)
        if (!isalnum((unsigned char)s[i])) return 0;
    return 1;
}

static int span_digits(const char *s, size_t n, size_t lo, size_t hi) {
    if (n < lo || n > hi) return 0;
    for (size_t i = 0; i < n; i++)
        if (!isdigit((unsigned char)s[i])) return 0;
    return 1;
}

static int valid_pair(const char *s) {
    if (strlen(s) != 6) return 0;
    for (int i = 0; i < 6; i++)
        if (!isupper((unsigned char)s[i])) return 0;
    return 1;
}

static void reject(const char *who, const char *code) {
    printf("R %s %s\n", who, code);
    n_rej++;
}

static book_t *find_book(const char *fold) {
    for (int i = 0; i < nbook; i++)
        if (strcmp(books[i].fold, fold) == 0) return &books[i];
    return NULL;
}

static pos_t *find_pair(book_t *b, const char *pair) {
    for (int i = 0; i < b->np; i++)
        if (strcmp(b->p[i].pair, pair) == 0) return &b->p[i];
    return NULL;
}

/* Q1 live path: per-pair half-even, then sum. Q3: JPY divisor in full. */
static long long net_live(const book_t *b) {
    long long total = 0;
    for (int i = 0; i < b->np; i++) {
        long long div = is_jpy(b->p[i].pair) ? 100 : 10000;
        total += round_half_even(b->p[i].pos * b->p[i].rate, div);
    }
    return total;
}

static void do_trade(char *line) {
    char *tok[64];
    int nt = 0;
    for (char *p = strtok(line, " "); p && nt < 64; p = strtok(NULL, " ")) tok[nt++] = p;
    const char *book = nt > 1 ? tok[1] : "";
    int book_ok = span_alnum(book, strlen(book), 1, BOOKMAX);
    if (nt < 6 || !book_ok) { /* Q7: extras beyond 6 ignored */
        reject(book_ok ? book : "????????", "FMT");
        return;
    }
    const char *pair = tok[2], *side = tok[3], *amt_s = tok[4], *rate_s = tok[5];
    if (!valid_pair(pair)) { reject(book, "PAIR"); return; }
    if (strlen(side) != 1 || (side[0] != 'B' && side[0] != 'S')) {
        reject(book, "SIDE");
        return;
    }
    if (!span_digits(amt_s, strlen(amt_s), 1, 8)) { reject(book, "AMT"); return; }
    if (!span_digits(rate_s, strlen(rate_s), 1, 7)) { reject(book, "RATE"); return; }
    long long amt = atoll(amt_s), rate = atoll(rate_s);
    if (rate == 0) { reject(book, "RATE"); return; }
    if (amt == 0) return; /* Q5: silent skip, no state, no counts */

    char fold[BOOKMAX + 1];
    size_t bl = strlen(book);
    for (size_t i = 0; i < bl; i++) fold[i] = (char)tolower((unsigned char)book[i]);
    fold[bl] = 0;
    book_t *b = find_book(fold); /* Q6: case-folded books */
    if (!b) {
        if (nbook >= MAXBOOK) { reject(book, "FMT"); return; }
        b = &books[nbook++];
        strcpy(b->fold, fold);
        strcpy(b->seen, book);
        b->np = 0;
    }
    pos_t *e = find_pair(b, pair);
    long long old_pos = e ? e->pos : 0;
    long long new_pos = old_pos + (side[0] == 'B' ? amt : -amt);
    if (new_pos == 0) { /* Q4: exact flat drops the pair, rate forgotten */
        if (e) {
            int idx = (int)(e - b->p);
            memmove(&b->p[idx], &b->p[idx + 1],
                    (size_t)(b->np - idx - 1) * sizeof(pos_t));
            b->np--;
        }
    } else {
        if (!e) {
            if (b->np >= MAXPAIR) { reject(book, "FMT"); return; }
            e = &b->p[b->np++];
            strcpy(e->pair, pair);
        }
        int flip = old_pos != 0 && ((old_pos > 0) != (new_pos > 0));
        e->pos = new_pos;
        e->rate = flip ? rate - 5 : rate; /* Q2: washout on sign flips */
    }
    printf("P %s %lld\n", b->seen, net_live(b));
    n_trade++;
}

static void do_reval(const char *arg) {
    /* parse and validate the whole list before touching any state */
    char pairs[128][7];
    long long rates[128];
    int nent = 0;
    size_t n = strlen(arg);
    if (n == 0) { reject("????????", "FMT"); return; }
    size_t start = 0;
    for (size_t i = 0; i <= n; i++) {
        if (i != n && arg[i] != ',') continue;
        const char *piece = arg + start;
        size_t plen = i - start;
        size_t eq = plen;
        int neq = 0;
        for (size_t j = 0; j < plen; j++)
            if (piece[j] == '=') { neq++; eq = j; }
        if (neq != 1) { reject("????????", "FMT"); return; }
        size_t kl = eq, vl = plen - eq - 1;
        if (kl != 6) { reject("????????", "FMT"); return; }
        char pr[7];
        for (size_t j = 0; j < 6; j++) {
            if (!isupper((unsigned char)piece[j])) { reject("????????", "FMT"); return; }
            pr[j] = piece[j];
        }
        pr[6] = 0;
        if (!span_digits(piece + eq + 1, vl, 1, 7)) { reject("????????", "FMT"); return; }
        long long rv = atoll(piece + eq + 1);
        if (rv == 0) { reject("????????", "FMT"); return; }
        int hit = -1;
        for (int j = 0; j < nent; j++)
            if (strcmp(pairs[j], pr) == 0) { hit = j; break; }
        if (hit >= 0) rates[hit] = rv; /* duplicate pair: last wins */
        else if (nent < 128) { strcpy(pairs[nent], pr); rates[nent++] = rv; }
        start = i + 1;
    }
    for (int i = 0; i < nbook; i++) {
        book_t *b = &books[i];
        long long tenths = 0;
        for (int j = 0; j < b->np; j++) {
            pos_t *e = &b->p[j];
            int hit = -1;
            for (int k = 0; k < nent; k++)
                if (strcmp(pairs[k], e->pair) == 0) { hit = k; break; }
            long long rate, div;
            if (hit >= 0) {
                e->rate = rates[hit]; /* mentioned pairs re-rate */
                rate = e->rate;
                /* Q3: the JPY divisor only when explicitly mentioned */
                div = is_jpy(e->pair) ? 100 : 10000;
            } else {
                rate = e->rate; /* absent pairs keep their last rate */
                div = 10000;    /* Q3: stale branch, even for JPY */
            }
            /* Q1 reval path: floored integer tenths of a cent */
            tenths += fdiv(e->pos * rate * 10, div);
        }
        /* Q1: one half-up rounding over the whole book */
        printf("P %s %lld\n", b->seen, round_half_up(tenths, 10));
    }
    n_reval++;
}

int main(void) {
    static char buf[262144];
    while (fgets(buf, sizeof buf, stdin)) {
        buf[strcspn(buf, "\r\n")] = 0;
        if (buf[0] == 0) continue;
        if (buf[0] == 'T' && (buf[1] == 0 || buf[1] == ' ')) {
            do_trade(buf);
        } else if (buf[0] == 'V' && buf[1] == ' ') {
            do_reval(buf + 2); /* Q7: taken verbatim, never token-split */
        } else {
            reject("????????", "FMT");
        }
    }
    printf("X %lld %lld %lld\n", n_trade, n_reval, n_rej);
    return 0;
}
