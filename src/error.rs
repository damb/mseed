use std::error;
use std::ffi::{c_int, c_long, CStr};
use std::fmt;

use num_traits::cast::AsPrimitive;

use crate::{raw, MSErrorCode, MSResult};

const MS_NOERROR: c_long = raw::MS_NOERROR as c_long;
const MS_ENDOFFILE: c_long = raw::MS_ENDOFFILE as c_long;
const MS_NSTERROR: c_long = raw::NSTERROR as c_long;

/// Utility function which turns a libmseed error into a result.
pub(crate) fn check<T: PartialOrd + AsPrimitive<c_long>>(code: T) -> MSResult<T> {
    let c = code.as_();
    if c >= MS_NOERROR {
        return Ok(code);
    }

    Err(MSError::from_raw(c as c_int))
}

/// Checks if the function's return code is `MS_ENDOFFILE`.
pub(crate) fn check_eof<T: PartialEq + AsPrimitive<c_long>>(code: T) -> bool {
    code.as_() == MS_ENDOFFILE
}

/// Turns the return code of libmseed functions which normally return a high precision time
/// into a result.
///
/// On error a ``MSError` with code `MSErrorCode::GenericError` is returned.
pub(crate) fn check_nst(code: c_long) -> MSResult<c_long> {
    if code > MS_NSTERROR {
        return Ok(code);
    }

    Err(MSError::from_str("time processing error"))
}

/// A structure representing libmseed errors.
#[derive(Debug, PartialEq)]
pub struct MSError {
    code: c_int,
    message: String,
}

pub(crate) const MS_GENERROR: c_int = raw::MS_GENERROR as c_int;
pub(crate) const MS_NOTSEED: c_int = raw::MS_NOTSEED as c_int;
pub(crate) const MS_WRONGLENGTH: c_int = raw::MS_WRONGLENGTH as c_int;
pub(crate) const MS_OUTOFRANGE: c_int = raw::MS_OUTOFRANGE as c_int;
pub(crate) const MS_UNKNOWNFORMAT: c_int = raw::MS_UNKNOWNFORMAT as c_int;
pub(crate) const MS_STBADCOMPFLAG: c_int = raw::MS_STBADCOMPFLAG as c_int;
pub(crate) const MS_INVALIDCRC: c_int = raw::MS_INVALIDCRC as c_int;

impl MSError {
    /// Creates a new error from a given raw error code.
    pub(crate) fn from_raw(code: c_int) -> Self {
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
    pub fn raw_code(&self) -> c_int {
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

    /// Returns the message associated with this error.
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
