use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::ptr;
use std::str::FromStr;

use crate::{
    error::{check, check_eof},
    raw, MSControlFlags, MSError, MSRecord, MSResult,
};
use raw::{MS3FileParam, MS3Record};

/// Holds the connection information that `MSFileParam` should use for reading.
#[derive(Debug)]
pub struct ConnectionInfo(CString);

/// A state container for reading miniSEED records.
#[derive(Debug)]
pub struct MSFileParam {
    connection_info: ConnectionInfo,
    flags: MSControlFlags,
    inner: *mut MS3FileParam,
}

impl MSFileParam {
    /// Creates a new `MSFileParam` state container from `path_or_url`
    pub fn new<T: IntoConnectionInfo>(path_or_url: T) -> MSResult<Self> {
        let connection_info = path_or_url.into_connection_info()?;

        Ok(Self {
            connection_info,
            flags: MSControlFlags::empty(),
            inner: ptr::null_mut(),
        })
    }

    /// Creates a new `MSFileParam` state container from `path_or_url` and control flags `flags`
    pub fn new_with_flags<T: IntoConnectionInfo>(
        path_or_url: T,
        flags: MSControlFlags,
    ) -> MSResult<Self> {
        let connection_info = path_or_url.into_connection_info()?;

        Ok(Self {
            connection_info,
            flags,
            inner: ptr::null_mut(),
        })
    }
}

impl Iterator for MSFileParam {
    type Item = MSResult<MSRecord>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut msr: *mut MS3Record = ptr::null_mut();
        let rv = unsafe {
            raw::ms3_readmsr_r(
                (&mut self.inner) as *mut *mut MS3FileParam,
                (&mut msr) as *mut *mut MS3Record,
                self.connection_info.0.as_ptr(),
                self.flags.bits(),
                0,
            )
        };

        if check_eof(rv) {
            return None;
        }

        match check(rv) {
            Ok(_) => {
                let msr = unsafe { MSRecord::from_raw(msr) };
                Some(Ok(msr))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

impl Drop for MSFileParam {
    fn drop(&mut self) {
        let mut msr: *mut MS3Record = ptr::null_mut();
        let _rv = unsafe {
            raw::ms3_readmsr_r(
                (&mut self.inner) as *mut *mut MS3FileParam,
                (&mut msr) as *mut *mut MS3Record,
                ptr::null_mut(),
                MSControlFlags::empty().bits(),
                0,
            )
        };
    }
}

impl FromStr for ConnectionInfo {
    type Err = MSError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        s.into_connection_info()
    }
}

/// Converts an object into a `ConnectionInfo` struct. This allows the constructor of the client to
/// accept a file path or an URL in a range of different formats.
pub trait IntoConnectionInfo {
    /// Converts the object into a connection info object.
    fn into_connection_info(self) -> MSResult<ConnectionInfo>;
}

impl IntoConnectionInfo for ConnectionInfo {
    fn into_connection_info(self) -> MSResult<ConnectionInfo> {
        Ok(self)
    }
}

impl<'a> IntoConnectionInfo for &'a str {
    fn into_connection_info(self) -> MSResult<ConnectionInfo> {
        match parse_url(self) {
            Some(url) => url.into_connection_info(),
            None => Err(MSError::from_str("URL did not parse")),
        }
    }
}

impl IntoConnectionInfo for String {
    fn into_connection_info(self) -> MSResult<ConnectionInfo> {
        match parse_url(&self) {
            Some(url) => url.into_connection_info(),
            None => Err(MSError::from_str("URL did not parse")),
        }
    }
}

impl<'a> IntoConnectionInfo for &'a Path {
    fn into_connection_info(self) -> MSResult<ConnectionInfo> {
        match parse_url(&self.to_string_lossy()) {
            Some(url) => url.into_connection_info(),
            None => Err(MSError::from_str("path did not parse")),
        }
    }
}

impl IntoConnectionInfo for PathBuf {
    fn into_connection_info(self) -> MSResult<ConnectionInfo> {
        match parse_url(&self.to_string_lossy()) {
            Some(url) => url.into_connection_info(),
            None => Err(MSError::from_str("path did not parse")),
        }
    }
}

impl IntoConnectionInfo for url::Url {
    fn into_connection_info(self) -> MSResult<ConnectionInfo> {
        match self.scheme() {
            "file" => url_to_connection_info(self),
            _ => Err(MSError::from_str("URL provided is not a valid URL")),
        }
    }
}

/// This function takes a URL string and parses it into a URL as used by rust-url.
pub fn parse_url(input: &str) -> Option<url::Url> {
    match url::Url::parse(input) {
        Ok(result) => match result.scheme() {
            "file" => Some(result),
            _ => None,
        },
        Err(e) => match e {
            url::ParseError::RelativeUrlWithoutBase => {
                let input = format!("file://{}", input);
                parse_url(&input)
            }
            _ => None,
        },
    }
}

// TODO(damb):
// - allow to configure a URL including username & password
fn url_to_connection_info(url: url::Url) -> MSResult<ConnectionInfo> {
    let url: String = url.into();
    let url =
        CString::new(url.as_bytes().to_vec()).map_err(|e| MSError::from_str(&e.to_string()))?;

    Ok(ConnectionInfo(url))
}
