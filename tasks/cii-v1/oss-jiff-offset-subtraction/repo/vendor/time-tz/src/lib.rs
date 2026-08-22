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

//! This provides traits and utilities to work with timezones to time-rs and additionally has an
//! implementation of IANA timezone database. To disable the integrated IANA/windows databases,
//! one can simply remove the `db` default feature.

// See https://doc.rust-lang.org/beta/unstable-book/language-features/doc-cfg.html & https://github.com/rust-lang/rust/pull/89596
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

use time::{OffsetDateTime, PrimitiveDateTime};

mod sealing {
    pub trait OffsetDateTimeExt {}
    pub trait PrimitiveDateTimeExt {}

    impl OffsetDateTimeExt for time::OffsetDateTime {}
    impl PrimitiveDateTimeExt for time::PrimitiveDateTime {}
}

// This trait is sealed and is only implemented in this library.
pub trait OffsetDateTimeExt: sealing::OffsetDateTimeExt {
    /// Converts this OffsetDateTime to a different TimeZone.
    fn to_timezone<T: TimeZone>(&self, tz: &T) -> OffsetDateTime;
}

/// This trait is sealed and is only implemented in this library.
pub trait PrimitiveDateTimeExt: sealing::PrimitiveDateTimeExt {
    /// Creates a new OffsetDateTime from a PrimitiveDateTime by assigning the main offset of the
    /// target timezone.
    ///
    /// *This assumes the PrimitiveDateTime is already in the target timezone.*
    ///
    /// # Arguments
    ///
    /// * `tz`: the target timezone.
    ///
    /// returns: `OffsetResult<OffsetDateTime>`
    fn assume_timezone<T: TimeZone>(&self, tz: &T) -> OffsetResult<OffsetDateTime>;

    /// Creates a new OffsetDateTime with the proper offset in the given timezone.
    ///
    /// *This assumes the PrimitiveDateTime is in UTC offset.*
    ///
    /// # Arguments
    ///
    /// * `tz`: the target timezone.
    ///
    /// returns: OffsetDateTime
    fn assume_timezone_utc<T: TimeZone>(&self, tz: &T) -> OffsetDateTime;
}

impl PrimitiveDateTimeExt for PrimitiveDateTime {
    fn assume_timezone<T: TimeZone>(&self, tz: &T) -> OffsetResult<OffsetDateTime> {
        match tz.get_offset_local(&self.assume_utc()) {
            OffsetResult::Some(a) => OffsetResult::Some(self.assume_offset(a.to_utc())),
            OffsetResult::Ambiguous(a, b) => OffsetResult::Ambiguous(
                self.assume_offset(a.to_utc()),
                self.assume_offset(b.to_utc()),
            ),
            OffsetResult::None => OffsetResult::None,
        }
    }

    fn assume_timezone_utc<T: TimeZone>(&self, tz: &T) -> OffsetDateTime {
        let offset = tz.get_offset_utc(&self.assume_utc());
        self.assume_offset(offset.to_utc())
    }
}

impl OffsetDateTimeExt for OffsetDateTime {
    fn to_timezone<T: TimeZone>(&self, tz: &T) -> OffsetDateTime {
        let offset = tz.get_offset_utc(self);
        self.to_offset(offset.to_utc())
    }
}

mod binary_search;
mod interface;
mod timezone_impl;

#[cfg(feature = "db")]
pub mod timezones;

pub use interface::*;

#[cfg(feature = "system")]
pub mod system;

#[cfg(feature = "posix-tz")]
pub mod posix_tz;

#[cfg(feature = "db")]
pub use timezone_impl::Tz;

#[cfg(test)]
mod tests {
    use crate::timezones;
    use crate::Offset;
    use crate::OffsetDateTimeExt;
    use crate::PrimitiveDateTimeExt;
    use crate::TimeZone;
    use time::macros::{datetime, offset};
    use time::OffsetDateTime;

    #[test]
    fn names() {
        //This test verifies that windows timezone names work fine.
        let shanghai = timezones::get_by_name("Asia/Shanghai");
        let china = timezones::get_by_name("China Standard Time");
        assert!(shanghai.is_some());
        assert!(china.is_some());
        assert_eq!(shanghai, china);
    }

    #[test]
    fn find() {
        let zones_iana = timezones::find_by_name("Asia");
        //let zones_win = timezones::find_by_name("China Standard Time");
        assert!(zones_iana.len() > 1);
        //assert!(zones_win.len() > 1);
    }

    #[test]
    fn offsets_and_name() {
        let tz = timezones::db::europe::LONDON;
        assert_eq!(tz.name(), "Europe/London");
        let offset = tz.get_offset_utc(&OffsetDateTime::now_utc());
        assert!(!offset.name().is_empty());
    }

    #[test]
    fn london_to_berlin() {
        let dt = datetime!(2016-10-8 17:0:0).assume_timezone_utc(timezones::db::europe::LONDON);
        let converted = dt.to_timezone(timezones::db::europe::BERLIN);
        let expected =
            datetime!(2016-10-8 18:0:0).assume_timezone_utc(timezones::db::europe::BERLIN);
        assert_eq!(converted, expected);
    }

    #[test]
    fn dst() {
        let london = timezones::db::europe::LONDON;
        let odt1 = datetime!(2021-01-01 12:0:0 UTC);
        assert_eq!(odt1.to_timezone(london), datetime!(2021-01-01 12:0:0 +0));
        let odt2 = datetime!(2021-07-01 12:0:0 UTC);
        // Adding offset to datetime call causes VERY surprising result: hours randomly changes!!
        // When using UTC followed by .to_offset no surprising result.
        assert_eq!(
            odt2.to_timezone(london),
            datetime!(2021-07-01 12:0:0 UTC).to_offset(offset!(+1))
        );
    }

    #[test]
    fn handles_forward_changeover() {
        assert_eq!(
            datetime!(2022-03-27 01:30)
                .assume_timezone(timezones::db::CET)
                .unwrap(),
            datetime!(2022-03-27 01:30 +01:00)
        );
    }

    #[test]
    fn handles_after_changeover() {
        assert_eq!(
            datetime!(2022-03-27 03:30)
                .assume_timezone(timezones::db::CET)
                .unwrap(),
            datetime!(2022-03-27 03:30 +02:00)
        );
    }

    #[test]
    fn handles_broken_time() {
        assert!(datetime!(2022-03-27 02:30)
            .assume_timezone(timezones::db::CET)
            .is_none());
    }

    #[test]
    fn handles_backward_changeover() {
        // During backward changeover, the hour between 02:00 and 03:00 occurs twice, so either answer is correct
        assert_eq!(
            datetime!(2022-10-30 02:30)
                .assume_timezone(timezones::db::CET)
                .unwrap_first(),
            datetime!(2022-10-30 02:30 +02:00)
        );
        assert_eq!(
            datetime!(2022-10-30 02:30)
                .assume_timezone(timezones::db::CET)
                .unwrap_second(),
            datetime!(2022-10-30 02:30 +01:00)
        );
    }
}
