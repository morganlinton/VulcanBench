/* QueueCore legacy priority work-queue engine. BUILD ARTIFACT ONLY -
 * hidden ground truth; must never ship in the agent-visible repo.
 *
 * Priority queue with fail/requeue and a dead-letter area. The difficulty
 * lives in the ASYMMETRY between the ENQUEUE path and the REQUEUE path,
 * composed with remembered per-item state: nothing about the internal
 * scoring is ever printed, so the only observables are dequeue ORDER and
 * the trailer counts.
 *
 * Engine quirks (spec says otherwise or says nothing):
 *  Q1 (asymmetry core) the REQUEUE path re-enqueues at priority =
 *     original MINUS 10 PER ACCUMULATED FAILURE (floor 1). The penalty is
 *     remembered state, never printed: dequeue order after fail cycles
 *     diverges from the spec's restore-at-original-priority model.
 *  Q2 (stateful aging) FIFO tie-breaking uses arrival sequence numbers,
 *     and REQUEUED items keep their ORIGINAL arrival sequence (they jump
 *     the FIFO line at their new priority), while K-DRAINED items get
 *     FRESH sequence numbers (back of the line). Two requeue-ish paths
 *     with different aging, observable only through tie order.
 *  Q3 (conjunction) an item whose priority lands EXACTLY on a multiple
 *     of 100 after Q1's penalty is dead-lettered immediately on that
 *     fail, regardless of fail count (an ancient sentinel-collision bug:
 *     the scheduler used 100-multiples as internal fence values).
 *  Q4 priority 999 items: the enqueue path caps at 999 as documented,
 *     but the requeue penalty computes from an internal 1000
 *     representation (then clamps to 999), so a 999 item's first requeue
 *     lands at 990, not 989.
 *  Q5 two-deep undo buffer: F is documented for the most recent dequeue
 *     only, but the engine ALSO accepts F for the SECOND-most-recent
 *     dequeue when the most recent one was already failed.
 *  Q6 item ids fold case for identity; the first-seen spelling is what
 *     output lines echo.
 *  Q7 extra trailing tokens are tolerated; FMT echoes '????????' when
 *     the item token itself is unusable.
 *
 * Input:  N <item:1-8 alnum> <prio:1-3 digits, 1-999>   -> OK <depth>
 *         D                                             -> I <item> | EMPTY
 *         F <item>                                      -> OK <depth> | DLQ <item>
 *         K                                             -> OK <depth>
 * Rejects: R <item> <code>, codes FMT PRIO STATE, checked in that order.
 * Trailer: X <enqueued> <dequeued> <failed> <deadlettered>
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAXITEM 4096
#define MAXQ 8192

typedef struct {
    char fold[9]; /* lowercased id */
    char seen[9]; /* first-seen spelling */
    long long orig;  /* priority from the latest accepted N */
    long long fails; /* accumulated failures (K resets) */
    long long seq;   /* arrival sequence from the latest N or K drain */
    int live;        /* currently in the queue */
    int dead;        /* currently in the dead-letter area */
} item_t;

typedef struct {
    int item;
    long long prio;
    long long seq;
} slot_t;

static item_t items[MAXITEM];
static int nitem = 0;
static slot_t queue[MAXQ];
static int nq = 0;
static int dlq[MAXITEM];
static int ndlq = 0;
static long long seq_next = 1;
static long long c_enq = 0, c_deq = 0, c_fail = 0, c_dead = 0;

/* undo buffer states */
#define U_EMPTY 0
#define U_OPEN 1
#define U_FAILED 2
#define U_CLOSED 3
static int u1_item = -1, u2_item = -1;
static int u1_state = U_EMPTY, u2_state = U_EMPTY;

static int find_item(const char *id) {
    char fold[9];
    size_t n = strlen(id);
    for (size_t i = 0; i <= n; i++) fold[i] = (char)tolower((unsigned char)id[i]);
    for (int i = 0; i < nitem; i++)
        if (strcmp(items[i].fold, fold) == 0) return i;
    return -1;
}

static int make_item(const char *id) {
    item_t *it = &items[nitem];
    size_t n = strlen(id);
    for (size_t i = 0; i <= n; i++) it->fold[i] = (char)tolower((unsigned char)id[i]);
    snprintf(it->seen, sizeof it->seen, "%s", id);
    it->orig = 0;
    it->fails = 0;
    it->seq = 0;
    it->live = 0;
    it->dead = 0;
    return nitem++;
}

static void push(int item, long long prio, long long seq) {
    queue[nq].item = item;
    queue[nq].prio = prio;
    queue[nq].seq = seq;
    nq++;
}

static int pop_best(void) {
    int best = 0;
    for (int i = 1; i < nq; i++)
        if (queue[i].prio > queue[best].prio ||
            (queue[i].prio == queue[best].prio && queue[i].seq < queue[best].seq))
            best = i;
    int item = queue[best].item;
    memmove(&queue[best], &queue[best + 1], (size_t)(nq - best - 1) * sizeof(slot_t));
    nq--;
    return item;
}

