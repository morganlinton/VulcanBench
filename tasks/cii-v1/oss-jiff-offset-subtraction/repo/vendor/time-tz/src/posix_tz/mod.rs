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

use crate::{Offset, TimeZone, Tz};
use std::fmt::{Display, Formatter};
use thiserror::Error;
use time::{OffsetDateTime, UtcOffset};

mod r#abstract;
mod intermediate;
mod parser;

/// A range error returned when a field is out of the range defined in POSIX.
#[derive(Debug)]
pub enum RangeError {
    /// One of the time field in the given string was out of range.
    Time,

    /// One of the date field in the given string was out of range.
    Date,
}

impl Display for RangeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RangeError::Time => f.write_str("time field out of range"),
            RangeError::Date => f.write_str("date field out of range"),
        }
    }
}

/// The main type of error that is returned when a TZ POSIX string fails to parse.
#[derive(Debug, Error)]
pub enum ParseError<'a> {
    /// A nom parsing error.
    #[error("nom error: {:?}", .0)]
    Nom(nom::error::ErrorKind),

    /// In case a short format was given, the POSIX standard doesn't define what to do,
    /// in this implementation we just try to match the first tzdb timezone containing the
    /// short name; if none could be found this error variant is returned.
    #[error("unknown short timezone name `{0}`")]
    UnknownName(&'a str),

    /// We've exceeded the range of a field when checking for conformance against the POSIX
    /// standard.
    #[error("range error: {0}")]
    Range(RangeError),

    /// We've exceeded the range of a date component when converting types to time-rs.
    #[error("time component range error: {0}")]
    ComponentRange(time::error::ComponentRange),
}

/// The type of error return when a given Offset/PrimitiveDateTime cannot be represented
/// in a given POSIX TZ formatted "timezone".
#[derive(Debug, Error)]
pub enum Error {
    /// We've exceeded the range of a date component when converting types to time-rs.
    #[error("time component range error: {0}")]
    ComponentRange(time::error::ComponentRange),

    /// We've exceeded the maximum date supported by time-rs.
    #[error("value of Date too large")]
    DateTooLarge,
}

/// A POSIX "timezone" offset.
pub struct PosixTzOffset<'a> {
    inner: r#abstract::TzOrExpandedOffset<'a>,
}

impl<'a> Offset for PosixTzOffset<'a> {
    fn to_utc(&self) -> UtcOffset {
        match &self.inner {
            r#abstract::TzOrExpandedOffset::Expanded(v) => v.to_utc(),
            r#abstract::TzOrExpandedOffset::Tz(v) => v.to_utc(),
        }
    }

    fn name(&self) -> &str {
        match &self.inner {
            r#abstract::TzOrExpandedOffset::Expanded(v) => v.name(),
            r#abstract::TzOrExpandedOffset::Tz(v) => v.name(),
        }
    }

    fn is_dst(&self) -> bool {
        match &self.inner {
            r#abstract::TzOrExpandedOffset::Expanded(v) => v.is_dst(),
            r#abstract::TzOrExpandedOffset::Tz(v) => v.is_dst(),
        }
    }
}

/// A "timezone" in POSIX TZ format.
pub struct PosixTz<'a> {
    inner: r#abstract::TzOrExpanded<'a>,
}

impl<'a> PosixTz<'a> {
    /// Parse the given POSIX TZ string.
    ///
    /// # Arguments
    ///
    /// * `input`: the string to parse.
    ///
    /// returns: Result<PosixTz, ParseError>
    ///
    /// # Errors
    ///
    /// Returns a [ParseError](crate::posix_tz::ParseError) if the given string is not a valid
    /// POSIX "timezone".
    pub fn parse(input: &'a str) -> Result<PosixTz<'a>, ParseError> {
        let intermediate = intermediate::parse_intermediate(input)?;
        let inner = r#abstract::parse_abstract(intermediate)?;
        Ok(PosixTz { inner })
    }

    /// Convert the given date_time to this "timezone".
    ///
    /// # Arguments
    ///
    /// * `date_time`: the date time to convert.
    ///
    /// returns: Result<OffsetDateTime, Error>
    ///
    /// # Errors
    ///
    /// Returns an [Error](crate::posix_tz::Error) if the date_time cannot be represented in this
    /// "timezone".
    pub fn convert(&self, date_time: &OffsetDateTime) -> Result<OffsetDateTime, Error> {
        let offset = self.get_offset(date_time)?;
        Ok(date_time.to_offset(offset.to_utc()))
    }

    /// Calculates the offset to add to the given date_time to convert it to this "timezone".
    ///
    /// # Arguments
    ///
    /// * `date_time`: the date time to calculate offset of.
    ///
    /// returns: Result<OffsetDateTime, Error>
    ///
    /// # Errors
    ///
    /// Returns an [Error](crate::posix_tz::Error) if the date_time cannot be represented in this
    /// "timezone".
    pub fn get_offset(&self, date_time: &OffsetDateTime) -> Result<PosixTzOffset<'a>, Error> {
        Ok(match &self.inner {
            r#abstract::TzOrExpanded::Tz(v) => PosixTzOffset {
                inner: r#abstract::TzOrExpandedOffset::Tz(v.get_offset_utc(date_time)),
            },
            r#abstract::TzOrExpanded::Expanded(v) => PosixTzOffset {
                inner: r#abstract::TzOrExpandedOffset::Expanded(v.get_offset_utc(date_time)?),
            },
        })
    }

    /// Gets the current time in this "timezone".
    ///
    /// returns: Result<OffsetDateTime, Error>
    ///
    /// # Errors
    ///
    /// Returns an [Error](crate::posix_tz::Error) if the current time cannot be represented in
    /// this "timezone".
    pub fn now(&self) -> Result<OffsetDateTime, Error> {
        self.convert(&OffsetDateTime::now_utc())
    }

    /// Returns the precise IANA TimeZone associated to this POSIX "timezone".
    ///
    /// If this POSIX "timezone" is not a precise IANA TimeZone, None is returned.
    pub fn as_iana(&self) -> Option<&'static Tz> {
        match &self.inner {
            r#abstract::TzOrExpanded::Tz(v) => Some(*v),
            r#abstract::TzOrExpanded::Expanded(_) => None,
        }
    }
}
