use std::ffi::{c_char, c_int, c_long, c_void, CString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::ptr;
use std::slice::from_raw_parts;
use std::str::FromStr;

use crate::{
    error::{check, check_eof},
    raw, MSControlFlags, MSDataEncoding, MSError, MSRecord, MSResult, MSTraceList,
};
use raw::{MS3FileParam, MS3Record};

/// Counterpart of [`MSWriter`].
///
/// Note that currently only reading miniSEED records from files is implemented.
pub type MSReader = MSFileParam;

/// Holds the connection information that [`MSFileParam`] should use for reading.
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
    /// Creates a new `MSFileParam` state container from `path_or_url`.
    pub fn new<T: IntoConnectionInfo>(path_or_url: T) -> MSResult<Self> {
        let connection_info = path_or_url.into_connection_info()?;

        Ok(Self {
            connection_info,
            flags: MSControlFlags::empty(),
            inner: ptr::null_mut(),
        })
    }

    /// Creates a new `MSFileParam` state container from `path_or_url` and control flags `flags`.
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

/// Converts an object into a [`ConnectionInfo`] struct. This allows the constructor of the client to
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
                // prefix relative paths with "./" to prevent `URL::parse()` from adding a trailing
                // slash
                let prefix = if Path::new(input).is_relative() {
                    "./"
                } else {
                    ""
                };
                let input = format!("file://{}{}", prefix, input);
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

/// Generic miniSEED record writer.
#[derive(Debug)]
pub struct MSWriter<W> {
    writer: W,
}

impl<W: Write> MSWriter<W> {
    /// Creates a new `MSWriter`.
    pub fn new(inner: W) -> MSWriter<W> {
        Self { writer: inner }
    }

    /// Consumes this `MSWriter`, returning the underlying writer.
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Returns a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Returns a mutable reference to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Writes the miniSEED record `msr` to the underlying writer.
    ///
    ///  If `flags` has [`MSControlFlags::MSF_FLUSHDATA`] set, all of the data will be packed into
    ///  data records even though the last one will probably be smaller than requested or, in the
    ///  case of miniSEED 2, unfilled.
    ///  If `flags` has [`MSControlFlags::MSF_PACKVER2`] set `msr` is packed as miniSEED v2
    ///  regardless of msr's [`MSRecord::format_version`].
    pub fn write_record(&mut self, msr: &MSRecord, flags: MSControlFlags) -> MSResult<c_long> {
        // XXX(damb): reimplementation of [`raw::msr3_writemseed`]
        unsafe {
            check(raw::msr3_pack(
                msr.get_raw(),
                Some(record_handler::<W>),
                (&mut self.writer) as *mut _ as *mut c_void,
                ptr::null_mut(),
                flags.bits(),
                0,
            ) as c_long)
        }
    }

    /// Writes `mstl` to the underlying writer.
    pub fn write_trace_list(
        &mut self,
        mstl: &mut MSTraceList,
        flags: MSControlFlags,
        encoding: MSDataEncoding,
        max_rec_len: c_int,
    ) -> MSResult<c_long> {
        // XXX(damb): reimplementation of [`raw::mstl3_writemseed`]
        let mut flags = flags;
        flags |= MSControlFlags::MSF_MAINTAINMSTL;
        flags |= MSControlFlags::MSF_FLUSHDATA;

        unsafe {
            check(raw::mstl3_pack(
                mstl.get_raw_mut(),
                Some(record_handler::<W>),
                (&mut self.writer) as *mut _ as *mut c_void,
                max_rec_len,
                encoding as _,
                ptr::null_mut(),
                flags.bits(),
                0,
                ptr::null_mut(),
            ) as c_long)
        }
    }
}

/// Generic record handler callback function used for writing miniSEED records.
extern "C" fn record_handler<W: Write>(rec: *mut c_char, rec_len: c_int, out: *mut c_void) {
    let writer: &mut W = unsafe { &mut *(out as *mut W) };
    let buf = unsafe { from_raw_parts(rec as *mut u8, rec_len.try_into().unwrap()) };

    writer.write_all(buf).unwrap();
}

#[cfg(test)]
mod tests {

    use super::*;

    use std::fs::File;

    use std::io::{BufReader, Read};

    use pretty_assertions::assert_eq;

    use crate::{test, MSControlFlags};

    #[test]
    fn test_read_write_msr() {
        let test_data = vec![
            "reference-testdata-text.mseed2",
            "reference-testdata-text.mseed3",
            "reference-testdata-steim2.mseed2",
            "reference-testdata-steim2.mseed3",
        ];

        for f in &test_data {
            let mut p = test::test_data_base_dir();
            assert!(p.is_dir());
            p.push(f);

            let writer = Vec::new();
            let mut writer = MSWriter::new(writer);
            let mut reader =
                MSReader::new_with_flags(p.clone(), MSControlFlags::MSF_UNPACKDATA).unwrap();
            while let Some(msr) = reader.next() {
                let mut msr = msr.unwrap();
                writer
                    .write_record(&mut msr, MSControlFlags::MSF_FLUSHDATA)
                    .unwrap();
            }

            let written = writer.into_inner();

            let reference_file = File::open(p).unwrap();
            let mut reference_file_reader = BufReader::new(reference_file);

            let mut expected = Vec::new();
            let expected_bytes_read = reference_file_reader.read_to_end(&mut expected).unwrap();

            assert_eq!(written.len(), expected_bytes_read);
            assert_eq!(written, expected);
        }
    }
}
