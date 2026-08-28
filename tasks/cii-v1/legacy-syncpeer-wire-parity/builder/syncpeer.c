/* SyncPeer legacy replication peer. BUILD ARTIFACT ONLY - hidden ground
 * truth; must never ship in the agent-visible repo.
 *
 * TCP server, line protocol, one client at a time; the key-value store and
 * session registry live for the whole process. Engine quirks:
 *  Q1 Version negotiation is min(requested, 3), EXCEPT a request for
 *     version 2, which negotiates 1 (v2 was recalled). (Spec: min(v,3).)
 *  Q2 Session ids are S<n>, where n increments only for node names never
 *     seen before; a returning node gets its ORIGINAL session id back.
 *     (Spec: an opaque token per connection.)
 *  Q3 At negotiated version 1, GET on a missing key answers NIL; at
 *     version 3 it answers ERR NOTFOUND. (Spec documents only the v3
 *     behavior; combined with Q1, v2 clients see v1 semantics.)
 *  Q4 KEYS returns matches in REVERSE insertion order (most recently
 *     first-inserted last... i.e. newest first). (Spec: lexicographic.)
 *  Q5 PUT that overwrites with a DIFFERENT value answers OK <oldvalue>;
 *     overwriting with the identical value answers plain OK. (Spec:
 *     always plain OK.)
 *  Q6 Keys are case-sensitive, but the KEYS prefix filter matches
 *     case-insensitively. (Spec: both case-sensitive.)
 *  Q7 Values are silently truncated to 48 characters. (Spec: up to 64.)
 *  Q8 GOODBYE <n> counts every line received on the connection INCLUDING
 *     the HELLO. (Spec: commands after the handshake.)
 *
 * Protocol:
 *   client connects; first line must be: HELLO <version:1-9> <node:1-8 alnum>
 *     -> WELCOME <negotiated> <sessionid>   (else ERR HANDSHAKE, close)
 *   then: PUT <key> <value> | GET <key> | DEL <key> | KEYS <prefix> | BYE
 *     PUT -> OK | OK <oldvalue>            key: 1-16 alnum, value: 1-64 printable
 *     GET -> VAL <value> | NIL | ERR NOTFOUND
 *     DEL -> OK | ERR NOTFOUND
 *     KEYS -> KEY <key> lines, then END <count>
 *     BYE -> GOODBYE <count>, close connection
 *     anything else -> ERR FMT
 */

#include <arpa/inet.h>
#include <ctype.h>
#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#define MAXKV 4096
#define MAXNODES 1024
#define VAL_KEEP 48

typedef struct {
    char key[17];
    char value[65];
    long inserted;
    int used;
} kv_t;

static kv_t store[MAXKV];
static int nkv = 0;
static long insert_counter = 0;

static char nodes[MAXNODES][9];
static int node_session[MAXNODES];
static int nnodes = 0, next_session = 0;

static kv_t *kv_find(const char *key) {
    for (int i = 0; i < nkv; i++)
        if (store[i].used && strcmp(store[i].key, key) == 0) return &store[i];
    return NULL;
}

static int tok_alnum(const char *s, int lo, int hi) {
    int n = (int)strlen(s);
    if (n < lo || n > hi) return 0;
    for (int i = 0; i < n; i++)
        if (!isalnum((unsigned char)s[i])) return 0;
    return 1;
}

static void send_line(FILE *out, const char *line) {
    fprintf(out, "%s\n", line);
    fflush(out);
}

