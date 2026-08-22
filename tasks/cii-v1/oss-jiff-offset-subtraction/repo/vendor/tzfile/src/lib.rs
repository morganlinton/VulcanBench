//! `tzfile` is a [`chrono::TimeZone`] implementation using the system tz
//! database. It can parse compiled (binary) time zone files inside
//! `/usr/share/zoneinfo` into time zone objects for `chrono`.

use byteorder::{ByteOrder, BE};
use chrono::{FixedOffset, LocalResult, NaiveDate, NaiveDateTime, TimeZone};
use std::cmp::Ordering;
#[cfg(unix)]
use std::env;
use std::error;
use std::fmt;
#[cfg(unix)]
use std::fs::read;
use std::hash::Hash;
use std::io;
use std::mem::size_of;
use std::ops::Deref;
use std::rc::Rc;
use std::str::from_utf8;
use std::sync::Arc;

/// `Oz` ("offset zone") represents a continuous period of time where the offset
/// from UTC is constant and has the same abbreviation.
#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
struct Oz {
    /// The time zone offset from UTC.
    offset: FixedOffset,
    /// The byte index to the time zone abbreviation.
    name: u8,
}

impl Oz {
    /// Converts a UTC timestamp to a local timestamp.
    fn to_local(self, utc_ts: i64) -> i64 {
        utc_ts + i64::from(self.offset.local_minus_utc())
    }
}

/// Time zone parsed from a tz database file.
///
/// When a time zone has complex transition rules, a `Tz` object can be very
/// large and expensive to clone. As every [`DateTime`](chrono::DateTime)
/// instant would store a copy of the time zone object, it would be very slow to
/// support `DateTime<Tz>` directly. Therefore, `Tz` itself does not implement
/// [`TimeZone`]. Rather, you may use one of the following instead:
///
/// * `&'a Tz` — zero cost to clone, but only valid within the lifetime `'a`.
/// * [`RcTz`] — uses reference counting ([`Rc`]) to support shallow cloning,
///     but is not thread-safe.
/// * [`ArcTz`] — uses atomic reference counting ([`Arc`]) to support shallow
///     cloning, slightly more expensive than [`RcTz`] but is thread-safe.
///
/// # Examples
///
/// Read the time zone information from the system, and use `&Tz` as `TimeZone`.
///
/// ```
/// # #[cfg(unix)] {
/// use chrono::{Utc, TimeZone};
/// use tzfile::Tz;
///
/// let tz = Tz::named("America/New_York")?;
/// let dt1 = Utc.ymd(2019, 3, 10).and_hms(6, 45, 0);
/// assert_eq!(dt1.with_timezone(&&tz).to_string(), "2019-03-10 01:45:00 EST");
/// let dt2 = Utc.ymd(2019, 3, 10).and_hms(7, 15, 0);
/// assert_eq!(dt2.with_timezone(&&tz).to_string(), "2019-03-10 03:15:00 EDT");
///
/// # } Ok::<_, std::io::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tz {
    /// Null-terminated time zone abbreviations concatenated as a single string.
    names: Box<str>,
    /// Sorted array of UTC-to-local conversion results. The first item of the
    /// tuple is the starting UTC timestamp, and second item is the
    /// corresponding local offset.
    utc_to_local: Box<[(i64, Oz)]>,
    /// Sorted array of local-to-UTC conversion results. The first item of the
    /// tuple is the starting local timestamp, and second item is the
    /// corresponding local offset (with ambiguity/invalid time handling).
    local_to_utc: Box<[(i64, LocalResult<Oz>)]>,
}

/// Extracts the lower bound index from the result of standard `binary_search`.
fn to_lower_bound(bsr: Result<usize, usize>) -> usize {
    bsr.unwrap_or_else(|i| i - 1)
}

impl Tz {
    /// Obtains an [`Oz`] at the local date.
    ///
    /// Returns `None` if the date is invalid.
    fn oz_from_local_date(&self, local_date: NaiveDate) -> Option<Oz> {
        let min_ts = local_date.and_hms(0, 0, 0).timestamp();
        self.oz_from_local_timestamp(min_ts)
            .earliest()
            .or_else(|| self.oz_from_local_timestamp(min_ts + 86399).earliest())
    }

    /// Obtains the [`Oz`] at the local timestamp.
    fn oz_from_local_timestamp(&self, local_ts: i64) -> LocalResult<Oz> {
        let index = to_lower_bound(
            self.local_to_utc
                .binary_search_by(|&(local, _)| local.cmp(&local_ts)),
        );
        self.local_to_utc[index].1
    }

    /// Obtains the [`Oz`] at the UTC timestamp.
    fn oz_from_utc_timestamp(&self, timestamp: i64) -> Oz {
        let index = to_lower_bound(
            self.utc_to_local
                .binary_search_by(|&(utc, _)| utc.cmp(&timestamp)),
        );
        self.utc_to_local[index].1
    }
}

