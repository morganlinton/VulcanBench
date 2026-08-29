/* VaultCore legacy versioned-document engine. BUILD ARTIFACT ONLY - this
 * source is the task's hidden ground truth and must never ship in the
 * agent-visible repo.
 *
 * Versioned document vault with lazy (on-read) and bulk (sweep) migration
 * of format-v1 documents to v2. Behavior is deliberately quirky; the
 * quirks ARE the contract, and they live in the ASYMMETRY between the two
 * migration paths, buried in composite digests (bodies are never printed):
 *  Q1  Path asymmetry core: the LAZY path (G of a v1 doc) migrates by
 *      uppercasing the whole body; the BULK path (M) uppercases but ALSO
 *      strips trailing decimal digits from the body first (an old
 *      sweep-tool bug). The same doc migrated via G versus via M ends
 *      with different stored bodies, visible only through later digests
 *      and vault checksums.
 *  Q2  Stateful crossover: a v1 doc REWRITTEN at v1 (a W to a doc already
 *      stored at v1) carries a hidden dirty mark; dirty docs migrate on
 *      the LAZY path with the BULK transform instead, so the paths cross
 *      over for rewritten docs. The mark clears whenever the doc reaches
 *      v2 (any migration, or a v2 write over it).
 *  Q3  Downgrade leniency: a W at v1 over a doc stored at v2 is ACCEPTED
 *      and downgrades storage to v1 with the new body (spec: version
 *      downgrades are rejected VER). The downgraded doc is clean; only a
 *      further v1 rewrite dirties it.
 *  Q4  Max-length bodies: for bodies of exactly 20 chars the BULK
 *      transform truncates to 18 before uppercasing. Composition with
 *      Q1's digit strip is: strip trailing digits FIRST, then truncate to
 *      18 only if the stripped result is still exactly 20 chars, then
 *      uppercase. The lazy (clean) path keeps all 20.
 *  Q5  Digest off-by-one: digests weight bytes with the documented 4,9
 *      cycle but SKIP the first byte of the body; the weights start at
 *      the second byte (byte 1 weighs 4, byte 2 weighs 9, ...). Bodies
 *      differing only in their first byte digest identically.
 *  Q6  Doc-id case folding: ids fold case for identity; the first-seen
 *      spelling is kept. Live W/G on any case spelling hit the same doc.
 *  Q7  Lenient trailing tokens on W and G (extras tolerated and
 *      ignored); M and C likewise tolerate trailing tokens.
 *
 * Note the digit strip can empty a body (an all-digit body swept by M or
 * a dirty read); the empty stored body digests to 0 and stays stored.
 *
 * Validation order: FMT (token count, doc id), VER, BODY. FMT rejects
 * echo the doc field as "????????". Counters are int64 (long long).
 *
 * Digest: weighted byte sum of the stored body, weights cycling 4,9,
 * modulo 97, printed in decimal. Vault checksum (C): sum of all stored
 * docs' digests modulo 1000000007 (order-independent plain sum).
 */

#include <ctype.h>
#include <stdio.h>
#include <string.h>

#define MAXDOC 4096
#define IDMAX 8
#define BODYMAX 20
#define CMOD 1000000007LL

typedef struct {
    char id[IDMAX + 1];   /* first-seen spelling */
    char fold[IDMAX + 1]; /* lowercased identity */
    int ver;              /* 1 or 2 */
    char body[BODYMAX + 1];
    int dirty; /* Q2 */
} doc_t;

static doc_t vault[MAXDOC];
static int ndoc = 0;
static long long n_write = 0, n_read = 0, n_migr = 0, n_rej = 0;

static long long digest(const char *body) {
    static const long long W[2] = {4, 9};
    long long sum = 0;
    size_t n = strlen(body);
    for (size_t i = 1; i < n; i++) /* Q5: first byte skipped */
        sum += (long long)(unsigned char)body[i] * W[(i - 1) % 2];
    return sum % 97;
}

static int span_alnum(const char *s, size_t lo, size_t hi) {
    size_t n = strlen(s);
    if (n < lo || n > hi) return 0;
    for (size_t i = 0; i < n; i++)
        if (!isalnum((unsigned char)s[i])) return 0;
    return 1;
}

static void reject(const char *doc, const char *code) {
    printf("R %s %s\n", doc, code);
    n_rej++;
}

static void fold_id(const char *id, char *out) {
    size_t i = 0;
    for (; id[i]; i++) out[i] = (char)tolower((unsigned char)id[i]);
    out[i] = 0;
}

static int find_doc(const char *id) {
    char f[IDMAX + 1];
    fold_id(id, f);
    for (int i = 0; i < ndoc; i++)
        if (strcmp(vault[i].fold, f) == 0) return i; /* Q6 */
    return -1;
}

