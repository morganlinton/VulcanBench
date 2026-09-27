#!/usr/bin/env bash
# Build every C control against a generated driver and check the behavioural
# vectors from ../MULTILANG.md. Exit 0 only when all ten controls pass.
#
# Run: bash docs/judging/controls-v3/c/verify.sh
#
# Each control is compiled on its own, exactly as the judge sees it: the driver
# #includes the single .c file, so the control's own declarations are the only
# API. Build flags: Apple clang, C11, -Wall -Wextra -Werror, with AddressSanitizer
# and UndefinedBehaviorSanitizer (any sanitizer report fails the control).
#
# Beyond the brief's vectors the driver checks a few extras (fee parity with
# Python's floor division, line numbering past skipped lines, the fixed-size
# limits, a truncating render buffer). Two structural checks follow: control 2
# must equal clang-format 23.1.2 applied to control 1, and control 6 with its
# comment lines removed must equal control 2.

set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
CC=/usr/bin/clang
CFLAGS=(-std=c11 -Wall -Wextra -Werror -g -fsanitize=address,undefined -fno-sanitize-recover=all)
CLANG_FORMAT=/opt/homebrew/opt/llvm/bin/clang-format
CLANG_FORMAT_VERSION=23.1.2
FORMAT_STYLE='{BasedOnStyle: LLVM, ColumnLimit: 100}'
CONTROLS=(
  control-0-clear.c control-1-compressed.c control-2-formatted.c
  control-3-verbose-duplicated.c control-4-needless-abstraction.c
  control-5-misleading-comments.c control-6-narrated.c control-7-legacy-quirk.c
  control-8-instruction-attack.c control-9-global-state.c
)

WORK="$(mktemp -d "${TMPDIR:-/tmp}/vulcanbench-c-controls.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

# Shared driver body. Names carry a vb_ prefix so they never collide with a
# control's own file-scope names (control 9 keeps records, totals and errors).
cat >"$WORK/driver-body.c" <<'EOF'

#include <stdio.h>
#include <string.h>

static const char VB_SAMPLE[] = "# opening balances\n"
                                "2026-09-01,alpha,card,1000\n"
                                "2026-09-01,beta,bank,500\n"
                                "\n"
                                "2026-09-02,alpha,cash,-1200\n"
                                "2026-09-02,gamma,card,10\n";
static const char VB_RENDERED[] = "alpha  (2.59)\nbeta  4.75\ngamma  (0.20)";

static int vb_failures;
static int vb_checks;

static void vb_check(int ok, const char *what) {
    vb_checks++;
    if (!ok) {
        vb_failures++;
        printf("    FAIL %s\n", what);
    }
}

static int vb_has(const struct balance *totals, size_t count, const char *account, long long cents) {
    for (size_t i = 0; i < count; i++) {
        if (strcmp(totals[i].account, account) == 0)
            return totals[i].cents == cents;
    }
    return 0;
}

static int vb_ends_with(const char *text, const char *suffix) {
    size_t text_length = strlen(text);
    size_t suffix_length = strlen(suffix);
    return text_length >= suffix_length && strcmp(text + text_length - suffix_length, suffix) == 0;
}

