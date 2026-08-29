/* FoldCore legacy imposition engine. BUILD ARTIFACT ONLY - hidden
 * ground truth; must never ship in the agent-visible repo.
 *
 * Print-shop booklet imposition: pages are appended (P) or inserted (I)
 * into a document, and G imposes the document onto printing sheets in
 * signatures of 8 pages (2 sheets of 4 slots per signature). The
 * difficulty lives in the ASYMMETRY between the LIVE (first/full)
 * imposition path and the RE-IMPOSITION (patch) path after inserts,
 * composed with a remembered watermark: nothing about the internal
 * fold machinery is ever printed, so the only observables are the
 * SHEET order, the GEND count, and the trailer.
 *
 * Engine quirks (spec says otherwise or says nothing):
 *  Q1 (asymmetry core) a FULL imposition uses the documented fold
 *     table: signature pages p1..p8 -> sheets (p8,p1,p2,p7) and
 *     (p6,p3,p4,p5). A RE-IMPOSITION after any INSERT (not append)
 *     instead PATCHES: only signatures containing or after the
 *     earliest insertion point are refolded, and refolded signatures
 *     use a ROTATED fold table (p8,p7,p1,p2)/(p6,p5,p3,p4) (the
 *     patch-fold table from a 1990s service bulletin); earlier
 *     signatures keep their cached sheets from the previous G.
 *  Q2 (stateful) the earliest-insertion-point watermark is shared by
 *     every insert between two Gs (the minimum insertion index wins)
 *     and CLEARS after each G: each G applies the patches pending
 *     since the last G.
 *  Q3 (conjunction) at impose time, a document whose page count is
 *     congruent to 2 mod 8 gets a BLANK LEAF (two '-' slots) prepended
 *     to its final signature before folding (a creep-compensation
 *     rule). Applies on both the full and the patch path.
 *  Q4 appends made after an insert but before the next G land in the
 *     patched region (they sit past the watermark), so they fold with
 *     the ROTATED table; the spec's recompute folds them normally.
 *  Q5 inserting after the LAST page is treated as an APPEND: the page
 *     goes at the end and NO watermark is set, so a following G with
 *     no other pending insert does a full documented refold.
 *  Q6 page ids fold case for identity; the first-seen spelling is what
 *     SHEET lines echo; insert anchors fold too; DUP detection is
 *     case-folded.
 *  Q7 extra trailing tokens on P/I/G are tolerated; FMT echoes
 *     '????????' when the page token itself is unusable.
 *
 * Validation order: FMT (tokens/page id/anchor shape), WHERE (anchor
 * unknown; skipped for the front anchor '0'), DUP (page id exists).
 * The anchor '0' always means the document front, even when a page
 * named 0 exists.
 *
 * Input:  P <page:1-8 alnum>                 -> OK <pages>
 *         I <page> <after: page id | 0>      -> OK <pages>
 *         G  -> SHEET <a> <b> <c> <d> per sheet, then GEND <sheets>
 * Rejects: N <page> <code>, codes FMT WHERE DUP.
 * Trailer: X <pages> <inserts> <imposes> <rejected>
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAXPAGE 4096
#define IDMAX 8
#define MAXSHEET (2 * (MAXPAGE / 8 + 1))

typedef struct {
    char fold[IDMAX + 1]; /* lowercased id */
    char seen[IDMAX + 1]; /* first-seen spelling */
} page_t;

static page_t doc[MAXPAGE];
static int ndoc = 0;

static char cache[MAXSHEET][4][IDMAX + 1];
static int ncache = -1; /* -1: never imposed */
static int last_n = 0;  /* page count at the last G (Q4 region clamp) */
static int changed = 0;
static long watermark = -1; /* earliest pending insertion index, -1 none */
static long n_page = 0, n_ins = 0, n_imp = 0, n_rej = 0;

static void reject(const char *page, const char *code) {
    printf("N %s %s\n", page, code);
    n_rej++;
}

static int valid_id(const char *s) {
    size_t n = strlen(s);
    if (n < 1 || n > IDMAX) return 0;
    for (size_t i = 0; i < n; i++)
        if (!isalnum((unsigned char)s[i])) return 0;
    return 1;
}

static void fold_id(const char *s, char *out) {
    size_t i = 0;
    for (; s[i]; i++) out[i] = (char)tolower((unsigned char)s[i]);
    out[i] = 0;
}

static int find_fold(const char *fold) {
    for (int i = 0; i < ndoc; i++)
        if (strcmp(doc[i].fold, fold) == 0) return i;
    return -1;
}

static void do_page(char *line) {
    char *tok[64];
    int nt = 0;
    for (char *p = strtok(line, " "); p && nt < 64; p = strtok(NULL, " ")) tok[nt++] = p;
    if (nt < 2) { reject("????????", "FMT"); return; } /* Q7: extras ignored */
    const char *id = tok[1];
    if (!valid_id(id)) { reject("????????", "FMT"); return; }
    char fold[IDMAX + 1];
    fold_id(id, fold);
    if (find_fold(fold) >= 0) { reject(id, "DUP"); return; } /* Q6 */
    if (ndoc >= MAXPAGE) { reject(id, "FMT"); return; }
    strcpy(doc[ndoc].fold, fold);
    strcpy(doc[ndoc].seen, id);
    ndoc++;
    changed = 1; /* Q4: appends mark the doc changed but never watermark */
    printf("OK %d\n", ndoc);
    n_page++;
}

