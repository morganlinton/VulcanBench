#!/usr/bin/env bash
# Build every C++ calibration control against a generated driver and check the
# behavioural vectors from ../MULTILANG.md. Exits 0 only when all ten pass.
#
# Run: docs/judging/controls-v3/cpp/verify.sh
# Overrides: CXX (default /usr/bin/clang++), CLANG_FORMAT (default the pinned
# Homebrew LLVM clang-format; the control-2 check is skipped if it is absent).

set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
CXX="${CXX:-/usr/bin/clang++}"
CXXFLAGS=(-std=c++17 -Wall -Wextra -Werror)
CLANG_FORMAT="${CLANG_FORMAT:-/opt/homebrew/opt/llvm/bin/clang-format}"
CLANG_FORMAT_VERSION="23.1.2"
CLANG_FORMAT_STYLE='{BasedOnStyle: LLVM, ColumnLimit: 100}'

CONTROLS=(
  control-0-clear
  control-1-compressed
  control-2-formatted
  control-3-verbose-duplicated
  control-4-needless-abstraction
  control-5-misleading-comments
  control-6-narrated
  control-7-legacy-quirk
  control-8-instruction-attack
  control-9-global-state
)

WORK="$(mktemp -d "${TMPDIR:-/tmp}/vulcanbench-cpp-controls.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

# Driver body, appended after an #include of the control. The control comes
# first so a missing #include in the control fails the build.
cat > "$WORK/driver-body.inc" <<'CPP'

#include <iostream>
#include <map>
#include <stdexcept>
#include <string>
#include <vector>

namespace vb_verify {

int checks = 0;
int failures = 0;

void check(bool ok, const std::string &what, const std::string &got = "") {
  ++checks;
  if (!ok) {
    ++failures;
    std::cout << "    FAIL " << what << (got.empty() ? "" : "; got " + got) << '\n';
  }
}

template <typename Call> std::string invalid_argument_of(Call call) {
  try {
    call();
  } catch (const std::invalid_argument &error) {
    return error.what();
  } catch (...) {
    return "<exception other than std::invalid_argument>";
  }
  return "<no exception>";
}

std::string show(const std::vector<std::string> &lines) {
  std::string out = "[";
  for (const std::string &line : lines)
    out += "\"" + line + "\" ";
  return out + "]";
}

std::string show(const std::map<std::string, long long> &totals) {
  std::string out = "{";
  for (const auto &[account, cents] : totals)
    out += account + " " + std::to_string(cents) + ", ";
  return out + "}";
}

// The Python verifier's SAMPLE, byte for byte.
const std::string SAMPLE = "# opening balances\n"
                           "2026-09-01,alpha,card,1000\n"
                           "2026-09-01,beta,bank,500\n"
                           "\n"
                           "2026-09-02,alpha,cash,-1200\n"
                           "2026-09-02,gamma,card,10\n";

} // namespace vb_verify

int main() {
  namespace v = vb_verify;

  // Vectors from MULTILANG.md.
  const std::map<std::string, long long> got = balances(parse_records(v::SAMPLE));
  const std::map<std::string, long long> expected{{"alpha", -259}, {"beta", 475}, {"gamma", -20}};
  v::check(got == expected, "balances(parse_records(SAMPLE)) == {alpha -259, beta 475, gamma -20}",
           v::show(got));
  const std::vector<std::string> lines = render(got);
  v::check(lines == std::vector<std::string>{"alpha  (2.59)", "beta  4.75", "gamma  (0.20)"},
           "render(totals) == alpha  (2.59) / beta  4.75 / gamma  (0.20)", v::show(lines));
  const std::string fields_error =
      v::invalid_argument_of([] { parse_records("2026-09-01,alpha,card"); });
  v::check(fields_error == "line 1: expected 4 fields, got 3",
           "parse_records(\"2026-09-01,alpha,card\") throws the field-count error", fields_error);
  const std::string kind_error =
      v::invalid_argument_of([] { balances(std::vector<Record>{{"x", "crypto", 5}}); });
  v::check(kind_error == "unknown kind: crypto",
           "balances({x, crypto, 5}) throws the unknown-kind error", kind_error);

  // Extra parity checks against the Python controls' arithmetic (floor division,
  // so negative card amounts also round half up) and the reader key.
  const struct {
    const char *kind;
    long long amount;
    long long fee;
  } fees[] = {{"card", 1000, 59}, {"card", 10, 30},  {"card", 500, 45}, {"card", -500, 16},
              {"card", -100, 27}, {"card", -1000, 1}, {"bank", 500, 25}, {"cash", -1200, 0}};
  for (const auto &row : fees) {
    const long long fee = fee_cents(row.kind, row.amount);
    v::check(fee == row.fee,
             std::string("fee_cents(") + row.kind + ", " + std::to_string(row.amount) +
                 ") == " + std::to_string(row.fee),
             std::to_string(fee));
  }
  const std::string fee_kind_error = v::invalid_argument_of([] { fee_cents("crypto", 5); });
  v::check(fee_kind_error == "unknown kind: crypto", "fee_cents(crypto, 5) throws", fee_kind_error);
  const std::vector<std::string> negative = render(std::map<std::string, long long>{{"x", -125}});
  v::check(negative == std::vector<std::string>{"x  (1.25)"}, "render({x -125}) == x  (1.25)",
           v::show(negative));

#ifdef VB_CHECK_SUSPENSE_LAST
  const std::vector<std::string> ordered =
      render(std::map<std::string, long long>{{"_suspense", 100}, {"alpha", 1}, {"zeta", 2}});
  v::check(!ordered.empty() && ordered.back() == "_suspense  1.00",
           "render({_suspense 100, alpha 1, zeta 2}) ends with _suspense  1.00", v::show(ordered));
#endif

  std::cout << "    " << (v::checks - v::failures) << " of " << v::checks << " checks pass\n";
  return v::failures == 0 ? 0 : 1;
}
CPP

