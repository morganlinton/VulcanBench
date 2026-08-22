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

use crate::binary_search::binary_search;
use crate::{Offset, OffsetResult, TimeZone};
use std::cmp::Ordering;
use std::ops::Index;
use time::{OffsetDateTime, UtcOffset};

//Inspired from https://github.com/chronotope/chrono-tz/blob/main/src/timezone_impl.rs

struct Span {
    start: Option<i64>,
    end: Option<i64>,
}

impl Span {
    fn contains(&self, x: i64) -> bool {
        match (self.start, self.end) {
            (Some(a), Some(b)) if a <= x && x < b => true,
            (Some(a), None) if a <= x => true,
            (None, Some(b)) if b > x => true,
            (None, None) => true,
            _ => false,
        }
    }

    fn cmp(&self, x: i64) -> Ordering {
        match (self.start, self.end) {
            (Some(a), Some(b)) if a <= x && x < b => Ordering::Equal,
            (Some(a), Some(b)) if a <= x && b <= x => Ordering::Less,
            (Some(_), Some(_)) => Ordering::Greater,
            (Some(a), None) if a <= x => Ordering::Equal,
            (Some(_), None) => Ordering::Greater,
            (None, Some(b)) if b <= x => Ordering::Less,
            (None, Some(_)) => Ordering::Equal,
            (None, None) => Ordering::Equal,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct FixedTimespan {
    pub utc_offset: i64,
    pub dst_offset: i64,
    pub name: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FixedTimespanSet {
    pub name: &'static str,
    pub first: FixedTimespan,
    pub others: &'static [(i64, FixedTimespan)],
}

impl FixedTimespanSet {
    fn len(&self) -> usize {
        1 + self.others.len()
    }

    fn span_utc(&self, i: usize) -> Span {
        let start = match i {
            0 => None,
            _ => Some(self.others[i - 1].0),
        };
        let end = if i >= self.others.len() {
            None
        } else {
            Some(self.others[i].0)
        };
        Span { start, end }
    }

    fn span_local(&self, i: usize) -> Span {
        let start = match i {
            0 => None,
            _ => Some(&self.others[i - 1]),
        }
        .map(|(i, v)| i + v.utc_offset + v.dst_offset);
        let end = if i >= self.others.len() {
            None
        } else if i == 0 {
            Some(self.others[i].0 + self.first.utc_offset + self.first.dst_offset)
        } else {
            let (_, v) = &self.others[i - 1];
            Some(self.others[i].0 + v.utc_offset + v.dst_offset)
        };
        Span { start, end }
    }
}

impl Index<usize> for FixedTimespanSet {
    type Output = FixedTimespan;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.len());
        match index {
            0 => &self.first,
            _ => &self.others[index - 1].1,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TzOffset {
    timespan: &'static FixedTimespan,
}

impl Offset for TzOffset {
    fn to_utc(&self) -> UtcOffset {
        UtcOffset::from_whole_seconds((self.timespan.utc_offset + self.timespan.dst_offset) as i32)
            .unwrap()
    }

    fn name(&self) -> &str {
        self.timespan.name
    }

    fn is_dst(&self) -> bool {
        self.timespan.dst_offset > 0
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Tz {
    set: &'static FixedTimespanSet,
}

impl TimeZone for Tz {
    type Offset = TzOffset;

    fn get_offset_utc(&self, date_time: &OffsetDateTime) -> TzOffset {
        let timestamp = date_time.unix_timestamp();
        let index =
            binary_search(0, self.set.len(), |i| self.set.span_utc(i).cmp(timestamp)).unwrap();
        TzOffset {
            timespan: &self.set[index],
        }
    }

    fn get_offset_local(&self, date_time: &OffsetDateTime) -> OffsetResult<Self::Offset> {
        let timestamp = date_time.unix_timestamp();
        if let Some(i) = binary_search(0, self.set.len(), |i| self.set.span_local(i).cmp(timestamp))
        {
            return if self.set.len() == 1 {
                OffsetResult::Some(TzOffset {
                    timespan: &self.set[i],
                })
            } else if i == 0 && self.set.span_local(1).contains(timestamp) {
                OffsetResult::Ambiguous(
                    TzOffset {
                        timespan: &self.set[0],
                    },
                    TzOffset {
                        timespan: &self.set[1],
                    },
                )
            } else if i == 0 {
                OffsetResult::Some(TzOffset {
                    timespan: &self.set[0],
                })
            } else if self.set.span_local(i - 1).contains(timestamp) {
                OffsetResult::Ambiguous(
                    TzOffset {
                        timespan: &self.set[i - 1],
                    },
                    TzOffset {
                        timespan: &self.set[i],
                    },
                )
            } else if i == self.set.len() - 1 {
                OffsetResult::Some(TzOffset {
                    timespan: &self.set[i],
                })
            } else if self.set.span_local(i + 1).contains(timestamp) {
                OffsetResult::Ambiguous(
                    TzOffset {
                        timespan: &self.set[i],
                    },
                    TzOffset {
                        timespan: &self.set[i + 1],
                    },
                )
            } else {
                OffsetResult::Some(TzOffset {
                    timespan: &self.set[i],
                })
            };
        }
        OffsetResult::None
    }

    fn get_offset_primary(&self) -> Self::Offset {
        TzOffset {
            timespan: &self.set.first,
        }
    }

    fn name(&self) -> &str {
        self.set.name
    }
}

pub const fn internal_tz_new(set: &'static FixedTimespanSet) -> Tz {
    Tz { set }
}
