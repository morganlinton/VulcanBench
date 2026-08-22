// Copyright (c) 2022, Yuri6037
//
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
// * Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
// * Neither the name of time-tz nor the names of its contributors
// may be used to endorse or promote products derived from this software
// without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
// CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
// EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
// PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
// PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
// LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
// NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
// SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use super::parser::Date;
use super::parser::Offset;
use super::parser::Time;
use crate::posix_tz::parser::{entry, Dst, Std, Tz};
use crate::posix_tz::{Error, ParseError, RangeError};
use nom::Err;
use time::{Duration, Month, Weekday};

impl Time {
    fn to_seconds(&self) -> u32 {
        self.hh as u32 * 3600 + self.mm.unwrap_or(0) as u32 * 60 + self.ss.unwrap_or(0) as u32
    }

    pub fn to_time(&self) -> time::Time {
        // Here unwrap should always pass because component range is checked in is_valid_range.
        time::Time::from_hms(self.hh, self.mm.unwrap_or(0), self.ss.unwrap_or(0)).unwrap()
    }

    fn is_valid_range(&self) -> bool {
        self.hh <= 24 && self.mm.unwrap_or(0) <= 59 && self.ss.unwrap_or(0) <= 59
    }
}

impl Offset {
    pub fn to_seconds(&self) -> i32 {
        match self.positive {
            true => self.time.to_seconds() as i32,
            false => -(self.time.to_seconds() as i32),
        }
    }
}

impl Date {
    pub fn to_date(self, year: i32) -> Result<time::Date, Error> {
        match self {
            Date::J(n) => {
                // Hack: the basic idea is 2021 was not a leap year so february only
                // contains 28 days instead of 29 which matches the POSIX spec.
                let date = time::Date::from_ordinal_date(2021, n).map_err(Error::ComponentRange)?;
                // Not sure if that will work in all cases though...
                time::Date::from_calendar_date(year, date.month(), date.day())
                    .map_err(Error::ComponentRange)
            }
            // ComponentRange errors should be prevented by is_valid_range.
            Date::N(n) => time::Date::from_ordinal_date(year, n + 1).map_err(Error::ComponentRange),
            Date::M { m, n, d } => {
                // One more hack: here w're trying to match Date::from_iso_week_date.
                // SAFETY: This always works because m >= 1
                // and m <= 12 (see is_valid_range).
                let month = unsafe { Month::try_from(m).unwrap_unchecked() };
                let day = match d {
                    0 => Weekday::Sunday,
                    1 => Weekday::Monday,
                    2 => Weekday::Tuesday,
                    3 => Weekday::Wednesday,
                    4 => Weekday::Thursday,
                    5 => Weekday::Friday,
                    6 => Weekday::Saturday,
                    // SAFETY: This is basically impossible because d is a u8 so by definition
                    // cannot be < 0 and d <= 6 (see is_valid_range).
                    _ => unsafe { std::hint::unreachable_unchecked() },
                };
                let mut date = time::Date::from_calendar_date(year, month, 1)
                    .map_err(Error::ComponentRange)?;
                while date.weekday() != day {
                    date = date.next_day().ok_or(Error::DateTooLarge)?;
                }
                let next_month = date.month().next();
                // Advance of (n - 1) * 7 days.
                date = date
                    .checked_add(Duration::days((n as i64 - 1) * 7))
                    .ok_or(Error::DateTooLarge)?;
                if n == 5 && date.month() == next_month {
                    date -= Duration::days(7); //Shift back of 7 days.
                }
                Ok(date)
            }
        }
    }

    fn is_valid_range(&self) -> bool {
        match self {
            Date::J(v) => (1..=365).contains(v),
            Date::N(v) => v <= &365,
            Date::M { m, n, d } => d <= &6 && (1..=5).contains(n) && (1..=12).contains(m),
        }
    }
}

impl<'a> Tz<'a> {
    fn ensure_valid_range(&self) -> Result<(), RangeError> {
        if let Tz::Expanded { std, dst } = self {
            if !std.offset.time.is_valid_range() {
                return Err(RangeError::Time);
            }
            if let Some(dst) = dst {
                if let Some(offset) = &dst.offset {
                    if !offset.time.is_valid_range() {
                        return Err(RangeError::Time);
                    }
                }
                if let Some(rule) = &dst.rule {
                    if !rule.start.0.is_valid_range() || !rule.end.0.is_valid_range() {
                        return Err(RangeError::Date);
                    }
                    if let Some(time) = &rule.start.1 {
                        if !time.is_valid_range() {
                            return Err(RangeError::Time);
                        }
                    }
                    if let Some(time) = &rule.end.1 {
                        if !time.is_valid_range() {
                            return Err(RangeError::Time);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub enum ParsedTz<'a> {
    Existing(&'static crate::Tz),
    Expanded((Std<'a>, Option<Dst<'a>>)),
}

pub fn parse_intermediate(input: &str) -> Result<ParsedTz, ParseError> {
    let (_, inner) = entry(input).map_err(|v| match v {
        Err::Incomplete(_) => {
            panic!("According to nom docs this case is impossible with complete API.")
        }
        Err::Error(e) => ParseError::Nom(e.code),
        Err::Failure(e) => ParseError::Nom(e.code),
    })?;
    inner.ensure_valid_range().map_err(ParseError::Range)?;
    Ok(match inner {
        Tz::Short(name) => {
            let tz = crate::timezones::find_by_name(name)
                .first()
                .copied()
                .ok_or(ParseError::UnknownName(name))?;
            ParsedTz::Existing(tz)
        }
        Tz::Expanded { std, dst } => ParsedTz::Expanded((std, dst)),
    })
}
