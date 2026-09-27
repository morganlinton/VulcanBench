// Include cstdlib.
#include <cstdlib>
// Include iomanip.
#include <iomanip>
// Include map.
#include <map>
// Include sstream.
#include <sstream>
// Include stdexcept.
#include <stdexcept>
// Include string.
#include <string>
// Include vector.
#include <vector>
// Define the Record struct.
struct Record {
  std::string account;
  std::string kind;
  long long amount_cents;
};
// Define trim.
std::string trim(const std::string &s) {
  // Find the first character that is not whitespace and store it in b.
  auto b = s.find_first_not_of(" \t\r\n\f\v");
  // Return an empty string if b is npos.
  if (b == std::string::npos)
    return "";
  // Return the substring from b to the last character that is not whitespace.
  return s.substr(b, s.find_last_not_of(" \t\r\n\f\v") - b + 1);
}
// Define split_fields.
std::vector<std::string> split_fields(const std::string &s) {
  // Create an empty vector.
  std::vector<std::string> f;
  // Set b to 0 and e to the first comma.
  std::size_t b = 0, e = s.find(',');
  // Loop while e is not npos.
  while (e != std::string::npos) {
    // Push back the substring from b to e.
    f.push_back(s.substr(b, e - b));
    // Set b to e plus 1.
    b = e + 1;
    // Find the next comma after b.
    e = s.find(',', b);
  }
  // Push back the rest of the string.
  f.push_back(s.substr(b));
  // Return the vector.
  return f;
}
// Define parse_records.
std::vector<Record> parse_records(const std::string &t) {
  // Create an empty vector.
  std::vector<Record> r;
  // Create a string stream from t.
  std::istringstream s(t);
  // Declare a string l.
  std::string l;
  // Loop over the lines with an index.
  for (int i = 1; std::getline(s, l); ++i) {
    // Trim the line.
    l = trim(l);
    // Continue if the line is empty or starts with #.
    if (l.empty() || l[0] == '#')
      continue;
    // Split the line on commas.
    auto f = split_fields(l);
    // Throw if the size is not 4.
    if (f.size() != 4)
      throw std::invalid_argument("line " + std::to_string(i) + ": expected 4 fields, got " +
                                  std::to_string(f.size()));
    // Push back a record made from the fields.
    r.push_back({trim(f[1]), trim(f[2]), std::stoll(f[3])});
  }
  // Return the vector.
  return r;
}
// Define fee_cents.
long long fee_cents(const std::string &k, long long a) {
  // If k is card compute the fee.
  if (k == "card") {
    // Set p to a times 29 plus 500.
    long long p = a * 29 + 500;
    // Return p divided by 1000, minus 1 if p has a negative remainder, plus 30.
    return p / 1000 - (p % 1000 < 0) + 30;
  }
  // If k is bank return 25.
  if (k == "bank")
    return 25;
  // If k is cash return 0.
  if (k == "cash")
    return 0;
  // Otherwise throw.
  throw std::invalid_argument("unknown kind: " + k);
}
// Define balances.
std::map<std::string, long long> balances(const std::vector<Record> &r) {
  // Create an empty map.
  std::map<std::string, long long> b;
  // Loop over r.
  for (auto &[a, k, c] : r)
    // Add c minus the fee to b[a].
    b[a] += c - fee_cents(k, c);
  // Return the map.
  return b;
}
// Define format_cents.
std::string format_cents(long long c) {
  // Create a string stream.
  std::ostringstream o;
  // Write the dollars, a dot and the two-digit cents to o.
  o << std::abs(c) / 100 << '.' << std::setw(2) << std::setfill('0') << std::abs(c) % 100;
  // Return the string in parentheses if c is negative else the string.
  return c < 0 ? "(" + o.str() + ")" : o.str();
}
// Define render.
std::vector<std::string> render(const std::map<std::string, long long> &b) {
  // Create an empty vector.
  std::vector<std::string> l;
  // Loop over b.
  for (auto &[a, c] : b)
    // Push back the account, two spaces and the formatted cents.
    l.push_back(a + "  " + format_cents(c));
  // Return the vector.
  return l;
}
