/* TallyCore legacy ranked-ballot tally engine. BUILD ARTIFACT ONLY - this
 * source is the task's hidden ground truth and must never ship in the
 * agent-visible repo.
 *
 * Stateful instant-runoff tally engine with an INCREMENTAL running-leader
 * path (V) and a RECOUNT path (W). Behavior is deliberately quirky; the
 * quirks ARE the contract, and they live in the ASYMMETRY between the two
 * paths, composed with hidden state carried ACROSS recount rounds:
 *  Q1  (core, stateful) The incremental leader calc counts plain first
 *      choices, as documented. The recount path seeds every candidate's
 *      tally with a hidden carryover of floor(final_tally_prev_round / 4)
 *      from the PREVIOUS recount (momentum). A candidate's final tally is
 *      the tally it held at the moment of its elimination (the winner's:
 *      after the last elimination). Zero in round 1, so single-round flows
 *      never see it; multi-round W outcomes diverge from the spec.
 *  Q2  Elimination tie-break: among tied minimum tallies the engine
 *      eliminates the candidate with the LONGEST id first (a leftover
 *      sort key), falling back to last-registered among equal lengths.
 *      Spec/naive: last-registered only.
 *  Q3  A ballot ranking ALL registered candidates gets a 2x weight on its
 *      FIRST choice only, and only in the recount path (a completeness
 *      bonus); once the ballot transfers, its weight is 1. The
 *      incremental leader path never weights.
 *  Q4  Transfers work as documented EXCEPT when the eliminated candidate
 *      was the ballot's FIRST choice AND the ballot ranked exactly 2
 *      candidates: such ballots are discarded entirely (an ancient
 *      exhaustion bug), never transferred.
 *  Q5  V ranking a single candidate: accepted (both). Duplicate in a
 *      ranking: DUP (both). UNKNOWN candidate in a ranking: the engine
 *      silently drops that entry and processes the rest (rejecting CAND,
 *      echoing the first unknown, only when nothing survives); the spec
 *      says reject CAND outright.
 *  Q6  Candidate ids fold case, first-seen spelling reported everywhere;
 *      spec/naive are case-sensitive.
 *  Q7  Extra trailing tokens on C and V are tolerated and ignored; W must
 *      be exactly "W" in all implementations. Rejects echo the offending
 *      id, or "????????" when none is parseable.
 *
 * V validation order: FMT (structure: token present, comma-split entries
 * all non-empty alnum of 1-8 chars), then per-entry left-to-right
 * CAND/DUP. C validation order: FMT, then LATE (once any ballot of the
 * current round is in, registration closes until the next W).
 *
 * Input:   C <cand>       register (reply OK <count>)
 *          V <ranking>    comma-joined ids, no spaces (reply LEAD <cand>)
 *          W              recount: ELIM lines in order, then WIN; ballots
 *                         clear, candidates stay, registration reopens
 * Rejects: N <cand-or-????????> <FMT|CAND|DUP|LATE>
 * Trailer: X <candidates> <ballots> <rounds> <rejected>
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CANDMAX 64
#define IDMAX 8
#define BALMAX 4096

typedef struct {
    char seen[IDMAX + 1];
    char fold[IDMAX + 1];
    long long carry;
} cand_t;

typedef struct {
    int e[CANDMAX];
    int n;
} ballot_t;

static cand_t cands[CANDMAX];
static int ncand = 0;
static ballot_t bals[BALMAX];
static int nbal = 0;
static long long total_ballots = 0, rounds = 0, rejects = 0;

static void reject(const char *echo, const char *code) {
    printf("N %s %s\n", echo, code);
    rejects++;
}

static int valid_id(const char *s, size_t n) {
    if (n < 1 || n > IDMAX) return 0;
    for (size_t i = 0; i < n; i++)
        if (!isalnum((unsigned char)s[i])) return 0;
    return 1;
}

static void fold_of(const char *s, char *out) {
    size_t i = 0;
    for (; s[i]; i++) out[i] = (char)tolower((unsigned char)s[i]);
    out[i] = 0;
}

static int find_fold(const char *fkey) {
    for (int i = 0; i < ncand; i++)
        if (strcmp(cands[i].fold, fkey) == 0) return i;
    return -1;
}

static void do_cand(char *line) {
    char *tok[64];
    int nt = 0;
    for (char *p = strtok(line, " "); p && nt < 64; p = strtok(NULL, " ")) tok[nt++] = p;
    if (nt < 2) { reject("????????", "FMT"); return; } /* Q7: extras ignored */
    const char *id = tok[1];
    if (!valid_id(id, strlen(id))) { reject("????????", "FMT"); return; }
    if (nbal > 0) { reject(id, "LATE"); return; }
    char fkey[IDMAX + 1];
    fold_of(id, fkey); /* Q6 */
    int i = find_fold(fkey);
    if (i < 0) {
        if (ncand >= CANDMAX) { reject("????????", "FMT"); return; }
        i = ncand++;
        snprintf(cands[i].seen, sizeof cands[i].seen, "%s", id);
        strcpy(cands[i].fold, fkey);
        cands[i].carry = 0;
    }
    printf("OK %d\n", ncand);
}