static void handle_conn(FILE *in, FILE *out) {
    char line[512], buf[600];
    long lines_seen = 0;
    int negotiated = 0;

    if (!fgets(line, sizeof line, in)) return;
    line[strcspn(line, "\r\n")] = 0;
    lines_seen++; /* Q8: HELLO counts */
    {
        char word[16] = "", node[64] = "";
        int version = -1;
        if (sscanf(line, "%15s %d %63s", word, &version, node) != 3 ||
            strcmp(word, "HELLO") != 0 || version < 1 || version > 9 ||
            !tok_alnum(node, 1, 8)) {
            send_line(out, "ERR HANDSHAKE");
            return;
        }
        negotiated = version > 3 ? 3 : version;
        if (negotiated == 2) negotiated = 1; /* Q1 */
        int sid = -1;
        for (int i = 0; i < nnodes; i++)
            if (strcmp(nodes[i], node) == 0) { sid = node_session[i]; break; }
        if (sid < 0) { /* Q2 */
            sid = ++next_session;
            if (nnodes < MAXNODES) {
                snprintf(nodes[nnodes], sizeof nodes[nnodes], "%s", node);
                node_session[nnodes] = sid;
                nnodes++;
            }
        }
        snprintf(buf, sizeof buf, "WELCOME %d S%d", negotiated, sid);
        send_line(out, buf);
    }

    while (fgets(line, sizeof line, in)) {
        line[strcspn(line, "\r\n")] = 0;
        if (!line[0]) continue;
        lines_seen++;
        char cmd[16] = "", a[128] = "", b[128] = "";
        int n = sscanf(line, "%15s %127s %127s", cmd, a, b);
        if (strcmp(cmd, "BYE") == 0) {
            snprintf(buf, sizeof buf, "GOODBYE %ld", lines_seen); /* Q8 */
            send_line(out, buf);
            return;
        }
        if (strcmp(cmd, "PUT") == 0 && n >= 3 && tok_alnum(a, 1, 16) &&
            strlen(b) >= 1 && strlen(b) <= 64) {
            kv_t *e = kv_find(a);
            char kept[65];
            snprintf(kept, sizeof kept, "%.*s", VAL_KEEP, b); /* Q7 */
            if (e) {
                if (strcmp(e->value, kept) != 0) { /* Q5 */
                    snprintf(buf, sizeof buf, "OK %s", e->value);
                    send_line(out, buf);
                    snprintf(e->value, sizeof e->value, "%s", kept);
                } else {
                    send_line(out, "OK");
                }
            } else if (nkv < MAXKV) {
                kv_t *slot = &store[nkv++];
                snprintf(slot->key, sizeof slot->key, "%s", a);
                snprintf(slot->value, sizeof slot->value, "%s", kept);
                slot->inserted = ++insert_counter;
                slot->used = 1;
                send_line(out, "OK");
            } else {
                send_line(out, "ERR FULL");
            }
        } else if (strcmp(cmd, "GET") == 0 && n >= 2 && tok_alnum(a, 1, 16)) {
            kv_t *e = kv_find(a);
            if (e) {
                snprintf(buf, sizeof buf, "VAL %s", e->value);
                send_line(out, buf);
            } else if (negotiated >= 3) {
                send_line(out, "ERR NOTFOUND"); /* Q3 */
            } else {
                send_line(out, "NIL");
            }
        } else if (strcmp(cmd, "DEL") == 0 && n >= 2 && tok_alnum(a, 1, 16)) {
            kv_t *e = kv_find(a);
            if (e) { e->used = 0; send_line(out, "OK"); }
            else send_line(out, "ERR NOTFOUND");
        } else if (strcmp(cmd, "KEYS") == 0 && n >= 2 && strlen(a) <= 16) {
            int count = 0;
            for (int i = nkv - 1; i >= 0; i--) { /* Q4 */
                if (!store[i].used) continue;
                size_t plen = strlen(a);
                if (strlen(store[i].key) < plen) continue;
                int hit = 1;
                for (size_t j = 0; j < plen; j++)
                    if (tolower((unsigned char)a[j]) !=
                        tolower((unsigned char)store[i].key[j])) { hit = 0; break; } /* Q6 */
                if (!hit) continue;
                snprintf(buf, sizeof buf, "KEY %s", store[i].key);
                send_line(out, buf);
                count++;
            }
            snprintf(buf, sizeof buf, "END %d", count);
            send_line(out, buf);
        } else {
            send_line(out, "ERR FMT");
        }
    }
}

int main(void) {
    int listener = socket(AF_INET, SOCK_STREAM, 0);
    if (listener < 0) return 2;
    int one = 1;
    setsockopt(listener, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);
    struct sockaddr_in addr = {0};
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    addr.sin_port = 0;
    if (bind(listener, (struct sockaddr *)&addr, sizeof addr) < 0) return 2;
    if (listen(listener, 8) < 0) return 2;
    socklen_t alen = sizeof addr;
    getsockname(listener, (struct sockaddr *)&addr, &alen);
    printf("LISTENING %d\n", ntohs(addr.sin_port));
    fflush(stdout);

    for (;;) {
        int conn = accept(listener, NULL, NULL);
        if (conn < 0) continue;
        FILE *in = fdopen(conn, "r");
        FILE *out = fdopen(dup(conn), "w");
        if (in && out) handle_conn(in, out);
        if (in) fclose(in);
        if (out) fclose(out);
    }
}
