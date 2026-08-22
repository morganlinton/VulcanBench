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

use crate::posix_tz::intermediate::ParsedTz;
use crate::posix_tz::parser::Date;
use crate::posix_tz::{Error, ParseError};
use crate::timezone_impl::TzOffset;
use crate::Tz;
use time::{OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

pub enum TzOrExpandedOffset<'a> {
    Expanded(ExpandedTzOffset<'a>),
    Tz(TzOffset),
}

pub struct ExpandedTzOffset<'a> {
    mode: ExpandedMode<'a>,
    is_dst: bool,
}

impl<'a> crate::Offset for ExpandedTzOffset<'a> {
    fn to_utc(&self) -> UtcOffset {
        self.mode.offset
    }

    fn name(&self) -> &str {
        self.mode.name
    }

    fn is_dst(&self) -> bool {
        self.is_dst
    }
}

#[derive(Copy, Clone)]
pub struct ExpandedMode<'a> {
    name: &'a str,
    offset: UtcOffset, //There's always an offset, even if not defined it's assumed to be +1.
}

pub struct Rule {
    //Pre-compute the rule dates
    start: (Date, Time),
    end: (Date, Time),
}

pub struct ExpandedTz<'a> {
    std: ExpandedMode<'a>,
    dst: Option<ExpandedMode<'a>>,
    rule: Option<Rule>,
}

impl<'a> ExpandedTz<'a> {
    pub fn get_offset_utc(
        &self,
        date_time: &OffsetDateTime,
    ) -> Result<ExpandedTzOffset<'a>, Error> {
        match self.dst {
            None => Ok(ExpandedTzOffset {
                mode: self.std,
                is_dst: false,
            }),
            Some(dst) => match &self.rule {
                None => {
                    use crate::Offset;
                    use crate::TimeZone;
                    let timezone = crate::timezones::db::america::NEW_YORK;
                    let tz_offset = timezone.get_offset_utc(date_time);
                    Ok(ExpandedTzOffset {
                        mode: if tz_offset.is_dst() { dst } else { self.std },
                        is_dst: tz_offset.is_dst(),
                    })
                }
                Some(rule) => {
                    let start = PrimitiveDateTime::new(
                        rule.start.0.to_date(date_time.year())?,
                        rule.start.1,
                    )
                    .assume_offset(self.std.offset);
                    let end =
                        PrimitiveDateTime::new(rule.end.0.to_date(date_time.year())?, rule.end.1)
                            .assume_offset(dst.offset);
                    if date_time >= &start && date_time < &end {
                        Ok(ExpandedTzOffset {
                            mode: dst,
                            is_dst: true,
                        })
                    } else {
                        Ok(ExpandedTzOffset {
                            mode: self.std,
                            is_dst: false,
                        })
                    }
                }
            },
        }
    }
}

pub enum TzOrExpanded<'a> {
    Tz(&'static Tz),
    Expanded(ExpandedTz<'a>),
}

pub fn parse_abstract(input: ParsedTz) -> Result<TzOrExpanded, ParseError> {
    match input {
        ParsedTz::Existing(v) => Ok(TzOrExpanded::Tz(v)),
        ParsedTz::Expanded((std, dst)) => {
            //Take the oposite of offset because POSIX assumes it at inverse:
            // local + offset = utc instead of utc + offset = local.
            let tmp = -std.offset.to_seconds();
            let std_offset =
                UtcOffset::from_whole_seconds(tmp).map_err(ParseError::ComponentRange)?;
            let std = ExpandedMode {
                name: std.name,
                offset: std_offset,
            };
            let (dst, rule) = match dst {
                None => (None, None),
                Some(v) => {
                    // If no offset is specified the POSIX standard defines +1 hour in standard
                    // time as default.
                    let offset = v.offset.map(|v| -v.to_seconds()).unwrap_or(tmp + 3600);
                    let dst_offset = UtcOffset::from_whole_seconds(offset)
                        .map_err(ParseError::ComponentRange)?;
                    let rule = match &v.rule {
                        None => None,
                        Some(v) => {
                            // SAFETY: This must be safe as never ever depends on user input.
                            let default =
                                unsafe { time::Time::from_hms(2, 0, 0).unwrap_unchecked() };
                            let start_time =
                                v.start.1.as_ref().map(|v| v.to_time()).unwrap_or(default);
                            let end_time = v.end.1.as_ref().map(|v| v.to_time()).unwrap_or(default);
                            Some(Rule {
                                start: (v.start.0, start_time),
                                end: (v.end.0, end_time),
                            })
                        }
                    };
                    (
                        Some(ExpandedMode {
                            name: v.name,
                            offset: dst_offset,
                        }),
                        rule,
                    )
                }
            };
            Ok(TzOrExpanded::Expanded(ExpandedTz { std, dst, rule }))
        }
    }
}