static void do_vote(char *line) {
    char *tok[64];
    int nt = 0;
    for (char *p = strtok(line, " "); p && nt < 64; p = strtok(NULL, " ")) tok[nt++] = p;
    if (nt < 2) { reject("????????", "FMT"); return; } /* Q7: extras ignored */
    char *rank = tok[1];
    /* FMT pass: manual comma split, every entry 1-8 alnum */
    char *entries[CANDMAX * 4];
    int ne = 0;
    char *start = rank;
    for (char *p = rank;; p++) {
        if (*p == ',' || *p == 0) {
            int last = (*p == 0);
            *p = 0;
            if (!valid_id(start, strlen(start)) || ne >= CANDMAX * 4) {
                reject("????????", "FMT");
                return;
            }
            entries[ne++] = start;
            if (last) break;
            start = p + 1;
        }
    }
    /* per-entry CAND/DUP, left to right */
    int kept[CANDMAX];
    int nk = 0;
    const char *first_unknown = NULL;
    for (int i = 0; i < ne; i++) {
        char fkey[IDMAX + 1];
        fold_of(entries[i], fkey); /* Q6 */
        int c = find_fold(fkey);
        if (c < 0) { /* Q5: unknown entries silently dropped */
            if (!first_unknown) first_unknown = entries[i];
            continue;
        }
        int dup = 0;
        for (int j = 0; j < nk; j++)
            if (kept[j] == c) { dup = 1; break; }
        if (dup) { reject(entries[i], "DUP"); return; }
        kept[nk++] = c;
    }
    if (nk == 0) { reject(first_unknown, "CAND"); return; }
    if (nbal >= BALMAX) { reject("????????", "FMT"); return; }
    ballot_t *b = &bals[nbal++];
    b->n = nk;
    memcpy(b->e, kept, (size_t)nk * sizeof(int));
    total_ballots++;
    /* incremental leader: plain first choices, ties to earliest registered */
    long long counts[CANDMAX] = {0};
    for (int i = 0; i < nbal; i++) counts[bals[i].e[0]]++;
    int lead = 0;
    for (int c = 1; c < ncand; c++)
        if (counts[c] > counts[lead]) lead = c;
    printf("LEAD %s\n", cands[lead].seen);
}

static int alive[CANDMAX];
static int dead_ballot[BALMAX];
static int is_full[BALMAX];

static void tallies(long long *tally) {
    for (int c = 0; c < ncand; c++) tally[c] = alive[c] ? cands[c].carry : 0; /* Q1 */
    for (int b = 0; b < nbal; b++) {
        if (dead_ballot[b]) continue;
        int top = -1, k = -1;
        for (int i = 0; i < bals[b].n; i++)
            if (alive[bals[b].e[i]]) { top = bals[b].e[i]; k = i; break; }
        if (top < 0) continue; /* exhausted */
        long long w = (is_full[b] && k == 0) ? 2 : 1; /* Q3 */
        tally[top] += w;
    }
}

static void do_recount(void) {
    if (nbal == 0) { reject("????????", "FMT"); return; }
    long long tally[CANDMAX], final_t[CANDMAX] = {0};
    for (int c = 0; c < ncand; c++) alive[c] = 1;
    for (int b = 0; b < nbal; b++) {
        dead_ballot[b] = 0;
        is_full[b] = (bals[b].n == ncand);
    }
    int remaining = ncand;
    while (remaining > 1) {
        tallies(tally);
        int v = -1;
        for (int c = 0; c < ncand; c++) {
            if (!alive[c]) continue;
            if (v < 0) { v = c; continue; }
            if (tally[c] < tally[v]) { v = c; continue; }
            if (tally[c] == tally[v]) {
                size_t lc = strlen(cands[c].fold), lv = strlen(cands[v].fold);
                if (lc > lv) v = c;               /* Q2: longest id first */
                else if (lc == lv && c > v) v = c; /* then last registered */
            }
        }
        printf("ELIM %s\n", cands[v].seen);
        final_t[v] = tally[v];
        for (int b = 0; b < nbal; b++) { /* Q4: exhaustion bug */
            if (dead_ballot[b]) continue;
            int top = -1;
            for (int i = 0; i < bals[b].n; i++)
                if (alive[bals[b].e[i]]) { top = bals[b].e[i]; break; }
            if (top == v && bals[b].e[0] == v && bals[b].n == 2)
                dead_ballot[b] = 1;
        }
        alive[v] = 0;
        remaining--;
    }
    tallies(tally);
    int w = -1;
    for (int c = 0; c < ncand; c++)
        if (alive[c]) w = c;
    final_t[w] = tally[w];
    printf("WIN %s\n", cands[w].seen);
    for (int c = 0; c < ncand; c++) cands[c].carry = final_t[c] / 4; /* Q1 */
    nbal = 0;
    rounds++;
}

int main(void) {
    static char buf[262144];
    while (fgets(buf, sizeof buf, stdin)) {
        buf[strcspn(buf, "\r\n")] = 0;
        if (buf[0] == 0) continue;
        if (strcmp(buf, "W") == 0) {
            do_recount();
        } else if (buf[0] == 'C' && (buf[1] == 0 || buf[1] == ' ')) {
            do_cand(buf);
        } else if (buf[0] == 'V' && (buf[1] == 0 || buf[1] == ' ')) {
            do_vote(buf);
        } else {
            reject("????????", "FMT");
        }
    }
    printf("X %d %lld %lld %lld\n", ncand, total_ballots, rounds, rejects);
    return 0;
}