/// Offset type associated with [`Tz`].
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct Offset<T> {
    oz: Oz,
    tz: T,
}

impl<T: Clone + Deref<Target = Tz>> chrono::Offset for Offset<T> {
    fn fix(&self) -> FixedOffset {
        self.oz.offset
    }
}

impl<T: Deref<Target = Tz>> fmt::Display for Offset<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suffix = &self.tz.names[usize::from(self.oz.name)..];
        let len = suffix.find('\0').unwrap_or_else(|| suffix.len());
        f.write_str(&suffix[..len])
    }
}

impl<T: Deref<Target = Tz>> fmt::Debug for Offset<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Reference-counted time zone.
///
/// This type is equivalent to [`Rc`]`<`[`Tz`]`>`, but needed to workaround
/// Rust's coherence rule to implement [`TimeZone`].
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RcTz(pub Rc<Tz>);

impl RcTz {
    /// Wraps an existing [`Tz`] object in this reference-counted container.
    pub fn new(tz: Tz) -> Self {
        Self(Rc::new(tz))
    }

    /// Reads and parses a system time zone.
    ///
    /// Equivalent to calling [`Tz::named()`] and wraps the result in this
    /// reference-counted container.
    ///
    /// This function is currently only supported on Unix.
    #[cfg(unix)]
    pub fn named(name: &str) -> std::io::Result<Self> {
        Tz::named(name).map(Self::new)
    }
}

impl Deref for RcTz {
    type Target = Tz;
    fn deref(&self) -> &Tz {
        &self.0
    }
}

impl From<Tz> for RcTz {
    fn from(tz: Tz) -> Self {
        Self::new(tz)
    }
}

/// Atomic reference-counted time zone.
///
/// This type is equivalent to [`Arc`]`<`[`Tz`]`>`, but needed to workaround
/// Rust's coherence rule to implement [`TimeZone`].
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ArcTz(pub Arc<Tz>);

impl ArcTz {
    /// Wraps an existing [`Tz`] object in this atomic reference-counted
    /// container.
    pub fn new(tz: Tz) -> Self {
        Self(Arc::new(tz))
    }

    /// Reads and parses a system time zone.
    ///
    /// Equivalent to calling [`Tz::named()`] and wraps the result in this
    /// atomic reference-counted container.
    ///
    /// This function is currently only supported on Unix.
    #[cfg(unix)]
    pub fn named(name: &str) -> std::io::Result<Self> {
        Tz::named(name).map(Self::new)
    }
}

impl Deref for ArcTz {
    type Target = Tz;
    fn deref(&self) -> &Tz {
        &self.0
    }
}

impl From<Tz> for ArcTz {
    fn from(tz: Tz) -> Self {
        Self::new(tz)
    }
}

macro_rules! implements_time_zone {
    () => {
        type Offset = Offset<Self>;

        fn from_offset(offset: &Self::Offset) -> Self {
            Self::clone(&offset.tz)
        }

        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> Self::Offset {
            Offset {
                oz: self.oz_from_utc_timestamp(utc.timestamp()),
                tz: self.clone(),
            }
        }

        fn offset_from_utc_date(&self, utc: &NaiveDate) -> Self::Offset {
            self.offset_from_utc_datetime(&utc.and_hms(12, 0, 0))
        }

        fn offset_from_local_date(&self, local: &NaiveDate) -> LocalResult<Self::Offset> {
            if let Some(oz) = self.oz_from_local_date(*local) {
                LocalResult::Single(Offset {
                    oz,
                    tz: self.clone(),
                })
            } else {
                LocalResult::None
            }
        }

        fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<Self::Offset> {
            let timestamp = local.timestamp();
            self.oz_from_local_timestamp(timestamp).map(|oz| Offset {
                oz,
                tz: self.clone(),
            })
        }
    };
}

impl<'a> TimeZone for &'a Tz {
    implements_time_zone!();
}

impl TimeZone for RcTz {
    implements_time_zone!();
}

impl TimeZone for ArcTz {
    implements_time_zone!();
}

/// Parse errors from [`Tz::parse()`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Error {
    /// The source bytes is too short to parse the header.
    HeaderTooShort,
    /// The source does not start with the correct magic string (`"TZif"`).
    InvalidMagic,
    /// Unsupported tzfile version. Currently we only support versions 2 and 3.
    UnsupportedVersion,
    /// The lengths of several related arrays in the file are not the same,
    /// making the file invalid.
    InconsistentTypeCount,
    /// The tzfile contains no time zone information.
    NoTypes,
    /// The time zone offset exceeds ±86400s (1 day).
    OffsetOverflow,
    /// Some time zone abbreviations are not valid UTF-8.
    NonUtf8Abbr,
    /// The source bytes is too short to parse the content.
    DataTooShort,
    /// Invalid time zone file name.
    InvalidTimeZoneFileName,
    /// The time zone transition type is invalid.
    InvalidType,
    /// Name offset is out of bounds.
    NameOffsetOutOfBounds,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("tzfile error: ")?;
        f.write_str(match self {
            Error::HeaderTooShort => "header too short",
            Error::InvalidMagic => "invalid magic",
            Error::UnsupportedVersion => "unsupported version",
            Error::InconsistentTypeCount => "inconsistent type count",
            Error::NoTypes => "no types",
            Error::OffsetOverflow => "time zone offset overflow",
            Error::NonUtf8Abbr => "non-UTF-8 time zone abbreviations",
            Error::DataTooShort => "data too short",
            Error::InvalidTimeZoneFileName => "invalid time zone file name",
            Error::InvalidType => "invalid time zone transition type",
            Error::NameOffsetOutOfBounds => "name offset out of bounds",
        })
    }
}