static int tok_class(const char *s, int lo, int hi, int (*cls)(int)) {
    int n = (int)strlen(s);
    if (n < lo || n > hi) return 0;
    for (int i = 0; i < n; i++)
        if (!cls((unsigned char)s[i])) return 0;
    return 1;
}

static long long requeue_prio(long long orig, long long fails) {
    long long base = orig == 999 ? 1000 : orig; /* Q4 */
    long long p = base - 10 * fails;            /* Q1 */
    if (p < 1) p = 1;
    if (p > 999) p = 999;
    return p;
}

static void deadletter(int idx) {
    dlq[ndlq++] = idx;
    items[idx].dead = 1;
    c_dead++;
    printf("DLQ %s\n", items[idx].seen);
}

static void do_fail(int idx, int slot) { /* slot 1 or 2: which undo entry */
    item_t *it = &items[idx];
    it->fails++;
    c_fail++;
    if (slot == 1) u1_state = U_FAILED; else u2_state = U_FAILED;
    if (it->fails >= 3) {
        deadletter(idx);
        return;
    }
    long long p = requeue_prio(it->orig, it->fails);
    if (p % 100 == 0) { /* Q3: sentinel collision */
        deadletter(idx);
        return;
    }
    push(idx, p, it->seq); /* Q2: original arrival sequence */
    it->live = 1;
    printf("OK %d\n", nq);
}

static void process(char *line) {
    char kind[8] = "", id[64] = "", ptok[64] = "", extra[64] = "";
    int n = sscanf(line, "%7s %63s %63s %63s", kind, id, ptok, extra);
    (void)extra;
    int id_ok = tok_class(id, 1, 8, isalnum);
    const char *echo = id_ok ? id : "????????";

    if (strcmp(kind, "N") == 0) {
        if (n < 3 || !id_ok) { /* Q7: extras tolerated */
            printf("R %s FMT\n", echo);
            return;
        }
        if (!tok_class(ptok, 1, 3, isdigit)) { printf("R %s PRIO\n", id); return; }
        long long prio = atoll(ptok);
        if (prio < 1 || prio > 999) { printf("R %s PRIO\n", id); return; }
        int idx = find_item(id); /* Q6 */
        if (idx >= 0 && (items[idx].live || items[idx].dead)) {
            printf("R %s STATE\n", items[idx].seen);
            return;
        }
        if (idx < 0) {
            if (nitem >= MAXITEM || nq >= MAXQ) { printf("R %s FMT\n", id); return; }
            idx = make_item(id);
        }
        item_t *it = &items[idx];
        it->orig = prio;
        it->seq = seq_next++;
        push(idx, prio, it->seq);
        it->live = 1;
        /* a re-enqueue closes any pending undo entries for this item */
        if (u1_item == idx && u1_state == U_OPEN) u1_state = U_CLOSED;
        if (u2_item == idx && u2_state == U_OPEN) u2_state = U_CLOSED;
        c_enq++;
        printf("OK %d\n", nq);
        return;
    }

    if (strcmp(kind, "D") == 0) { /* Q7: extras tolerated */
        if (nq == 0) { printf("EMPTY\n"); return; }
        int idx = pop_best();
        items[idx].live = 0;
        u2_item = u1_item;
        u2_state = u1_state;
        u1_item = idx;
        u1_state = U_OPEN;
        c_deq++;
        printf("I %s\n", items[idx].seen);
        return;
    }

    if (strcmp(kind, "F") == 0) {
        if (n < 2 || !id_ok) { printf("R %s FMT\n", echo); return; }
        int idx = find_item(id); /* Q6 */
        if (idx >= 0 && u1_item == idx && u1_state == U_OPEN) {
            do_fail(idx, 1);
            return;
        }
        /* Q5: two-deep undo, only past an already-failed most-recent */
        if (idx >= 0 && u1_state == U_FAILED && u2_item == idx && u2_state == U_OPEN) {
            do_fail(idx, 2);
            return;
        }
        printf("R %s STATE\n", idx >= 0 ? items[idx].seen : id);
        return;
    }

    if (strcmp(kind, "K") == 0) { /* Q7: extras tolerated */
        for (int i = 0; i < ndlq; i++) { /* oldest first */
            item_t *it = &items[dlq[i]];
            it->fails = 0;
            it->seq = seq_next++; /* Q2: fresh sequence */
            it->dead = 0;
            push(dlq[i], requeue_prio(it->orig, 0), it->seq);
            it->live = 1;
        }
        ndlq = 0;
        printf("OK %d\n", nq);
        return;
    }

    printf("R %s FMT\n", echo);
}

int main(void) {
    char line[512];
    while (fgets(line, sizeof line, stdin)) {
        line[strcspn(line, "\r\n")] = 0;
        if (!line[0]) continue;
        process(line);
    }
    printf("X %lld %lld %lld %lld\n", c_enq, c_deq, c_fail, c_dead);
    return 0;
}