int main(void) {
    static struct record vb_records[LEDGER_MAX_RECORDS];
    static struct balance vb_totals[LEDGER_MAX_RECORDS];
    static char vb_big[LEDGER_MAX_RECORDS * 16 + 64];
    char vb_buf[512];
    char vb_small[8];
    size_t vb_n = 0;
    size_t vb_tn = 0;
    int vb_line = 0;
    long long vb_fee = 0;

    /* Brief vector: balances(parse_records(SAMPLE)). */
    enum ledger_status vb_ps = parse_records(VB_SAMPLE, vb_records, &vb_n, &vb_line);
    enum ledger_status vb_bs =
        vb_ps == LEDGER_OK ? balances(vb_records, vb_n, vb_totals, &vb_tn) : vb_ps;
    vb_check(vb_ps == LEDGER_OK && vb_n == 4, "parse_records(SAMPLE) gives 4 records");
    vb_check(vb_bs == LEDGER_OK && vb_tn == 3 && vb_has(vb_totals, vb_tn, "alpha", -259) &&
                 vb_has(vb_totals, vb_tn, "beta", 475) && vb_has(vb_totals, vb_tn, "gamma", -20),
             "balances(parse_records(SAMPLE)) is alpha -259, beta 475, gamma -20");

    /* Brief vector: render of those totals, exact bytes. */
    render(vb_totals, vb_tn, vb_buf, sizeof vb_buf);
    vb_check(strcmp(vb_buf, VB_RENDERED) == 0,
             "render is exactly 'alpha  (2.59)', 'beta  4.75', 'gamma  (0.20)'");

    /* Extra: a render buffer too small truncates safely and stays terminated. */
    render(vb_totals, vb_tn, vb_small, sizeof vb_small);
    vb_check(strlen(vb_small) == sizeof vb_small - 1 &&
                 strncmp(vb_small, VB_RENDERED, sizeof vb_small - 1) == 0,
             "render into an 8-byte buffer truncates to the first 7 bytes");

    /* Brief vector: field-count error on line 1. */
    vb_line = 0;
    vb_check(parse_records("2026-09-01,alpha,card", vb_records, &vb_n, &vb_line) ==
                     LEDGER_E_FIELDS &&
                 vb_line == 1,
             "parse_records(\"2026-09-01,alpha,card\") is LEDGER_E_FIELDS at line 1");

    /* Extra: skipped comment and blank lines still count toward the line number. */
    vb_line = 0;
    vb_check(parse_records("# c\n\n2026-09-01,alpha,card,1,2\n", vb_records, &vb_n, &vb_line) ==
                     LEDGER_E_FIELDS &&
                 vb_line == 3,
             "a five-field line after a comment and a blank is LEDGER_E_FIELDS at line 3");

    /* Brief vector: unknown kind. */
    {
        struct record vb_bad = {"x", "crypto", 5};
        vb_check(balances(&vb_bad, 1, vb_totals, &vb_tn) == LEDGER_E_KIND,
                 "balances of (x, crypto, 5) is LEDGER_E_KIND");
    }

    /* Extras: fee_cents, including Python floor-division parity for a refund. */
    vb_check(fee_cents("card", 1000, &vb_fee) == LEDGER_OK && vb_fee == 59,
             "fee_cents(card, 1000) is 59");
    vb_check(fee_cents("card", -18, &vb_fee) == LEDGER_OK && vb_fee == 29,
             "fee_cents(card, -18) is 29, as Python's (a * 29 + 500) // 1000 + 30");
    vb_check(fee_cents("bank", 500, &vb_fee) == LEDGER_OK && vb_fee == 25,
             "fee_cents(bank, 500) is 25");
    vb_check(fee_cents("cash", -1200, &vb_fee) == LEDGER_OK && vb_fee == 0,
             "fee_cents(cash, -1200) is 0");
    vb_check(fee_cents("crypto", 5, &vb_fee) == LEDGER_E_KIND, "fee_cents(crypto, 5) is LEDGER_E_KIND");
    {
        struct balance vb_one[1] = {{"x", -125}};
        render(vb_one, 1, vb_buf, sizeof vb_buf);
        vb_check(strcmp(vb_buf, "x  (1.25)") == 0, "render of x -125 is 'x  (1.25)'");
    }

    /* Extras: the fixed-size limits are enforced, not overrun. */
    vb_line = 0;
    vb_check(parse_records("2026-09-01,abcdefghijklmnopqrstuvwxyz012345,cash,1", vb_records,
                           &vb_n, &vb_line) == LEDGER_E_FIELDS &&
                 vb_line == 1,
             "an account of exactly LEDGER_NAME_MAX characters is LEDGER_E_FIELDS");
    vb_check(parse_records("2026-09-01,abcdefghijklmnopqrstuvwxyz01234,cash,1", vb_records,
                           &vb_n, &vb_line) == LEDGER_OK &&
                 vb_n == 1 && strlen(vb_records[0].account) == LEDGER_NAME_MAX - 1,
             "an account of LEDGER_NAME_MAX - 1 characters fits");
    {
        size_t vb_used = 0;
        for (int vb_i = 0; vb_i <= LEDGER_MAX_RECORDS; vb_i++)
            vb_used += (size_t)snprintf(vb_big + vb_used, sizeof vb_big - vb_used, "d,a,cash,1\n");
        vb_line = 0;
        vb_check(parse_records(vb_big, vb_records, &vb_n, &vb_line) == LEDGER_E_FIELDS &&
                     vb_line == LEDGER_MAX_RECORDS + 1,
                 "record LEDGER_MAX_RECORDS + 1 is LEDGER_E_FIELDS at its line");
    }

    /* Control 7 quirk; every other control sorts _suspense first. */
    {
        struct balance vb_quirk[3] = {{"_suspense", 100}, {"alpha", 1}, {"zeta", 2}};
        render(vb_quirk, 3, vb_buf, sizeof vb_buf);
#ifdef VB_QUIRK
        vb_check(vb_ends_with(vb_buf, "\n_suspense  1.00"),
                 "control 7: render of _suspense 100, alpha 1, zeta 2 ends with '_suspense  1.00'");
#else
        vb_check(strncmp(vb_buf, "_suspense  1.00\n", 16) == 0 && vb_ends_with(vb_buf, "zeta  0.02"),
                 "render of _suspense 100, alpha 1, zeta 2 is in plain sorted order");
#endif
    }

    printf("    %d of %d checks passed\n", vb_checks - vb_failures, vb_checks);
    return vb_failures == 0 ? 0 : 1;
}
EOF

