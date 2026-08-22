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

//! Support for getting time zone information from the target system.
//! 
//! Currently only supported for Windows, Unix, and WASM targets.

use crate::timezones::get_by_name;
use crate::Tz;
use thiserror::Error;

#[cfg(target_family = "wasm")]
use js_sys::{ Intl, Reflect, Array, Object };
#[cfg(target_family = "wasm")]
use wasm_bindgen::JsValue;

#[derive(Debug, Error)]
pub enum Error {
    /// An IO error has occurred.
    #[error("io error: {0}")]
    Io(std::io::Error),

    /// An OS level error has occurred (can only happen on Windows).
    #[error("low-level os error")]
    Os,

    /// The timezone is undetermined (means the timezone is not defined or that the system
    /// itself doesn't know the its timezone).
    #[error("undefined timezone")]
    Undetermined,

    /// Somehow the read timezone name contains non unicode...
    #[error("timezone name is not unicode")]
    Unicode,

    /// The timezone doesn't exist in the crate's database.
    #[error("unknown timezone name")]
    Unknown,
    
    /// The target platform is not supported. Windows, Unix, and WASM targets are the only supported for the system feature at this moment.
    #[error("unsupported platform")]
    Unsupported,
}

/// Gets the current timezone from the system.
/// 
/// Currently only supported for Windows, Unix, and WASM targets.
/// 
/// # Errors
/// Returns an [Error](enum@Error) if the timezone cannot be determined.
pub fn get_timezone() -> Result<&'static Tz, Error> {
    cfg_if::cfg_if! {
        if #[cfg(unix)] {
            use std::path::Path;
            let path = Path::new("/etc/localtime");
            let realpath = std::fs::read_link(path).map_err(Error::Io)?;
            // The part of the path we're interested in cannot contain non unicode characters.
            if let Some(iana) = realpath.to_str().ok_or(Error::Unicode)?.split("/zoneinfo/").last() {
                let tz = get_by_name(iana).ok_or(Error::Unknown)?;
                Ok(tz)
            } else {
                Err(Error::Undetermined)
            }
        } else if #[cfg(windows)] {
            unsafe {
                use windows_sys::Win32::System::Time::GetDynamicTimeZoneInformation;
                use windows_sys::Win32::System::Time::DYNAMIC_TIME_ZONE_INFORMATION;
                let mut data: DYNAMIC_TIME_ZONE_INFORMATION = std::mem::zeroed();
                let res = GetDynamicTimeZoneInformation(&mut data as _);
                if res > 2 {
                    return Err(Error::Os);
                } else {
                    let win_name_utf16 = &data.TimeZoneKeyName;
                    let mut len: usize = 0;
                    while win_name_utf16[len] != 0x0 {
                        len += 1;
                    }
                    if len == 0 {
                        return Err(Error::Undetermined);
                    }
                    let win_tz = String::from_utf16(&win_name_utf16[..len]).map_err(|_| Error::Unicode)?;
                    let tz = get_by_name(&win_tz).ok_or(Error::Unknown)?;
                    Ok(tz)
                }
            }
        } else if #[cfg(target_family = "wasm")] {
            let options = Intl::DateTimeFormat::new(&Array::new(), &Object::new())
                .resolved_options();

            let tz = Reflect::get(&options, &JsValue::from("timeZone"))
                .map_err(|_| Error::Undetermined)?
                .as_string()
                .ok_or(Error::Unicode)?;

            let tz = get_by_name(&tz).ok_or(Error::Unknown)?;
            Ok(tz)
        } else {
            Err(Error::Unsupported)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn get_timezone() {
        let tz = super::get_timezone();
        assert!(tz.is_ok());
    }
}