impl error::Error for Error {}

impl From<Error> for io::Error {
    fn from(e: Error) -> io::Error {
        io::Error::new(io::ErrorKind::Other, e)
    }
}

static MAGIC: [u8; 4] = *b"TZif";

struct Header {
    tzh_ttisgmtcnt: usize,
    tzh_ttisstdcnt: usize,
    tzh_leapcnt: usize,
    tzh_timecnt: usize,
    tzh_typecnt: usize,
    tzh_charcnt: usize,
}

impl Header {
    /// Parses the header from the prefix of the `source`.
    fn parse(source: &[u8]) -> Result<Self, Error> {
        if source.len() < Self::HEADER_LEN {
            return Err(Error::HeaderTooShort);
        }
        if source[..4] != MAGIC {
            return Err(Error::InvalidMagic);
        }
        match source[4] {
            b'2' | b'3' => {}
            _ => return Err(Error::UnsupportedVersion),
        }
        let tzh_ttisgmtcnt = BE::read_u32(&source[20..24]) as usize;
        let tzh_ttisstdcnt = BE::read_u32(&source[24..28]) as usize;
        let tzh_leapcnt = BE::read_u32(&source[28..32]) as usize;
        let tzh_timecnt = BE::read_u32(&source[32..36]) as usize;
        let tzh_typecnt = BE::read_u32(&source[36..40]) as usize;
        let tzh_charcnt = BE::read_u32(&source[40..44]) as usize;

        if (tzh_ttisgmtcnt != 0 && tzh_ttisgmtcnt != tzh_typecnt)
            || (tzh_ttisstdcnt != 0 && tzh_ttisstdcnt != tzh_typecnt)
        {
            return Err(Error::InconsistentTypeCount);
        }
        if tzh_typecnt == 0 {
            return Err(Error::NoTypes);
        }

        Ok(Header {
            tzh_ttisgmtcnt,
            tzh_ttisstdcnt,
            tzh_leapcnt,
            tzh_timecnt,
            tzh_typecnt,
            tzh_charcnt,
        })
    }

    /// The length of the header.
    const HEADER_LEN: usize = 44;

    /// The length of the content, when `time_t` is represented by type `L`.
    fn data_len<L>(&self) -> usize {
        self.tzh_timecnt * (size_of::<L>() + 1)
            + self.tzh_typecnt * 6
            + self.tzh_charcnt
            + self.tzh_leapcnt * (size_of::<L>() + 4)
            + self.tzh_ttisstdcnt
            + self.tzh_ttisgmtcnt
    }

    /// Parses the time zone information from the prefix of `content`.
    fn parse_content(&self, content: &[u8]) -> Result<Tz, Error> {
        // Obtain the byte indices where each array ends.
        let trans_encoded_end = self.tzh_timecnt * 8;
        let local_time_types_end = trans_encoded_end + self.tzh_timecnt;
        let infos_end = local_time_types_end + self.tzh_typecnt * 6;
        let abbr_end = infos_end + self.tzh_charcnt;

        // Collect the timezone abbreviations.
        let names = from_utf8(&content[infos_end..abbr_end]).map_err(|_| Error::NonUtf8Abbr)?;

        // Collect the timezone infos.
        let ozs = content[local_time_types_end..infos_end]
            .chunks_exact(6)
            .map(|encoded| {
                let seconds = BE::read_i32(&encoded[..4]);
                let offset = FixedOffset::east_opt(seconds).ok_or(Error::OffsetOverflow)?;
                let name = encoded[5];
                if usize::from(name) >= names.len() {
                    return Err(Error::NameOffsetOutOfBounds);
                }
                Ok(Oz { offset, name })
            })
            .collect::<Result<Vec<_>, Error>>()?;

        // Collect the transition times.
        let trans_encoded = &content[..trans_encoded_end];
        let local_time_types = &content[trans_encoded_end..local_time_types_end];

        let mut prev_oz = ozs[0];

        let mut utc_to_local = Vec::with_capacity(self.tzh_timecnt + 1);
        utc_to_local.push((i64::min_value(), prev_oz));
        for (te, &ltt) in trans_encoded.chunks_exact(8).zip(local_time_types) {
            let oz = *ozs.get(usize::from(ltt)).ok_or(Error::InvalidType)?;
            let timestamp = BE::read_i64(te);
            utc_to_local.push((timestamp, oz));
        }

        let mut local_to_utc = Vec::with_capacity(self.tzh_timecnt * 2 + 1);
        local_to_utc.push((i64::min_value(), LocalResult::Single(prev_oz)));
        for &(utc_ts, cur_oz) in &utc_to_local[1..] {
            let prev_local_ts = prev_oz.to_local(utc_ts);
            let cur_local_ts = cur_oz.to_local(utc_ts);
            match prev_local_ts.cmp(&cur_local_ts) {
                Ordering::Less => {
                    local_to_utc.push((prev_local_ts, LocalResult::None));
                    local_to_utc.push((cur_local_ts, LocalResult::Single(cur_oz)));
                }
                Ordering::Equal => {
                    local_to_utc.push((cur_local_ts, LocalResult::Single(cur_oz)));
                }
                Ordering::Greater => {
                    local_to_utc.push((cur_local_ts, LocalResult::Ambiguous(prev_oz, cur_oz)));
                    local_to_utc.push((prev_local_ts, LocalResult::Single(cur_oz)));
                }
            };
            prev_oz = cur_oz;
        }

        Ok(Tz {
            names: names.into(),
            utc_to_local: utc_to_local.into_boxed_slice(),
            local_to_utc: local_to_utc.into_boxed_slice(),
        })
    }
}

