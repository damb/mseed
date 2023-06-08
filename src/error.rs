use std::error;
use std::ffi::CStr;
use std::fmt;

use num_traits::cast::AsPrimitive;

use crate::{raw, MSErrorCode, MSResult};

const MS_NOERROR: i64 = raw::MS_NOERROR as i64;
const MS_ENDOFFILE: i64 = raw::MS_ENDOFFILE as i64;
const MS_NSTERROR: i64 = raw::NSTERROR;

/// Utility function which turns a libmseed error into a result.
pub(crate) fn check<T: PartialOrd + AsPrimitive<i64>>(code: T) -> MSResult<T> {
    let c = code.as_();
    if c >= MS_NOERROR {
        return Ok(code);
    }

    Err(MSError::from_raw(c as i32))
}

/// Checks if the function's return code is `MS_ENDOFFILE`.
pub(crate) fn check_eof<T: PartialEq + AsPrimitive<i64>>(code: T) -> bool {
    code.as_() == MS_ENDOFFILE
}

/// Turns the return code of libmseed functions which normally return a high precision time
/// into a result.
///
/// On error a ``MSError` with code `MSErrorCode::GenericError` is returned.
pub(crate) fn check_nst(code: i64) -> MSResult<i64> {
    if code > MS_NSTERROR {
        return Ok(code);
    }

    Err(MSError::from_str("time processing error"))
}

/// A structure representing libmseed errors.
#[derive(Debug, PartialEq)]
pub struct MSError {
    code: i32,
    message: String,
}

pub(crate) const MS_GENERROR: i32 = raw::MS_GENERROR as i32;
pub(crate) const MS_NOTSEED: i32 = raw::MS_NOTSEED as i32;
pub(crate) const MS_WRONGLENGTH: i32 = raw::MS_WRONGLENGTH as i32;
pub(crate) const MS_OUTOFRANGE: i32 = raw::MS_OUTOFRANGE as i32;
pub(crate) const MS_UNKNOWNFORMAT: i32 = raw::MS_UNKNOWNFORMAT as i32;
pub(crate) const MS_STBADCOMPFLAG: i32 = raw::MS_STBADCOMPFLAG as i32;
pub(crate) const MS_INVALIDCRC: i32 = raw::MS_INVALIDCRC as i32;

impl MSError {
    /// Creates a new error from a given raw error code.
    pub(crate) fn from_raw(code: i32) -> Self {
        unsafe {
            let message = CStr::from_ptr(raw::ms_errorstr(code)).to_bytes();
            let message = String::from_utf8_lossy(message).into_owned();

            Self { code, message }
        }
    }

    /// Creates a new error from the given string as the error.
    ///
    /// The error returned will have the code `MS_GENERROR`.
    pub fn from_str(s: &str) -> Self {
        Self {
            code: MS_GENERROR,
            message: s.to_string(),
        }
    }

    /// Returns the error code associated with this error.
    pub fn code(&self) -> MSErrorCode {
        match self.raw_code() {
            MS_NOTSEED => MSErrorCode::NotSEED,
            MS_WRONGLENGTH => MSErrorCode::WrongLength,
            MS_OUTOFRANGE => MSErrorCode::OutOfRange,
            MS_UNKNOWNFORMAT => MSErrorCode::UnknownFormat,
            MS_STBADCOMPFLAG => MSErrorCode::SteimBadCompressionFlag,
            MS_INVALIDCRC => MSErrorCode::InvalidCRC,
            _ => MSErrorCode::GenericError,
        }
    }

    /// Returns the raw error code associated with this error.
    pub fn raw_code(&self) -> i32 {
        match self.code {
            MS_NOTSEED => MS_NOTSEED,
            MS_WRONGLENGTH => MS_WRONGLENGTH,
            MS_OUTOFRANGE => MS_OUTOFRANGE,
            MS_UNKNOWNFORMAT => MS_UNKNOWNFORMAT,
            MS_STBADCOMPFLAG => MS_STBADCOMPFLAG,
            MS_INVALIDCRC => MS_INVALIDCRC,
            _ => MS_GENERROR,
        }
    }

    /// Returns the message associated with this error
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl error::Error for MSError {}

impl fmt::Display for MSError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}; code={:?} ({})",
            self.message,
            self.code(),
            self.code
        )?;

        Ok(())
    }
}
