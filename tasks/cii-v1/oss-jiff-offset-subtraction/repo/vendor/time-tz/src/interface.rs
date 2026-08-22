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

use time::{OffsetDateTime, UtcOffset};

/// This trait represents a particular timezone offset.
pub trait Offset {
    /// Converts this timezone offset to a [UtcOffset](time::UtcOffset).
    fn to_utc(&self) -> UtcOffset;

    /// Returns the name of this offset.
    fn name(&self) -> &str;

    /// Returns true if this offset is DST in the corresponding timezone, false otherwise.
    fn is_dst(&self) -> bool;
}

/// This trait represents a timezone provider.
pub trait TimeZone {
    /// The type of offset.
    type Offset: Offset;

    /// Search for the given date time offset (assuming it is UTC) in this timezone.
    fn get_offset_utc(&self, date_time: &OffsetDateTime) -> Self::Offset;

    /// Search for the given date time offset (assuming it is already local) in this timezone.
    fn get_offset_local(&self, date_time: &OffsetDateTime) -> OffsetResult<Self::Offset>;

    /// Gets the main/default offset in this timezone.
    fn get_offset_primary(&self) -> Self::Offset;

    /// Returns the name of this timezone.
    fn name(&self) -> &str;
}

/// This represents the possible types of errors when trying to find a local offset.
#[derive(Clone, Copy, Debug)]
pub enum OffsetResult<T> {
    /// The date time is not ambiguous (exactly 1 is possible).
    Some(T),

    /// The date time is ambiguous (2 are possible).
    Ambiguous(T, T),

    /// The date time is invalid.
    None,
}

impl<T> OffsetResult<T> {
    /// Unwraps this OffsetResult assuming ambiguity is an error.
    pub fn unwrap(self) -> T {
        match self {
            OffsetResult::Some(v) => v,
            OffsetResult::Ambiguous(_, _) => panic!("Attempt to unwrap an ambiguous offset"),
            OffsetResult::None => panic!("Attempt to unwrap an invalid offset"),
        }
    }

    /// Unwraps this OffsetResult resolving ambiguity by taking the first result.
    pub fn unwrap_first(self) -> T {
        match self {
            OffsetResult::Some(v) => v,
            OffsetResult::Ambiguous(v, _) => v,
            OffsetResult::None => panic!("Attempt to unwrap an invalid offset"),
        }
    }

    /// Unwraps this OffsetResult resolving ambiguity by taking the second result.
    pub fn unwrap_second(self) -> T {
        match self {
            OffsetResult::Some(v) => v,
            OffsetResult::Ambiguous(_, v) => v,
            OffsetResult::None => panic!("Attempt to unwrap an invalid offset"),
        }
    }

    /// Turns this OffsetResult into an Option assuming ambiguity is an error.
    pub fn take(self) -> Option<T> {
        match self {
            OffsetResult::Some(v) => Some(v),
            OffsetResult::Ambiguous(_, _) => None,
            OffsetResult::None => None,
        }
    }

    /// Turns this OffsetResult into an Option resolving ambiguity by taking the first result.
    pub fn take_first(self) -> Option<T> {
        match self {
            OffsetResult::Some(v) => Some(v),
            OffsetResult::Ambiguous(v, _) => Some(v),
            OffsetResult::None => None,
        }
    }

    /// Turns this OffsetResult into an Option resolving ambiguity by taking the second result.
    pub fn take_second(self) -> Option<T> {
        match self {
            OffsetResult::Some(v) => Some(v),
            OffsetResult::Ambiguous(_, v) => Some(v),
            OffsetResult::None => None,
        }
    }

    /// Returns true if this OffsetResult is None.
    pub fn is_none(&self) -> bool {
        match self {
            OffsetResult::Some(_) => false,
            OffsetResult::Ambiguous(_, _) => false,
            OffsetResult::None => true,
        }
    }

    /// Returns true if this OffsetResult is ambiguous.
    pub fn is_ambiguous(&self) -> bool {
        match self {
            OffsetResult::Some(_) => false,
            OffsetResult::Ambiguous(_, _) => true,
            OffsetResult::None => false,
        }
    }
}