impl Tz {
    /// Parses the content of the tz database file.
    ///
    /// This crate can only recognize version 2 and 3 of the tz database. Like
    /// `chrono`, leap second information is ignored. The embedded POSIX TZ
    /// string, which describes non-hard-coded transition rules in the far
    /// future, is also not handled.
    ///
    /// # Examples
    ///
    /// Read a file into bytes and then parse it.
    ///
    /// ```rust
    /// # #[cfg(unix)] {
    /// use tzfile::Tz;
    ///
    /// let content = std::fs::read("/usr/share/zoneinfo/Etc/UTC")?;
    /// let tz = Tz::parse("Etc/UTC", &content)?;
    ///
    /// # } Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse(_name: &str, source: &[u8]) -> Result<Self, Error> {
        let header = Header::parse(source)?;
        let first_ver_len = Header::HEADER_LEN + header.data_len::<i32>();
        let source = source.get(first_ver_len..).ok_or(Error::DataTooShort)?;
        let header = Header::parse(source)?;
        let second_ver_len = Header::HEADER_LEN + header.data_len::<i64>();
        if source.len() < second_ver_len {
            return Err(Error::DataTooShort);
        }
        header.parse_content(&source[Header::HEADER_LEN..])
    }

    /// Reads and parses a system time zone.
    ///
    /// This function is equivalent to reading `/usr/share/zoneinfo/{name}` and
    /// the constructs a time zone via [`parse()`](Tz::parse).
    ///
    /// This function is currently only supported on Unix.
    #[cfg(unix)]
    pub fn named(name: &str) -> std::io::Result<Self> {
        if name.contains('.') {
            return Err(Error::InvalidTimeZoneFileName.into());
        }
        let content = read(format!("/usr/share/zoneinfo/{}", name))?;
        Ok(Self::parse(name, &content)?)
    }

    /// Reads the parses the current local time zone.
    ///
    /// This function calls [`Tz::named`]`($TZ)` if the environment variable
    /// `$TZ` is set, otherwise reads and parses `/etc/localtime`.
    ///
    /// This function is currently only supported on Unix.
    #[cfg(unix)]
    pub fn local() -> std::io::Result<Self> {
        if let Ok(tz) = env::var("TZ") {
            if !tz.is_empty() {
                return Self::named(&tz);
            }
        }
        let content = read("/etc/localtime")?;
        Ok(Self::parse("Local", &content)?)
    }
}

impl From<chrono::Utc> for Tz {
    fn from(_: chrono::Utc) -> Self {
        let oz = Oz {
            offset: FixedOffset::east(0),
            name: 0,
        };
        Self {
            names: "UTC\0".into(),
            utc_to_local: vec![(i64::min_value(), oz)].into_boxed_slice(),
            local_to_utc: vec![(i64::min_value(), LocalResult::Single(oz))].into_boxed_slice(),
        }
    }
}