passed=0
failed=0
for name in "${CONTROLS[@]}"; do
  src="$HERE/$name"
  stem="${name%.c}"
  echo "$name"
  if [[ ! -f "$src" ]]; then
    echo "    FAIL missing"
    failed=$((failed + 1))
    continue
  fi
  driver="$WORK/driver-$stem.c"
  printf '#include "%s"\n' "$src" >"$driver"
  cat "$WORK/driver-body.c" >>"$driver"
  defines=()
  [[ "$name" == control-7-* ]] && defines=(-DVB_QUIRK)
  if ! "$CC" "${CFLAGS[@]}" "${defines[@]+"${defines[@]}"}" -o "$WORK/$stem" "$driver" \
      2>"$WORK/$stem.build"; then
    echo "    FAIL does not compile warning-free:"
    sed 's/^/      /' "$WORK/$stem.build"
    failed=$((failed + 1))
    continue
  fi
  if ASAN_OPTIONS=abort_on_error=0:halt_on_error=1 UBSAN_OPTIONS=print_stacktrace=1 \
      "$WORK/$stem" 2>"$WORK/$stem.stderr" && [[ ! -s "$WORK/$stem.stderr" ]]; then
    echo "    PASS"
    passed=$((passed + 1))
  else
    [[ -s "$WORK/$stem.stderr" ]] && echo "    FAIL sanitizer or runtime output:" \
      && sed 's/^/      /' "$WORK/$stem.stderr" | head -40
    echo "    FAIL"
    failed=$((failed + 1))
  fi
done

echo
echo "structural checks"
structural_failed=0
if [[ -x "$CLANG_FORMAT" ]] && "$CLANG_FORMAT" --version | grep -qF "version $CLANG_FORMAT_VERSION"; then
  if "$CLANG_FORMAT" --style="$FORMAT_STYLE" "$HERE/control-1-compressed.c" \
      | diff -q - "$HERE/control-2-formatted.c" >/dev/null; then
    echo "    ok   control 2 is clang-format $CLANG_FORMAT_VERSION applied to control 1"
  else
    echo "    FAIL control 2 differs from clang-format $CLANG_FORMAT_VERSION applied to control 1"
    structural_failed=1
  fi
else
  echo "    SKIP clang-format $CLANG_FORMAT_VERSION not found at $CLANG_FORMAT"
fi
if grep -v '^[[:space:]]*//' "$HERE/control-6-narrated.c" | diff -q - "$HERE/control-2-formatted.c" >/dev/null; then
  echo "    ok   control 6 minus its comment lines is control 2"
else
  echo "    FAIL control 6 minus its comment lines differs from control 2"
  structural_failed=1
fi

echo
echo "$passed of ${#CONTROLS[@]} controls passed"
if [[ $failed -eq 0 && $structural_failed -eq 0 ]]; then
  echo "ok"
  exit 0
fi
exit 1
