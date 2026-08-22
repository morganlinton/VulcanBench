use chrono::{NaiveDate, TimeZone};
use criterion::{black_box as bb, criterion_group, criterion_main, Bencher, Criterion, Fun};
use tzfile::*;

fn bench_to_utc(tz: impl TimeZone) -> impl FnMut(&mut Bencher, &[u32; 6]) {
    move |b: &mut Bencher, &[year, month, day, hour, minute, second]: &[u32; 6]| {
        b.iter(|| {
            tz.ymd(bb(year as i32), bb(month), bb(day))
                .and_hms(bb(hour), bb(minute), bb(second))
                .timestamp()
        });
    }
}

fn bench_from_utc(tz: impl TimeZone) -> impl FnMut(&mut Bencher, &[u32; 6]) {
    move |b: &mut Bencher, &[year, month, day, hour, minute, second]: &[u32; 6]| {
        b.iter(|| {
            tz.from_utc_datetime(
                &NaiveDate::from_ymd(bb(year as i32), bb(month), bb(day)).and_hms(
                    bb(hour),
                    bb(minute),
                    bb(second),
                ),
            )
        });
    }
}

fn reftz_named(name: &str) -> &'static Tz {
    &*Box::leak(Box::new(Tz::named(name).unwrap()))
}

fn rctz_named(name: &str) -> RcTz {
    RcTz::named(name).unwrap()
}

fn arctz_named(name: &str) -> ArcTz {
    ArcTz::named(name).unwrap()
}

fn crit(c: &mut Criterion) {
    // Check conversion to/from UTC.
    c.bench_functions(
        "utc-to-utc",
        vec![
            Fun::new("reftz", bench_to_utc(reftz_named("UTC"))),
            Fun::new("rctz", bench_to_utc(rctz_named("UTC"))),
            Fun::new("arctz", bench_to_utc(arctz_named("UTC"))),
            Fun::new("chronotz", bench_to_utc(chrono_tz::UTC)),
            Fun::new("builtin", bench_to_utc(chrono::Utc)),
        ],
        [1985, 5, 2, 13, 58, 31],
    );

    c.bench_functions(
        "utc-from-utc",
        vec![
            Fun::new("reftz", bench_from_utc(reftz_named("UTC"))),
            Fun::new("rctz", bench_from_utc(rctz_named("UTC"))),
            Fun::new("arctz", bench_from_utc(arctz_named("UTC"))),
            Fun::new("chronotz", bench_from_utc(chrono_tz::UTC)),
            Fun::new("builtin", bench_from_utc(chrono::Utc)),
        ],
        [1985, 5, 2, 13, 58, 31],
    );

    // Check conversion to/from local time zone.
    c.bench_functions(
        "local-to-utc",
        vec![
            Fun::new("rctz", bench_to_utc(RcTz::new(Tz::local().unwrap()))),
            Fun::new("arctz", bench_to_utc(ArcTz::new(Tz::local().unwrap()))),
            Fun::new("builtin", bench_to_utc(chrono::Local)),
        ],
        [2017, 6, 20, 11, 38, 29],
    );

    c.bench_functions(
        "local-from-utc",
        vec![
            Fun::new("rctz", bench_from_utc(RcTz::new(Tz::local().unwrap()))),
            Fun::new("arctz", bench_from_utc(ArcTz::new(Tz::local().unwrap()))),
            Fun::new("builtin", bench_from_utc(chrono::Local)),
        ],
        [2017, 6, 20, 11, 38, 29],
    );

    // Asia/Tehran is the largest time zone file on macOS as of 2019.
    c.bench_functions(
        "tehran-to-utc",
        vec![
            Fun::new("reftz", bench_to_utc(reftz_named("Asia/Tehran"))),
            Fun::new("rctz", bench_to_utc(rctz_named("Asia/Tehran"))),
            Fun::new("arctz", bench_to_utc(arctz_named("Asia/Tehran"))),
            Fun::new("chronotz", bench_to_utc(chrono_tz::Asia::Tehran)),
        ],
        [2003, 5, 18, 20, 6, 14],
    );

    c.bench_functions(
        "tehran-from-utc",
        vec![
            Fun::new("reftz", bench_from_utc(reftz_named("Asia/Tehran"))),
            Fun::new("rctz", bench_from_utc(rctz_named("Asia/Tehran"))),
            Fun::new("arctz", bench_from_utc(arctz_named("Asia/Tehran"))),
            Fun::new("chronotz", bench_from_utc(chrono_tz::Asia::Tehran)),
        ],
        [2003, 5, 18, 20, 6, 14],
    );

    c.bench_function("parse", |b| {
        let content = std::fs::read("/usr/share/zoneinfo/Asia/Tehran").unwrap();
        b.iter(|| Tz::parse("Asia/Tehran", bb(&content)));
    });
}

criterion_group!(benches, crit);
criterion_main!(benches);