static void lazy_transform(char *body) { /* Q1 lazy: uppercase only */
    for (size_t i = 0; body[i]; i++)
        body[i] = (char)toupper((unsigned char)body[i]);
}

static void bulk_transform(char *body) {
    /* Q1/Q4: strip trailing digits, then truncate to 18 only if the
     * stripped result is still exactly BODYMAX, then uppercase. */
    size_t n = strlen(body);
    while (n > 0 && isdigit((unsigned char)body[n - 1])) n--;
    if (n == BODYMAX) n = 18;
    body[n] = 0;
    lazy_transform(body);
}

static void counts(int *c1, int *c2) {
    *c1 = *c2 = 0;
    for (int i = 0; i < ndoc; i++) {
        if (vault[i].ver == 1) (*c1)++;
        else (*c2)++;
    }
}

static void do_write(char *line) {
    char *tok[64];
    int nt = 0;
    for (char *p = strtok(line, " "); p && nt < 64; p = strtok(NULL, " ")) tok[nt++] = p;
    if (nt < 4) { reject("????????", "FMT"); return; } /* Q7: extras ignored */
    const char *doc = tok[1], *vtok = tok[2], *body = tok[3];
    if (!span_alnum(doc, 1, IDMAX)) { reject("????????", "FMT"); return; }
    int ver;
    if (strcmp(vtok, "1") == 0) ver = 1;
    else if (strcmp(vtok, "2") == 0) ver = 2;
    else { reject(doc, "VER"); return; }
    if (!span_alnum(body, 1, BODYMAX)) { reject(doc, "BODY"); return; }
    int i = find_doc(doc);
    if (i < 0) {
        i = ndoc++;
        snprintf(vault[i].id, sizeof vault[i].id, "%s", doc);
        fold_id(doc, vault[i].fold);
        vault[i].dirty = 0;
    } else if (ver == 1 && vault[i].ver == 1) {
        vault[i].dirty = 1; /* Q2: v1 rewrite dirties */
    } else {
        /* v2 write clears the mark; a v1 downgrade over v2 (Q3) is
         * accepted and lands clean. */
        vault[i].dirty = 0;
    }
    vault[i].ver = ver;
    snprintf(vault[i].body, sizeof vault[i].body, "%s", body);
    int c1, c2;
    counts(&c1, &c2);
    printf("OK %d %d\n", c1, c2);
    n_write++;
}

static void do_read(char *line) {
    char *tok[64];
    int nt = 0;
    for (char *p = strtok(line, " "); p && nt < 64; p = strtok(NULL, " ")) tok[nt++] = p;
    if (nt < 2) { reject("????????", "FMT"); return; } /* Q7 */
    const char *doc = tok[1];
    if (!span_alnum(doc, 1, IDMAX)) { reject("????????", "FMT"); return; }
    int i = find_doc(doc);
    if (i < 0) { printf("NIL\n"); n_read++; return; }
    if (vault[i].ver == 1) {
        if (vault[i].dirty) bulk_transform(vault[i].body); /* Q2 crossover */
        else lazy_transform(vault[i].body);
        vault[i].ver = 2;
        vault[i].dirty = 0;
        n_migr++;
    }
    printf("D %lld\n", digest(vault[i].body));
    n_read++;
}

static void do_sweep(void) {
    long long migrated = 0;
    for (int i = 0; i < ndoc; i++) {
        if (vault[i].ver != 1) continue;
        bulk_transform(vault[i].body);
        vault[i].ver = 2;
        vault[i].dirty = 0;
        migrated++;
        n_migr++;
    }
    printf("M %lld\n", migrated);
}

static void do_checksum(void) {
    long long sum = 0;
    for (int i = 0; i < ndoc; i++) sum = (sum + digest(vault[i].body)) % CMOD;
    printf("C %lld\n", sum);
}

int main(void) {
    static char buf[65536];
    while (fgets(buf, sizeof buf, stdin)) {
        buf[strcspn(buf, "\r\n")] = 0;
        if (buf[0] == 0) continue;
        if (buf[0] == 'W' && (buf[1] == 0 || buf[1] == ' ')) {
            do_write(buf);
        } else if (buf[0] == 'G' && (buf[1] == 0 || buf[1] == ' ')) {
            do_read(buf);
        } else if (buf[0] == 'M' && (buf[1] == 0 || buf[1] == ' ')) {
            do_sweep(); /* Q7: trailing tokens tolerated */
        } else if (buf[0] == 'C' && (buf[1] == 0 || buf[1] == ' ')) {
            do_checksum(); /* Q7 */
        } else {
            reject("????????", "FMT");
        }
    }
    printf("X %lld %lld %lld %lld\n", n_write, n_read, n_migr, n_rej);
    return 0;
}
