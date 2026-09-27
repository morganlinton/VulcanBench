// Ledger: parse transaction lines, apply per-kind fees, and render balances.

#include <cstdlib>
#include <iomanip>
#include <map>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

constexpr long long CARD_FEE_PERMILLE = 29; // 2.9 percent of the amount
constexpr long long CARD_FEE_FIXED_CENTS = 30;
constexpr long long BANK_FEE_CENTS = 25;

/// One transaction: the account it belongs to, its kind, and its amount in cents.
struct Record {
  std::string account;
  std::string kind;
  long long amount_cents;
};

namespace {

/// The text without leading or trailing whitespace.
std::string trim(const std::string &text) {
  const std::size_t first = text.find_first_not_of(" \t\r\n\f\v");
  if (first == std::string::npos)
    return "";
  return text.substr(first, text.find_last_not_of(" \t\r\n\f\v") - first + 1);
}

/// Every comma-separated field of the line, empty fields included.
std::vector<std::string> split_fields(const std::string &line) {
  std::vector<std::string> fields;
  std::size_t start = 0;
  std::size_t comma = line.find(',');
  while (comma != std::string::npos) {
    fields.push_back(line.substr(start, comma - start));
    start = comma + 1;
    comma = line.find(',', start);
  }
  fields.push_back(line.substr(start));
  return fields;
}

/// Dollars with two decimals; negatives in parentheses, for example (1.25).
std::string format_cents(long long cents) {
  std::ostringstream dollars;
  dollars << std::abs(cents) / 100 << '.' << std::setw(2) << std::setfill('0')
          << std::abs(cents) % 100;
  return cents < 0 ? "(" + dollars.str() + ")" : dollars.str();
}

} // namespace

/// Return one Record per data line, skipping blanks and # comments.
/// Throws std::invalid_argument naming the line when it does not have four fields.
std::vector<Record> parse_records(const std::string &text) {
  std::vector<Record> records;
  std::istringstream lines(text);
  std::string raw;
  for (int line_number = 1; std::getline(lines, raw); ++line_number) {
    const std::string line = trim(raw);
    if (line.empty() || line.front() == '#')
      continue;
    const std::vector<std::string> fields = split_fields(line);
    if (fields.size() != 4)
      throw std::invalid_argument("line " + std::to_string(line_number) +
                                  ": expected 4 fields, got " + std::to_string(fields.size()));
    // The fields are date, account, kind and amount; the date is not used.
    records.push_back({trim(fields[1]), trim(fields[2]), std::stoll(fields[3])});
  }
  return records;
}

/// Fee for one transaction. The percentage part rounds half up to whole cents.
/// Throws std::invalid_argument for an unknown kind.
long long fee_cents(const std::string &kind, long long amount_cents) {
  if (kind == "card") {
    const long long scaled = amount_cents * CARD_FEE_PERMILLE + 500;
    const long long percentage = scaled / 1000 - (scaled % 1000 < 0 ? 1 : 0); // floor, not truncate
    return percentage + CARD_FEE_FIXED_CENTS;
  }
  if (kind == "bank")
    return BANK_FEE_CENTS;
  if (kind == "cash")
    return 0;
  throw std::invalid_argument("unknown kind: " + kind);
}

/// Sum amount minus fee for each account.
std::map<std::string, long long> balances(const std::vector<Record> &records) {
  std::map<std::string, long long> totals;
  for (const auto &[account, kind, amount_cents] : records)
    totals[account] += amount_cents - fee_cents(kind, amount_cents);
  return totals;
}

/// One "ACCOUNT  BALANCE" line for each account in totals.
std::vector<std::string> render(const std::map<std::string, long long> &totals) {
  std::vector<std::string> lines;
  for (const auto &[account, cents] : totals)
    lines.push_back(account + "  " + format_cents(cents));
  return lines;
}
