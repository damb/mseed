//! # libmseed bindings for Rust.
//!
//! This library contains bindings to the [libmseed][1] C library which is used
//! to manage miniSEED data. The library itself is a work in progress and is
//! likely lacking some bindings here and there, so be warned.
//!
//! [1]: https://github.com/EarthScope/libmseed
//!
//! The mseed library strives to be as close to libmseed as possible, but also
//! strives to make using libmseed as safe as possible. All resource management
//! is automatic as well as adding strong types to all interfaces (including
//! `MSResult`)
//!
//! ## High-level miniSEED record I/O
//!
//! Reading and writing miniSEED records is implemented by means of [`MSReader`] and [`MSWriter`],
//! respectively.
//!
//! ```no_run
//! use std::fs::OpenOptions;
//!
//! use mseed::{MSControlFlags, MSReader, MSWriter};
//!
//! let mut reader =
//!     MSReader::new_with_flags("path/to/in.mseed", MSControlFlags::MSF_UNPACKDATA).unwrap();
//!
//! let out_file = OpenOptions::new().write(true).open("out.mseed").unwrap();
//! let mut writer = MSWriter::new(out_file);
//!
//! while let Some(msr) = reader.next() {
//!     let mut msr = msr.unwrap();
//!
//!     if msr.network().unwrap() == "NET" && msr.station().unwrap() == "STA" {
//!         // do something with msr
//!
//!         writer
//!             .write_record(&mut msr, MSControlFlags::MSF_FLUSHDATA)
//!             .unwrap();
//!     }
//! }
//! ```
//!
//!
//! ## Low-level miniSEED record I/O
//!
//! Creating miniSEED records from raw data samples is possible using the low-level [`pack()`]
//! function:
//!
//! ```no_run
//! use std::fs::OpenOptions;
//! use std::io::{BufWriter, Write};
//!
//! use time::format_description::well_known::Iso8601;
//! use time::OffsetDateTime;
//!
//! use mseed::{self, MSControlFlags, PackInfo};
//!
//! let mut pack_info = PackInfo::new("FDSN:XX_TEST__X_Y_Z").unwrap();
//!
//! let file = OpenOptions::new()
//!     .create(true)
//!     .write(true)
//!     .open("path/to/out.mseed")
//!     .unwrap();
//! let mut writer = BufWriter::new(file);
//!
//! let record_handler = move |rec: &[u8]| {
//!     let _ = writer.write(rec);
//! };
//!
//! let mut data_samples: Vec<i32> = (1..100).collect();
//! let start_time = OffsetDateTime::parse("2012-01-01T00:00:00Z", &Iso8601::DEFAULT).unwrap();
//! mseed::pack(
//!     &mut data_samples,
//!     &start_time,
//!     record_handler,
//!     &pack_info,
//!     MSControlFlags::MSF_FLUSHDATA,
//! )
//! .unwrap();
//! ```

use std::ffi::c_uint;

use bitflags::bitflags;

use libmseed_sys as raw;

pub use crate::error::MSError;
pub use crate::io::{ConnectionInfo, IntoConnectionInfo, MSFileParam, MSReader, MSWriter};
pub use crate::pack::{pack, PackInfo};
pub use crate::record::{MSDataEncoding, MSRecord, MSSampleType, RecordDisplay};
pub use crate::trace::{
    DataSampleType, MSTraceId, MSTraceIdIter, MSTraceList, MSTraceSegment, MSTraceSegmentIter,
    TlPackInfo,
};

mod error;
mod io;
mod pack;
mod record;
mod test;
mod trace;
mod util;

/// A specialized library `Result` type.
pub type MSResult<T> = std::result::Result<T, MSError>;

/// An enumeration of possible errors that can happen when working with miniSEED records.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum MSErrorCode {
    /// Generic unspecified error
    GenericError,
    /// Data not SEED
    NotSEED,
    /// Length of data read was incorrect
    WrongLength,
    /// SEED record length out of range
    OutOfRange,
    /// Unknown data encoding format
    UnknownFormat,
    /// Steim, invalid compression
    SteimBadCompressionFlag,
    /// Invalid CRC
    InvalidCRC,
}

bitflags! {
    /// Parsing, packing and trace construction control flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct MSControlFlags: c_uint {
        /// **Parsing**: Unpack data samples.
        const MSF_UNPACKDATA = raw::MSF_UNPACKDATA;
        /// **Parsing**: Skip input that cannot be identified as miniSEED.
        const MSF_SKIPNOTDATA = raw::MSF_SKIPNOTDATA;
        /// **Parsing**: Validate CRC (if version 3).
        const MSF_VALIDATECRC = raw::MSF_VALIDATECRC;
        /// **Parsing**: Parse and utilize byte range from path name suffix.
        const MSF_PNAMERANGE = raw::MSF_PNAMERANGE;
        /// **Parsing**: Reading routine is at the end of the file.
        const MSF_ATENDOFFILE = raw::MSF_ATENDOFFILE;
        /// **Packing**: UNSUPPORTED: Maintain a record-level sequence number.
        const MSF_SEQUENCE = raw::MSF_SEQUENCE;
        /// **Packing**: Pack all available data even if final record would not be filled.
        const MSF_FLUSHDATA = raw::MSF_FLUSHDATA;
        /// **Packing**: Pack as miniSEED version 2 instead of version 3.
        const MSF_PACKVER2 = raw::MSF_PACKVER2;
        /// **TraceList**: Build a [`raw::MS3RecordList`] for each [`raw::MS3TraceSeg`].
        const MSF_RECORDLIST = raw::MSF_RECORDLIST;
        /// **TraceList**: Do not modify a trace list when packing.
        const MSF_MAINTAINMSTL = raw::MSF_MAINTAINMSTL;
    }
}
