tzfile
======

[![Crates.io](https://img.shields.io/crates/v/tzfile.svg)](https://crates.io/crates/tzfile)
[![docs.rs](https://docs.rs/tzfile/badge.svg)](https://docs.rs/tzfile)
[![Build status](https://github.com/kennytm/tzfile/workflows/Rust/badge.svg)](https://github.com/kennytm/tzfile/actions?query=workflow%3ARust)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE.txt)

`tzfile` is a `chrono::TimeZone` implementation using the system [tz database].
It can parse compiled (binary) time zone files inside `/usr/share/zoneinfo` into
time zone objects for [`chrono`].

[tz database]: https://en.wikipedia.org/wiki/Tz_database
[`chrono`]: https://github.com/chronotope/chrono

```rust
use chrono::{Utc, TimeZone};
use tzfile::Tz;

let tz = Tz::named("America/New_York")?;
let dt1 = Utc.ymd(2019, 3, 10).and_hms(6, 45, 0);
assert_eq!(dt1.with_timezone(&&tz).to_string(), "2019-03-10 01:45:00 EST");
let dt2 = Utc.ymd(2019, 3, 10).and_hms(7, 15, 0);
assert_eq!(dt2.with_timezone(&&tz).to_string(), "2019-03-10 03:15:00 EDT");
```

## `tzfile` vs `chrono-tz`

`tzfile` loads the time zone information dynamically from the OS, while
`chrono-tz` pins to a particular version of tz database and embeds the parsed
result statically in the resulting executable.

Users of `tzfile` can get the same time zone configuration as the system, and
guarantees the same behavior with other non-Rust programs. However, some systems
do not provide a complete tz database (e.g. Windows), making the program not
portable across multiple platforms.

## Performance

Conversion from UTC to local has comparable performance with `chrono-tz` as both
uses the same data structure (sorted array). In the reverse direction (local to
UTC), `tzfile` caches the conversion table, allowing to be faster than
`chrono-tz` in all cases.

| Time zone type  | Asia/Tehran → UTC | UTC → Asia/Tehran |
|-----------------|------------------:|------------------:|
| `&tzfile::Tz`   | 107 ns            | 33 ns             |
| `tzfile::RcTz`  | 125 ns            | 38 ns             |
| `tzfile::ArcTz` | 156 ns            | 48 ns             |
| `chrono_tz::Tz` | 227 ns            | 43 ns             |

Note: parsing the tz file has an upfront cost of 16 µs.

## Limitations

Due to how `chrono` is designed, leap seconds are always ignored, even when
present in the tz file. Also, only Gregorian calendar is supported.

Currently the `Tz::named()` and `Tz::local()` convenient functions are available
on Unix only.

Parsing POSIX TZ rules is not yet supported, so the predicted time for the far
future may be incorrect.