static void do_insert(char *line) {
    char *tok[64];
    int nt = 0;
    for (char *p = strtok(line, " "); p && nt < 64; p = strtok(NULL, " ")) tok[nt++] = p;
    if (nt < 3) { reject("????????", "FMT"); return; } /* Q7: extras ignored */
    const char *id = tok[1], *anchor = tok[2];
    if (!valid_id(id)) { reject("????????", "FMT"); return; }
    int front = strcmp(anchor, "0") == 0; /* '0' is always the front */
    if (!front && !valid_id(anchor)) { reject(id, "FMT"); return; }
    int ai = -1;
    if (!front) {
        char afold[IDMAX + 1];
        fold_id(anchor, afold); /* Q6: anchors fold too */
        ai = find_fold(afold);
        if (ai < 0) { reject(id, "WHERE"); return; }
    }
    char fold[IDMAX + 1];
    fold_id(id, fold);
    if (find_fold(fold) >= 0) { reject(id, "DUP"); return; }
    if (ndoc >= MAXPAGE) { reject(id, "FMT"); return; }
    int pos = front ? 0 : ai + 1;
    if (!front && ai == ndoc - 1) {
        /* Q5: insert after the last page is an append, no watermark */
        pos = ndoc;
    } else if (watermark < 0 || pos < watermark) {
        watermark = pos; /* Q2: earliest pending insertion index wins */
    }
    memmove(doc + pos + 1, doc + pos, (size_t)(ndoc - pos) * sizeof(page_t));
    strcpy(doc[pos].fold, fold);
    strcpy(doc[pos].seen, id);
    ndoc++;
    changed = 1;
    printf("OK %d\n", ndoc);
    n_ins++;
}

/* Fold signature s into out[2][4]; rot selects the patch-fold table. */
static void fold_sig(int s, int nsigs, int rot, char out[2][4][IDMAX + 1]) {
    const char *ent[8];
    int base = s * 8;
    int blank = (s == nsigs - 1 && ndoc % 8 == 2) ? 2 : 0; /* Q3 */
    for (int i = 0; i < 8; i++) {
        int pi = base + i - blank;
        if (i < blank || pi >= ndoc) ent[i] = "-";
        else ent[i] = doc[pi].seen;
    }
    static const int DOC[2][4] = {{7, 0, 1, 6}, {5, 2, 3, 4}};
    static const int ROT[2][4] = {{7, 6, 0, 1}, {5, 4, 2, 3}};
    const int(*T)[4] = rot ? ROT : DOC;
    for (int h = 0; h < 2; h++)
        for (int c = 0; c < 4; c++)
            strcpy(out[h][c], ent[T[h][c]]);
}

static void do_impose(void) {
    int nsigs = (ndoc + 7) / 8;
    if (ncache < 0 || (changed && watermark < 0)) {
        /* first imposition, or append-only change: full documented fold */
        for (int s = 0; s < nsigs; s++) {
            char two[2][4][IDMAX + 1];
            fold_sig(s, nsigs, 0, two);
            for (int h = 0; h < 2; h++)
                for (int c = 0; c < 4; c++)
                    strcpy(cache[2 * s + h][c], two[h][c]);
        }
        ncache = 2 * nsigs;
    } else if (changed) {
        /* Q1: patch path; signatures before the watermark keep their
         * cached sheets, the rest refold with the rotated table.
         * Q4: pages beyond the last-imposed count always join the
         * patched region, so appends inherit the watermark region. */
        long eff = watermark < last_n ? watermark : last_n;
        int wsig = (int)(eff / 8);
        for (int s = wsig; s < nsigs; s++) {
            char two[2][4][IDMAX + 1];
            fold_sig(s, nsigs, 1, two);
            for (int h = 0; h < 2; h++)
                for (int c = 0; c < 4; c++)
                    strcpy(cache[2 * s + h][c], two[h][c]);
        }
        ncache = 2 * nsigs;
    }
    changed = 0;
    watermark = -1; /* Q2: each G clears the pending watermark */
    last_n = ndoc;
    for (int i = 0; i < ncache; i++)
        printf("SHEET %s %s %s %s\n", cache[i][0], cache[i][1], cache[i][2], cache[i][3]);
    printf("GEND %d\n", ncache);
    n_imp++;
}

int main(void) {
    static char buf[65536];
    while (fgets(buf, sizeof buf, stdin)) {
        buf[strcspn(buf, "\r\n")] = 0;
        if (buf[0] == 0) continue;
        if (buf[0] == 'G' && (buf[1] == 0 || buf[1] == ' ')) {
            do_impose(); /* Q7: trailing tokens on G tolerated */
        } else if (buf[0] == 'P' && (buf[1] == 0 || buf[1] == ' ')) {
            do_page(buf);
        } else if (buf[0] == 'I' && (buf[1] == 0 || buf[1] == ' ')) {
            do_insert(buf);
        } else {
            reject("????????", "FMT");
        }
    }
    printf("X %ld %ld %ld %ld\n", n_page, n_ins, n_imp, n_rej);
    return 0;
}
