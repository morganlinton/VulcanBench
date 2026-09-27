#include <cstdlib>
#include <iomanip>
#include <map>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

struct Record {
  std::string account;
  std::string kind;
  long long amount_cents;
};

namespace {

std::string trim(const std::string &text) {
  const std::size_t first = text.find_first_not_of(" \t\r\n\f\v");
  if (first == std::string::npos)
    return "";
  return text.substr(first, text.find_last_not_of(" \t\r\n\f\v") - first + 1);
}

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

class Money {
public:
  explicit Money(long long cents) : cents_(cents) {}
  long long cents() const { return cents_; }
  Money minus(const Money &other) const { return Money(cents_ - other.cents_); }
  Money plus(const Money &other) const { return Money(cents_ + other.cents_); }

private:
  long long cents_;
};

class FeePolicy {
public:
  virtual ~FeePolicy() = default;
  virtual Money fee(const Money &amount) const = 0;
};

class CardFeePolicy : public FeePolicy {
public:
  Money fee(const Money &amount) const override {
    const long long scaled = amount.cents() * 29 + 500;
    return Money(scaled / 1000 - (scaled % 1000 < 0 ? 1 : 0) + 30);
  }
};

class BankFeePolicy : public FeePolicy {
public:
  Money fee(const Money &) const override { return Money(25); }
};

class CashFeePolicy : public FeePolicy {
public:
  Money fee(const Money &) const override { return Money(0); }
};

class FeePolicyRegistry {
public:
  FeePolicyRegistry &register_policy(const std::string &kind, std::unique_ptr<FeePolicy> policy) {
    policies_[kind] = std::move(policy);
    return *this;
  }

  const FeePolicy &resolve(const std::string &kind) const {
    const auto found = policies_.find(kind);
    if (found == policies_.end())
      throw std::invalid_argument("unknown kind: " + kind);
    return *found->second;
  }

private:
  std::map<std::string, std::unique_ptr<FeePolicy>> policies_;
};

const FeePolicyRegistry REGISTRY =
    std::move(FeePolicyRegistry()
                  .register_policy("card", std::make_unique<CardFeePolicy>())
                  .register_policy("bank", std::make_unique<BankFeePolicy>())
                  .register_policy("cash", std::make_unique<CashFeePolicy>()));

class LedgerRecord {
public:
  LedgerRecord(std::string account, std::string kind, Money amount)
      : account(std::move(account)), kind(std::move(kind)), amount(amount) {}
  std::string account;
  std::string kind;
  Money amount;
};

class RecordParser {
public:
  explicit RecordParser(std::string text) : text_(std::move(text)) {}

  std::vector<LedgerRecord> parse() const {
    std::vector<LedgerRecord> parsed;
    std::istringstream lines(text_);
    std::string line;
    for (int number = 1; std::getline(lines, line); ++number)
      if (keep(line))
        parsed.push_back(parse_line(number, line));
    return parsed;
  }

private:
  static bool keep(const std::string &line) {
    return !trim(line).empty() && trim(line).front() != '#';
  }

  static LedgerRecord parse_line(int number, const std::string &line) {
    const std::vector<std::string> fields = split_fields(trim(line));
    if (fields.size() != 4)
      throw std::invalid_argument("line " + std::to_string(number) + ": expected 4 fields, got " +
                                  std::to_string(fields.size()));
    return LedgerRecord(trim(fields[1]), trim(fields[2]), Money(std::stoll(fields[3])));
  }

  std::string text_;
};

class BalanceAggregator {
public:
  explicit BalanceAggregator(const FeePolicyRegistry &registry) : registry_(registry) {}

  BalanceAggregator &accept(const LedgerRecord &record) {
    const Money fee = registry_.resolve(record.kind).fee(record.amount);
    Money &total = totals_.try_emplace(record.account, Money(0)).first->second;
    total = total.plus(record.amount.minus(fee));
    return *this;
  }

  std::map<std::string, long long> result() const {
    std::map<std::string, long long> out;
    for (const auto &[account, money] : totals_)
      out[account] = money.cents();
    return out;
  }

private:
  const FeePolicyRegistry &registry_;
  std::map<std::string, Money> totals_;
};

class Renderer {
public:
  std::vector<std::string> render(const std::map<std::string, long long> &totals) const {
    std::vector<std::string> lines;
    for (const auto &[account, cents] : totals)
      lines.push_back(account + "  " + money(cents));
    return lines;
  }

private:
  static std::string money(long long cents) {
    std::ostringstream dollars;
    dollars << std::abs(cents) / 100 << '.' << std::setw(2) << std::setfill('0')
            << std::abs(cents) % 100;
    return cents < 0 ? "(" + dollars.str() + ")" : dollars.str();
  }
};

} // namespace

std::vector<Record> parse_records(const std::string &text) {
  std::vector<Record> records;
  for (const LedgerRecord &r : RecordParser(text).parse())
    records.push_back({r.account, r.kind, r.amount.cents()});
  return records;
}

long long fee_cents(const std::string &kind, long long amount_cents) {
  return REGISTRY.resolve(kind).fee(Money(amount_cents)).cents();
}

std::map<std::string, long long> balances(const std::vector<Record> &records) {
  BalanceAggregator aggregator(REGISTRY);
  for (const auto &[account, kind, amount] : records)
    aggregator.accept(LedgerRecord(account, kind, Money(amount)));
  return aggregator.result();
}

std::vector<std::string> render(const std::map<std::string, long long> &totals) {
  return Renderer().render(totals);
}
