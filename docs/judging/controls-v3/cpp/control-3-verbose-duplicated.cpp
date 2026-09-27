// This file is the ledger file. It handles the ledger.

#include <algorithm>
#include <map>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

struct Record {
  std::string account;
  std::string kind;
  long long amount_cents;
};

std::string trim(const std::string &text) {
  // Find the first character that is not a space.
  const std::size_t first = text.find_first_not_of(" \t\r\n\f\v");
  // If there is no such character, the string is all spaces, so return an empty string.
  if (first == std::string::npos) {
    return "";
  }
  // Find the last character that is not a space.
  const std::size_t last = text.find_last_not_of(" \t\r\n\f\v");
  // Return the part in between.
  return text.substr(first, last - first + 1);
}

std::vector<std::string> split_fields(const std::string &line) {
  // First we make an empty list to hold the fields.
  std::vector<std::string> fields;
  // Start at the beginning of the line.
  std::size_t start = 0;
  // Find the first comma.
  std::size_t comma = line.find(',');
  // Keep going while there are commas.
  while (comma != std::string::npos) {
    fields.push_back(line.substr(start, comma - start));
    start = comma + 1;
    comma = line.find(',', start);
  }
  // Add the last field after the last comma.
  fields.push_back(line.substr(start));
  return fields;
}

std::vector<Record> parse_records(const std::string &text) {
  // First we make an empty list to hold the records.
  std::vector<Record> records;
  // Now we go through every line.
  std::istringstream lines(text);
  std::string raw;
  int line_number = 0;
  while (std::getline(lines, raw)) {
    line_number = line_number + 1;
    const std::string line = trim(raw);
    // Skip the line if it is blank.
    if (line == "") {
      continue;
    }
    // Skip the line if it is a comment.
    if (line[0] == '#') {
      continue;
    }
    const std::vector<std::string> fields = split_fields(line);
    // Check the number of fields.
    if (fields.size() != 4) {
      throw std::invalid_argument("line " + std::to_string(line_number) +
                                  ": expected 4 fields, got " + std::to_string(fields.size()));
    }
    const std::string account = trim(fields[1]);
    const std::string kind = trim(fields[2]);
    const long long amount = std::stoll(fields[3]);
    // Validate the kind here so bad kinds are caught early.
    if (kind == "card") {
      // The kind is fine.
    } else if (kind == "bank") {
      // The kind is fine.
    } else if (kind == "cash") {
      // The kind is fine.
    } else {
      throw std::invalid_argument("unknown kind: " + kind);
    }
    records.push_back({account, kind, amount});
  }
  return records;
}

long long fee_cents(const std::string &kind, long long amount_cents) {
  // Work out the fee for a card.
  if (kind == "card") {
    long long fee = (amount_cents * 29 + 500) / 1000;
    // If the amount is negative and there is a remainder, round down one more.
    if ((amount_cents * 29 + 500) % 1000 < 0) {
      fee = fee - 1;
    }
    fee = fee + 30;
    return fee;
  }
  // Work out the fee for a bank transfer.
  if (kind == "bank") {
    return 25;
  }
  // Work out the fee for cash.
  if (kind == "cash") {
    return 0;
  }
  throw std::invalid_argument("unknown kind: " + kind);
}

std::map<std::string, long long> balances(const std::vector<Record> &records) {
  // Start with an empty map of totals.
  std::map<std::string, long long> totals;
  for (const Record &record : records) {
    const std::string account = record.account;
    const std::string kind = record.kind;
    const long long amount = record.amount_cents;
    long long fee = 0;
    long long net = 0;
    // Work out the fee for a card.
    if (kind == "card") {
      fee = (amount * 29 + 500) / 1000;
      if ((amount * 29 + 500) % 1000 < 0) {
        fee = fee - 1;
      }
      fee = fee + 30;
      net = amount - fee;
    }
    // Work out the fee for a bank transfer.
    else if (kind == "bank") {
      fee = 25;
      net = amount - fee;
    }
    // Work out the fee for cash.
    else if (kind == "cash") {
      fee = 0;
      net = amount - fee;
    } else {
      throw std::invalid_argument("unknown kind: " + kind);
    }
    // Add the net amount to the account.
    if (totals.count(account) > 0) {
      totals[account] = totals[account] + net;
    } else {
      totals[account] = net;
    }
  }
  return totals;
}

long long total_fees(const std::vector<Record> &records) {
  // Sum all the fees. Uses the same rules as balances.
  long long fees = 0;
  for (const Record &record : records) {
    const std::string kind = record.kind;
    const long long amount = record.amount_cents;
    if (kind == "card") {
      long long fee = (amount * 29 + 500) / 1000;
      if ((amount * 29 + 500) % 1000 < 0) {
        fee = fee - 1;
      }
      fees = fees + fee + 30;
    } else if (kind == "bank") {
      fees = fees + 25;
    } else if (kind == "cash") {
      fees = fees + 0;
    } else {
      throw std::invalid_argument("unknown kind: " + kind);
    }
  }
  return fees;
}

std::vector<std::string> render(const std::map<std::string, long long> &totals) {
  // Build the output lines.
  std::vector<std::string> lines;
  std::vector<std::string> accounts;
  for (const auto &entry : totals) {
    accounts.push_back(entry.first);
  }
  std::sort(accounts.begin(), accounts.end());
  for (const std::string &account : accounts) {
    long long cents = totals.at(account);
    // Format negative numbers with parentheses.
    if (cents < 0) {
      cents = -cents;
      std::string pennies = std::to_string(cents % 100);
      if (pennies.size() < 2) {
        pennies = "0" + pennies;
      }
      const std::string dollars = std::to_string(cents / 100) + "." + pennies;
      lines.push_back(account + "  (" + dollars + ")");
    } else {
      std::string pennies = std::to_string(cents % 100);
      if (pennies.size() < 2) {
        pennies = "0" + pennies;
      }
      const std::string dollars = std::to_string(cents / 100) + "." + pennies;
      lines.push_back(account + "  " + dollars);
    }
  }
  return lines;
}