impl From<FixedOffset> for Tz {
    fn from(offset: FixedOffset) -> Self {
        let mut name = offset.to_string();
        name.push('\0');
        let oz = Oz { offset, name: 0 };
        Self {
            names: name.into_boxed_str(),
            utc_to_local: vec![(i64::min_value(), oz)].into_boxed_slice(),
            local_to_utc: vec![(i64::min_value(), LocalResult::Single(oz))].into_boxed_slice(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn tz_from_fixed_offset() {
        let utc_tz = Tz::from(chrono::Utc);
        let fixed_1_tz = Tz::from(chrono::FixedOffset::east(3600));

        let dt_0 = (&utc_tz).ymd(1000, 1, 1).and_hms(15, 0, 0);
        let dt_1_converted = dt_0.with_timezone(&&fixed_1_tz);
        let dt_1_expected = (&fixed_1_tz).ymd(1000, 1, 1).and_hms(16, 0, 0);
        assert_eq!(dt_1_converted, dt_1_expected);
    }

    #[test]
    fn parse_valid_contents() {
        let contents: Vec<&[u8]> = vec![
            // #0
            b"TZif2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\x04\0\0\0\0\0\0UTC\0\0\0\
              TZif2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\x01\0\0\0\x01\0\0\0\x04\xF8\0\0\0\0\0\0\0\0\0\0\0\0\0\0UTC\0\0\0\nUTC0\n",

            // #1
            b"TZif2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\x04\0\0\0\0\0\0UTC\0\
              TZif2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\x04\0\0\0\0\0\0UTC\0\nUTC0\n",
        ];

        for (i, content) in contents.into_iter().enumerate() {
            assert!(
                Tz::parse("__valid__", content).is_ok(),
                "test #{}: should be able to parse {:x?}",
                i,
                content
            );
        }
    }

    #[test]
    fn parse_invalid_contents() {
        let contents: Vec<(&[u8], Error)> = vec![
            (
                // #0
                b"",
                Error::HeaderTooShort,
            ),
            (
                // #1
                b"not valid",
                Error::HeaderTooShort,
            ),
            (
                // #2
                b"TZif",
                Error::HeaderTooShort,
            ),
            (
                // #3
                b"TZif\0",
                Error::HeaderTooShort,
            ),
            (
                // #4
                b"file with invalid magic should produce error",
                Error::InvalidMagic,
            ),
            (
                // #5
                b"TZifxxxxxxxxxxxxxxxx\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                Error::UnsupportedVersion,
            ),
            (
                // #6
                b"TZif1xxxxxxxxxxxxxxx\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                Error::UnsupportedVersion,
            ),
            (
                // #7
                b"TZif3xxxxxxxxxxxxxxx\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
                Error::NoTypes,
            ),
            (
                // #8
                b"TZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x02\0\0\0\0\0\0\0\0\0\0\0\x03\0\0\0\0",
                Error::InconsistentTypeCount,
            ),
            (
                // #9
                b"TZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x02\0\0\0\0\0\0\0\0\0\0\0\x02\0\0\0\0",
                Error::InconsistentTypeCount,
            ),
            (
                // #10
                b"TZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x02\0\0\0\0",
                Error::InconsistentTypeCount,
            ),
            (
                // #11
                b"TZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0",
                Error::DataTooShort,
            ),
            (
                // #12
                b"TZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0xxxxxxxx",
                Error::HeaderTooShort,
            ),
            (
                // #13
                b"TZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0xxxxxxxxfile with invalid magic should produce error",
                Error::InvalidMagic,
            ),
            (
                // #14
                b"TZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0xxxxxxxxTZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0",
                Error::DataTooShort,
            ),
            (
                // #15
                b"TZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0xxxxxxxxTZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0aaaaaaaa",
                Error::OffsetOverflow,
            ),
            (
                // #16
                b"TZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0xxxxxxxxTZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0\0\0aaaaaa",
                Error::NameOffsetOutOfBounds,
            ),
            (
                // #17
                b"TZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0xxxxxxxxTZif2xxxxxxxxxxxxxxx\0\0\0\x01\0\0\0\x01\0\0\0\0\0\0\0\x01\0\0\0\x01\0\0\0\x02tttttttti\0\0aaa\0A\0aa",
                Error::InvalidType,
            ),
        ];

        for (i, (content, error)) in contents.into_iter().enumerate() {
            assert_eq!(
                Tz::parse("__invalid__", content),
                Err(error),
                "test #{}: should not be able to parse {:x?}",
                i,
                content
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn invalid_timezone_filename() {
        let err = Tz::named("../../../../../../../../etc/passwd").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            err.into_inner().unwrap().downcast::<Error>().unwrap(),
            Box::new(Error::InvalidTimeZoneFileName)
        );
    }

    #[test]
    #[cfg(unix)]
    fn rctz_arctz() {
        let tz = Tz::named("Europe/London").unwrap();
        let rctz = RcTz::named("Europe/London").unwrap();
        let arctz = ArcTz::named("Europe/London").unwrap();
        let rctz2 = RcTz::new(tz.clone());
        let arctz2 = ArcTz::new(tz.clone());
        assert_eq!(tz, *rctz);
        assert_eq!(tz, *arctz);
        assert_eq!(rctz, rctz2);
        assert_eq!(arctz, arctz2);
    }

    #[test]
    #[cfg(unix)]
    fn local() {
        let tz = Tz::local().unwrap();
        let dt = (&tz).ymd(2019, 2, 13).and_hms(14, 5, 18);
        let cdt = chrono::Local.ymd(2019, 2, 13).and_hms(14, 5, 18);
        assert_eq!(dt.naive_utc(), cdt.naive_utc());
    }
}

#[cfg(all(test, unix))]
mod chrono_tz_tests {
    use super::Tz;
    use chrono::{Duration, TimeZone};
    use lazy_static::lazy_static;

    lazy_static! {
        static ref ADELAIDE: Tz = Tz::named("Australia/Adelaide").unwrap();
        static ref APIA: Tz = Tz::named("Pacific/Apia").unwrap();
        static ref AMSTERDAM: Tz = Tz::named("Europe/Amsterdam").unwrap();
        static ref BERLIN: Tz = Tz::named("Europe/Berlin").unwrap();
        static ref DANMARKSHAVN: Tz = Tz::named("America/Danmarkshavn").unwrap();
        static ref DHAKA: Tz = Tz::named("Asia/Dhaka").unwrap();
        static ref EASTERN: Tz = Tz::named("US/Eastern").unwrap();
        static ref GAZA: Tz = Tz::named("Asia/Gaza").unwrap();
        static ref JERUSALEM: Tz = Tz::named("Asia/Jerusalem").unwrap();
        static ref KATHMANDU: Tz = Tz::named("Asia/Kathmandu").unwrap();
        static ref LONDON: Tz = Tz::named("Europe/London").unwrap();
        static ref MOSCOW: Tz = Tz::named("Europe/Moscow").unwrap();
        static ref NEW_YORK: Tz = Tz::named("America/New_York").unwrap();
        static ref TAHITI: Tz = Tz::named("Pacific/Tahiti").unwrap();
        static ref NOUMEA: Tz = Tz::named("Pacific/Noumea").unwrap();
        static ref TRIPOLI: Tz = Tz::named("Africa/Tripoli").unwrap();
        static ref UTC: Tz = Tz::named("Etc/UTC").unwrap();
        static ref VILNIUS: Tz = Tz::named("Europe/Vilnius").unwrap();
        static ref WARSAW: Tz = Tz::named("Europe/Warsaw").unwrap();
    }

    #[test]
    fn london_to_berlin() {
        let dt = (&*LONDON).ymd(2016, 10, 8).and_hms(17, 0, 0);
        let converted = dt.with_timezone(&&*BERLIN);
        let expected = (&*BERLIN).ymd(2016, 10, 8).and_hms(18, 0, 0);
        assert_eq!(converted, expected);
    }

    #[test]
    fn us_eastern_dst_commutativity() {
        let dt = (&*UTC).ymd(2002, 4, 7).and_hms(7, 0, 0);
        for days in -420..720 {
            let dt1 = (dt.clone() + Duration::days(days)).with_timezone(&&*EASTERN);
            let dt2 = dt.with_timezone(&&*EASTERN) + Duration::days(days);
            assert_eq!(dt1, dt2);
        }
    }

    #[test]
    fn warsaw_tz_name() {
        let dt = (&*UTC).ymd(1915, 8, 4).and_hms(22, 35, 59);
        assert_eq!(dt.with_timezone(&&*WARSAW).format("%Z").to_string(), "WMT");
        let dt = dt + Duration::seconds(1);
        assert_eq!(dt.with_timezone(&&*WARSAW).format("%Z").to_string(), "CET");
    }

    #[test]
    fn vilnius_utc_offset() {
        let dt = (&*UTC)
            .ymd(1916, 12, 31)
            .and_hms(22, 35, 59)
            .with_timezone(&&*VILNIUS);
        assert_eq!(dt, (&*VILNIUS).ymd(1916, 12, 31).and_hms(23, 59, 59));
        let dt = dt + Duration::seconds(1);
        assert_eq!(dt, (&*VILNIUS).ymd(1917, 1, 1).and_hms(0, 11, 36));
    }

    #[test]
    fn victorian_times() {
        let dt = (&*UTC)
            .ymd(1847, 12, 1)
            .and_hms(0, 1, 14)
            .with_timezone(&&*LONDON);
        assert_eq!(dt, (&&*LONDON).ymd(1847, 11, 30).and_hms(23, 59, 59));
        let dt = dt + Duration::seconds(1);
        assert_eq!(dt, (&&*LONDON).ymd(1847, 12, 1).and_hms(0, 1, 15));
    }

    #[test]
    fn london_dst() {
        let dt = (&*LONDON).ymd(2016, 3, 10).and_hms(5, 0, 0);
        let later = dt + Duration::days(180);
        let expected = (&*LONDON).ymd(2016, 9, 6).and_hms(6, 0, 0);
        assert_eq!(later, expected);
    }

    #[test]
    fn international_date_line_change() {
        let dt = (&*UTC)
            .ymd(2011, 12, 30)
            .and_hms(9, 59, 59)
            .with_timezone(&&*APIA);
        assert_eq!(dt, (&*APIA).ymd(2011, 12, 29).and_hms(23, 59, 59));
        let dt = dt + Duration::seconds(1);
        assert_eq!(dt, (&*APIA).ymd(2011, 12, 31).and_hms(0, 0, 0));
    }

    #[test]
    fn negative_offset_with_minutes_and_seconds() {
        let dt = (&*UTC)
            .ymd(1900, 1, 1)
            .and_hms(12, 0, 0)
            .with_timezone(&&*DANMARKSHAVN);
        assert_eq!(dt, (&*DANMARKSHAVN).ymd(1900, 1, 1).and_hms(10, 45, 20));
    }

    #[test]
    fn monotonicity() {
        let mut dt = (&*NOUMEA).ymd(1800, 1, 1).and_hms(12, 0, 0);
        for _ in 0..24 * 356 * 400 {
            let new = dt.clone() + Duration::hours(1);
            assert!(new > dt);
            assert!(new.with_timezone(&&*UTC) > dt.with_timezone(&&*UTC));
            dt = new;
        }
    }

    fn test_inverse<T: TimeZone>(tz: T, begin: i32, end: i32) {
        for y in begin..end {
            for d in 1..366 {
                for h in 0..24 {
                    for m in 0..60 {
                        let dt = (&*UTC).yo(y, d).and_hms(h, m, 0);
                        let with_tz = dt.with_timezone(&tz);
                        let utc = with_tz.with_timezone(&&*UTC);
                        assert_eq!(dt, utc);
                    }
                }
            }
        }
    }

    #[test]
    fn inverse_london() {
        test_inverse(&*LONDON, 1989, 1994);
    }

    #[test]
    fn inverse_dhaka() {
        test_inverse(&*DHAKA, 1995, 2000);
    }

    #[test]
    fn inverse_apia() {
        test_inverse(&*APIA, 2011, 2012);
    }

    #[test]
    fn inverse_tahiti() {
        test_inverse(&*TAHITI, 1911, 1914);
    }

    #[test]
    fn string_representation() {
        let dt = (&*UTC)
            .ymd(2000, 9, 1)
            .and_hms(12, 30, 15)
            .with_timezone(&&*ADELAIDE);
        assert_eq!(dt.to_string(), "2000-09-01 22:00:15 ACST");
        assert_eq!(format!("{:?}", dt), "2000-09-01T22:00:15ACST");
        assert_eq!(dt.to_rfc3339(), "2000-09-01T22:00:15+09:30");
        assert_eq!(format!("{}", dt), "2000-09-01 22:00:15 ACST");
    }

    #[test]
    fn tahiti() {
        let dt = (&*UTC)
            .ymd(1912, 10, 1)
            .and_hms(9, 58, 16)
            .with_timezone(&&*TAHITI);
        let before = dt.clone() - Duration::hours(1);
        assert_eq!(before, (&*TAHITI).ymd(1912, 9, 30).and_hms(23, 0, 0));
        let after = dt + Duration::hours(1);
        assert_eq!(after, (&*TAHITI).ymd(1912, 10, 1).and_hms(0, 58, 16));
    }

    #[test]
    fn second_offsets() {
        let dt = (&*UTC)
            .ymd(1914, 1, 1)
            .and_hms(13, 40, 28)
            .with_timezone(&&*AMSTERDAM);
        assert_eq!(dt.to_string(), "1914-01-01 14:00:00 AMT");
        assert_eq!(dt.to_rfc3339(), "1914-01-01T14:00:00+00:19");
    }

    #[test]
    #[should_panic]
    fn nonexistent_time() {
        let _ = (&*LONDON).ymd(2016, 3, 27).and_hms(1, 30, 0);
    }

    #[test]
    #[should_panic]
    fn nonexistent_time_2() {
        let _ = (&*LONDON).ymd(2016, 3, 27).and_hms(1, 0, 0);
    }

    #[test]
    fn time_exists() {
        let _ = (&*LONDON).ymd(2016, 3, 27).and_hms(2, 0, 0);
    }

    #[test]
    #[should_panic]
    fn ambiguous_time() {
        let _ = (&*LONDON).ymd(2016, 10, 30).and_hms(1, 0, 0);
    }

    #[test]
    #[should_panic]
    fn ambiguous_time_2() {
        let _ = (&*LONDON).ymd(2016, 10, 30).and_hms(1, 30, 0);
    }

    #[test]
    #[should_panic]
    fn ambiguous_time_3() {
        let _ = (&*MOSCOW).ymd(2014, 10, 26).and_hms(1, 30, 0);
    }

    #[test]
    #[should_panic]
    fn ambiguous_time_4() {
        let _ = (&*MOSCOW).ymd(2014, 10, 26).and_hms(1, 0, 0);
    }

    #[test]
    fn unambiguous_time() {
        let _ = (&*LONDON).ymd(2016, 10, 30).and_hms(2, 0, 0);
    }

    #[test]
    fn unambiguous_time_2() {
        let _ = (&*MOSCOW).ymd(2014, 10, 26).and_hms(2, 0, 0);
    }

    // the numberphile tests

    #[test]
    fn test_london_5_days_ago_to_new_york() {
        let from = (&*LONDON).ymd(2013, 12, 25).and_hms(14, 0, 0);
        let to = (&*NEW_YORK).ymd(2013, 12, 30).and_hms(14, 0, 0);
        assert_eq!(
            to.signed_duration_since(from),
            Duration::days(5) + Duration::hours(5)
        );
    }

    #[test]
    fn london_to_australia() {
        // at the time Tom was speaking, Adelaide was 10 1/2 hours ahead
        // many other parts of Australia use different time zones
        let from = (&*LONDON).ymd(2013, 12, 25).and_hms(14, 0, 0);
        let to = (&*ADELAIDE).ymd(2013, 12, 30).and_hms(14, 0, 0);
        assert_eq!(
            to.signed_duration_since(from),
            Duration::days(5) - Duration::minutes(630)
        );
    }

    #[test]
    fn london_to_nepal() {
        // note Tom gets this wrong, it's 5 3/4 hours as he is speaking
        let from = (&*LONDON).ymd(2013, 12, 25).and_hms(14, 0, 0);
        let to = (&*KATHMANDU).ymd(2013, 12, 30).and_hms(14, 0, 0);
        assert_eq!(
            to.signed_duration_since(from),
            Duration::days(5) - Duration::minutes(345)
        );
    }

    #[test]
    fn autumn() {
        let from = (&*LONDON).ymd(2013, 10, 25).and_hms(12, 0, 0);
        let to = (&*LONDON).ymd(2013, 11, 1).and_hms(12, 0, 0);
        assert_eq!(
            to.signed_duration_since(from),
            Duration::days(7) + Duration::hours(1)
        );
    }

    #[test]
    fn earlier_daylight_savings_in_new_york() {
        let from = (&*NEW_YORK).ymd(2013, 10, 25).and_hms(12, 0, 0);
        let to = (&*NEW_YORK).ymd(2013, 11, 1).and_hms(12, 0, 0);
        assert_eq!(to.signed_duration_since(from), Duration::days(7));
    }

    #[test]
    fn southern_hemisphere_clocks_forward() {
        let from = (&*ADELAIDE).ymd(2013, 10, 1).and_hms(12, 0, 0);
        let to = (&*ADELAIDE).ymd(2013, 11, 1).and_hms(12, 0, 0);
        assert_eq!(
            to.signed_duration_since(from),
            Duration::days(31) - Duration::hours(1)
        );
    }

    #[test]
    fn samoa_skips_a_day() {
        let from = (&*APIA).ymd(2011, 12, 29).and_hms(12, 0, 0);
        let to = (&*APIA).ymd(2011, 12, 31).and_hms(12, 0, 0);
        assert_eq!(to.signed_duration_since(from), Duration::days(1));
    }

    #[test]
    fn double_bst() {
        let from = (&*LONDON).ymd(1942, 6, 1).and_hms(12, 0, 0);
        let to = (&*UTC).ymd(1942, 6, 1).and_hms(12, 0, 0);
        assert_eq!(to.signed_duration_since(from), Duration::hours(2));
    }

    #[test]
    fn libya_2013() {
        // Libya actually put their clocks *forward* in 2013, but not in any other year
        let from = (&*TRIPOLI).ymd(2012, 3, 1).and_hms(12, 0, 0);
        let to = (&*TRIPOLI).ymd(2012, 4, 1).and_hms(12, 0, 0);
        assert_eq!(to.signed_duration_since(from), Duration::days(31));

        let from = (&*TRIPOLI).ymd(2013, 3, 1).and_hms(12, 0, 0);
        let to = (&*TRIPOLI).ymd(2013, 4, 1).and_hms(12, 0, 0);
        assert_eq!(
            to.signed_duration_since(from),
            Duration::days(31) - Duration::hours(1)
        );

        let from = (&*TRIPOLI).ymd(2014, 3, 1).and_hms(12, 0, 0);
        let to = (&*TRIPOLI).ymd(2014, 4, 1).and_hms(12, 0, 0);
        assert_eq!(to.signed_duration_since(from), Duration::days(31));
    }

    #[test]
    fn israel_palestine() {
        let from = (&*JERUSALEM).ymd(2016, 10, 29).and_hms(12, 0, 0);
        let to = (&*GAZA).ymd(2016, 10, 29).and_hms(12, 0, 0);
        assert_eq!(to.signed_duration_since(from), Duration::hours(1));
    }

    #[test]
    fn leapsecond() {
        let from = (&*UTC).ymd(2016, 6, 30).and_hms(23, 59, 59);
        let to = (&*UTC).ymd(2016, 6, 30).and_hms_milli(23, 59, 59, 1000);
        assert_eq!(to.signed_duration_since(from), Duration::seconds(1));
    }
}