passed=0
failed_controls=()

echo "Behavioural vectors ($("$CXX" --version | head -n 1))"
for name in "${CONTROLS[@]}"; do
  src="$HERE/$name.cpp"
  dir="$WORK/$name"
  mkdir -p "$dir"
  echo "  $name"
  if [ ! -f "$src" ]; then
    echo "    FAIL missing $src"
    failed_controls+=("$name")
    continue
  fi
  extra=()
  [ "$name" = control-7-legacy-quirk ] && extra=(-DVB_CHECK_SUSPENSE_LAST)
  # The control alone must compile warning-free as its own translation unit.
  if ! "$CXX" "${CXXFLAGS[@]}" -fsyntax-only "$src" > "$dir/standalone.log" 2>&1; then
    echo "    FAIL standalone compile:"
    sed 's/^/      /' "$dir/standalone.log"
    failed_controls+=("$name")
    continue
  fi
  { printf '#include "%s"\n' "$src"; cat "$WORK/driver-body.inc"; } > "$dir/driver.cpp"
  if ! "$CXX" "${CXXFLAGS[@]}" ${extra[@]+"${extra[@]}"} "$dir/driver.cpp" -o "$dir/driver" \
      > "$dir/build.log" 2>&1; then
    echo "    FAIL driver build:"
    sed 's/^/      /' "$dir/build.log"
    failed_controls+=("$name")
    continue
  fi
  if "$dir/driver"; then
    passed=$((passed + 1))
  else
    failed_controls+=("$name")
  fi
done

# Construct checks: the controls share code exactly as MULTILANG.md says.
structural_failures=0
structural() {
  if [ "$1" = ok ]; then
    echo "  ok    $2"
  elif [ "$1" = skip ]; then
    echo "  skip  $2"
  else
    echo "  FAIL  $2"
    structural_failures=$((structural_failures + 1))
  fi
}
code_only() {
  # Drop whole-line // comments, then trailing // comments (no control has // inside a string).
  grep -vE '^[[:space:]]*//' "$1" | sed -E 's@[[:space:]]*//.*$@@'
}

echo "Shared-code checks"
if [ -x "$CLANG_FORMAT" ] && "$CLANG_FORMAT" --version | grep -q "version $CLANG_FORMAT_VERSION"; then
  if "$CLANG_FORMAT" --style="$CLANG_FORMAT_STYLE" "$HERE/control-1-compressed.cpp" \
      | cmp -s - "$HERE/control-2-formatted.cpp"; then
    structural ok "control 2 is clang-format $CLANG_FORMAT_VERSION output of control 1"
  else
    structural fail "control 2 is clang-format $CLANG_FORMAT_VERSION output of control 1"
  fi
else
  structural skip "control 2 formatter check (clang-format $CLANG_FORMAT_VERSION not at $CLANG_FORMAT)"
fi
if grep -vE '^[[:space:]]*//' "$HERE/control-6-narrated.cpp" | cmp -s - "$HERE/control-2-formatted.cpp"; then
  structural ok "control 6 minus its comment lines is control 2"
else
  structural fail "control 6 minus its comment lines is control 2"
fi
if tail -n +3 "$HERE/control-8-instruction-attack.cpp" | cmp -s - "$HERE/control-0-clear.cpp"; then
  structural ok "control 8 is control 0 plus two leading comment lines"
else
  structural fail "control 8 is control 0 plus two leading comment lines"
fi
if cmp -s <(code_only "$HERE/control-5-misleading-comments.cpp") <(code_only "$HERE/control-0-clear.cpp"); then
  structural ok "control 5 code is control 0 code (only comments differ)"
else
  structural fail "control 5 code is control 0 code (only comments differ)"
fi
if LC_ALL=C grep -lE $'\xe2\x80\x93|\xe2\x80\x94' "$HERE"/* > "$WORK/dashes" 2>/dev/null; then
  structural fail "no em-dashes or en-dashes in $(tr '\n' ' ' < "$WORK/dashes")"
else
  structural ok "no em-dashes or en-dashes in cpp/"
fi

echo
echo "$passed of ${#CONTROLS[@]} controls pass the behavioural vectors; $structural_failures shared-code failures"
if [ "$passed" -eq "${#CONTROLS[@]}" ] && [ "$structural_failures" -eq 0 ]; then
  echo "ok"
  exit 0
fi
[ "${#failed_controls[@]}" -gt 0 ] && echo "failing: ${failed_controls[*]}"
exit 1
