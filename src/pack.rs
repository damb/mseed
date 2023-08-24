use std::ffi::{c_char, c_double, c_int, c_long, c_uchar, c_uint, c_ushort, c_void, CString};
use std::mem;
use std::ptr;
use std::slice;

use crate::{
    error::check, raw, util, MSControlFlags, MSDataEncoding, MSError, MSRecord, MSResult,
    MSSampleType, MSTraceList,
};
use raw::MS3Record;

/// Struct aggregating [`MSTraceList`] packing information.
///
/// See also [`PackInfo`].
#[derive(Debug, Clone)]
pub struct TlPackInfo {
    // /// The miniSEED format version.
    // pub format_version: c_uchar,
    /// Data encoding.
    pub encoding: MSDataEncoding,
    /// Record length used for encoding.
    pub rec_len: c_int,
    /// Extra headers.
    ///
    /// If not `None` it is expected to contain extra headers, i.e. a string containing (compact)
    /// JSON, that will be added to each output record.
    pub extra_headers: Option<CString>,
}

impl Default for TlPackInfo {
    fn default() -> Self {
        Self {
            encoding: MSDataEncoding::Steim2,
            rec_len: 4096,
            extra_headers: None,
        }
    }
}

/// Packs the trace lists' data into miniSEED records.
///
/// Buffers containing the packed miniSEED records are passed to the `record_handler` closure.
/// Returns on success a tuple where the first value is the number of totally packed records
/// and the second value is the number of totally packed samples.
///
/// Packing is controlled by the following `flags`:
/// - If `flags` has [`MSControlFlags::MSF_FLUSHDATA`] set, all of the trace lists' data will be
/// packed into miniSEED records even though the last one will probably be smaller than
/// requested or, in the case of miniSEED v2, unfilled.
/// - If `flags` has [`MSControlFlags::MSF_PACKVER2`] set records are packed as miniSEED v2.
/// - If `flags` has [`MSControlFlags::MSF_MAINTAINMSTL`] packed data is not removed from the
/// trace lists' internal buffers.
///
/// See also [`pack_record()`] for packing record data and [`pack_raw()`] for packing raw data
/// samples.
pub fn pack_trace_list<F>(
    mstl: &mut MSTraceList,
    mut record_handler: F,
    info: &TlPackInfo,
    flags: MSControlFlags,
) -> MSResult<(usize, usize)>
where
    F: FnMut(&[u8]),
{
    let mut extra_ptr = ptr::null_mut();
    if let Some(extra_headers) = &info.extra_headers {
        let cloned = extra_headers.clone();
        extra_ptr = cloned.into_raw();
    }

    let mut cnt_samples: i64 = 0;
    let cnt_samples_ptr: *mut i64 = &mut cnt_samples;
    let cnt_records = unsafe {
        check(raw::mstl3_pack(
            mstl.get_raw_mut(),
            Some(rh_wrapper::<F>),
            (&mut record_handler) as *mut _ as *mut c_void,
            info.rec_len,
            info.encoding as _,
            cnt_samples_ptr,
            flags.bits(),
            0,
            extra_ptr,
        ))?
    };

    if !extra_ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(extra_ptr);
        }
    }

    Ok((cnt_records as usize, cnt_samples as usize))
}

/// Struct providing miniSEED record packing information.
#[derive(Debug, Clone)]
pub struct PackInfo {
    /// [FDSN source identifier](https://docs.fdsn.org/projects/source-identifiers/).
    sid: CString,
    /// Record sample rate.
    pub sample_rate: c_double,
    /// The miniSEED format version.
    pub format_version: c_uchar,
    /// Publication version.
    pub pub_version: c_uchar,
    /// Data encoding.
    pub encoding: MSDataEncoding,
    /// Record length used for encoding.
    pub rec_len: c_int,
    /// Extra headers.
    ///
    /// If not `None` it is expected to contain extra headers, i.e. a string containing (compact)
    /// JSON, that will be added to each output record.
    pub extra_headers: Option<CString>,
}

impl PackInfo {
    /// Creates a new `PackInfo` from a [FDSN source identifier](https://docs.fdsn.org/projects/source-identifiers/).
    pub fn new<T>(sid: T) -> MSResult<Self>
    where
        T: Into<Vec<u8>>,
    {
        Ok(Self {
            sid: sid_as_cstring(sid)?,
            sample_rate: 1.0,
            format_version: 3,
            pub_version: 1,
            encoding: MSDataEncoding::Steim2,
            rec_len: 4096,
            extra_headers: None,
        })
    }

    /// Creates a new `PackInfo` from a [FDSN source
    /// identifier](https://docs.fdsn.org/projects/source-identifiers/) with configured sample
    /// rate.
    pub fn with_sample_rate<T>(sid: T, sample_rate: c_double) -> MSResult<Self>
    where
        T: Into<Vec<u8>>,
    {
        let mut rv = Self::new(sid)?;
        rv.sample_rate = sample_rate;

        Ok(rv)
    }

    /// Returns a reference to the [FDSN source
    /// identifier](https://docs.fdsn.org/projects/source-identifiers/).
    pub fn sid(&self) -> &CString {
        &self.sid
    }

    /// Sets the [FDSN source identifier](https://docs.fdsn.org/projects/source-identifiers/).
    pub fn set_sid<T>(&mut self, sid: T) -> MSResult<()>
    where
        T: Into<Vec<u8>>,
    {
        self.sid = sid_as_cstring(sid)?;

        Ok(())
    }
}

fn sid_as_cstring<T>(sid: T) -> MSResult<CString>
where
    T: Into<Vec<u8>>,
{
    let sid = CString::new(sid).map_err(|e| MSError::from_str(&e.to_string()))?;
    if sid.as_bytes_with_nul().len() > raw::LM_SIDLEN as usize {
        return Err(MSError::from_str("sid too large"));
    }

    Ok(sid)
}

/// Low level function that packs raw data samples into miniSEED records.
///
/// `start_time` is the time of the first data sample. Buffers containing the packed miniSEED
/// records are passed to the `record_handler` closure. Returns on success a tuple where the first
/// value is the number of totally packed records and the second value is the number of totally
/// packed samples.
///
/// If `flags` has [`MSControlFlags::MSF_FLUSHDATA`] set, all of the `data_samples `will be packed
/// into miniSEED records even though the last one will probably be smaller than requested or, in
/// the case of miniSEED v2, unfilled.
/// If `flags` has [`MSControlFlags::MSF_PACKVER2`] set records are packed as miniSEED v2
/// regardless of [`PackInfo::format_version`].
///
/// See also [`raw::msr3_pack`].
///
/// # Examples
///
/// Basic usage
///
/// ```rust
/// # use pretty_assertions::assert_eq;
/// # use mseed::MSResult;
/// # fn main() -> MSResult<()> {
/// use time::format_description::well_known::Iso8601;
/// use time::OffsetDateTime;
///
/// use mseed::{MSControlFlags, MSDataEncoding, MSRecord, MSSampleType, PackInfo};
///
/// let mut pack_info = PackInfo::new("FDSN:XX_TEST__X_Y_Z").unwrap();
/// pack_info.encoding = MSDataEncoding::Text;
///
/// let record_handler = |rec: &[u8]| {
///     let mut buf = rec.to_vec();
///     let msr = MSRecord::parse(&mut buf, MSControlFlags::MSF_UNPACKDATA).unwrap();
///
///     assert_eq!(msr.sid().unwrap(), "FDSN:XX_TEST__X_Y_Z");
///     assert_eq!(msr.encoding().unwrap(), MSDataEncoding::Text);
///     assert_eq!(msr.sample_type(), MSSampleType::Text);
/// };
///
/// let flags = MSControlFlags::MSF_FLUSHDATA;
/// let start_time = OffsetDateTime::parse("2012-01-01T00:00:00Z", &Iso8601::DEFAULT).unwrap();
///
/// let mut payload: Vec<u8> = "Hello, miniSEED!".bytes().collect();
/// let (cnt_records, cnt_samples) = mseed::pack_raw(
///     &mut payload,
///     &start_time,
///     record_handler,
///     &pack_info,
///     flags,
/// )
/// .unwrap();
///
/// assert_eq!(cnt_records, 1);
/// assert_eq!(cnt_samples, 16);
/// # Ok(())
/// # }
///
/// ```
///
/// The `record_handler` closure may be customized to process the injected packed miniSEED record
/// buffers. For instance, writing the records to a file may be implemented as follows:
///
/// ```no_run
/// use std::fs::OpenOptions;
/// use std::io::{BufWriter, Write};
///
/// use time::format_description::well_known::Iso8601;
/// use time::OffsetDateTime;
///
/// use mseed::{self, MSControlFlags, PackInfo};
///
/// let pack_info = PackInfo::new("FDSN:XX_TEST__X_Y_Z").unwrap();
///
/// let file = OpenOptions::new()
///     .create(true)
///     .write(true)
///     .open("path/to/out.mseed")
///     .unwrap();
/// let mut writer = BufWriter::new(file);
///
/// let record_handler = move |rec: &[u8]| {
///     let _ = writer.write(rec);
/// };
///
/// let mut data_samples: Vec<i32> = (1..100).collect();
/// let start_time = OffsetDateTime::parse("2012-01-01T00:00:00Z", &Iso8601::DEFAULT).unwrap();
/// mseed::pack_raw(
///     &mut data_samples,
///     &start_time,
///     record_handler,
///     &pack_info,
///     MSControlFlags::MSF_FLUSHDATA,
/// )
/// .unwrap();
/// ```
pub fn pack_raw<T, F>(
    data_samples: &mut [T],
    start_time: &time::OffsetDateTime,
    mut record_handler: F,
    info: &PackInfo,
    flags: MSControlFlags,
) -> MSResult<(usize, usize)>
where
    F: FnMut(&[u8]),
{
    let msr: *mut MS3Record = ptr::null_mut();
    let mut msr = unsafe { raw::msr3_init(msr) };
    if msr.is_null() {
        return Err(MSError::from_str("failed to initialize record"));
    }

    unsafe {
        let sid_len = info.sid().as_bytes_with_nul().len();
        ptr::copy_nonoverlapping(info.sid().as_ptr(), (*msr).sid.as_mut_ptr(), sid_len);
        (*msr).encoding = info.encoding as _;
        (*msr).sampletype = {
            use MSDataEncoding::*;
            match info.encoding {
                Text => MSSampleType::Text,
                Integer16 | Integer32 | Steim1 | Steim2 => MSSampleType::Integer32,
                Float32 => MSSampleType::Float32,
                Float64 => MSSampleType::Float64,
                _ => MSSampleType::Unknown,
            }
        } as c_char;
        (*msr).reclen = info.rec_len;
        (*msr).starttime = util::time_to_nstime(start_time);
        (*msr).pubversion = info.pub_version;
        (*msr).formatversion = info.format_version;
        (*msr).numsamples = c_long::try_from(data_samples.len())
            .map_err(|e| MSError::from_str(&format!("invalid data sample length ({})", e)))?
            as _;
        (*msr).datasamples = data_samples.as_mut_ptr() as *mut _ as *mut c_void;
        (*msr).datasize = mem::size_of_val(data_samples);
        (*msr).extralength = 0;
        (*msr).extra = ptr::null_mut();
    }

    if let Some(extra_headers) = &info.extra_headers {
        let cloned = extra_headers.clone();
        unsafe {
            (*msr).extralength = c_ushort::try_from(cloned.as_bytes_with_nul().len())
                .map_err(|e| MSError::from_str(&format!("invalid extra header length ({})", e)))?;
            (*msr).extra = cloned.into_raw();
        }
    }

    let mut cnt_samples: i64 = 0;
    let cnt_samples_ptr: *mut i64 = &mut cnt_samples;

    let cnt_records = unsafe {
        check(raw::msr3_pack(
            msr,
            Some(rh_wrapper::<F>),
            (&mut record_handler) as *mut _ as *mut c_void,
            cnt_samples_ptr,
            flags.bits(),
            0,
        ))?
    };

    unsafe {
        let extra_ptr = (*msr).extra;
        if !extra_ptr.is_null() {
            let _ = CString::from_raw(extra_ptr);
            (*msr).extra = ptr::null_mut();
        }
        (*msr).datasamples = ptr::null_mut();
        (*msr).numsamples = 0;
        (*msr).datasize = 0;

        raw::msr3_free((&mut msr) as *mut *mut _);
    }

    Ok((cnt_records as usize, cnt_samples as usize))
}

extern "C" fn rh_wrapper<F>(rec: *mut c_char, rec_len: c_int, out: *mut c_void)
where
    F: FnMut(&[u8]),
{
    let rec = unsafe { slice::from_raw_parts(rec as *mut u8, rec_len as usize) };
    let callback = unsafe { &mut *(out as *mut F) };

    callback(rec);
}

/// Pack record data into miniSEED records.
///
/// Buffers containing the packed miniSEED records are passed to the `record_handler` closure.
/// Returns on success a tuple where the first value is the number of totally packed records and
/// the second value is the number of totally packed samples.
///
/// If `flags` has [`MSControlFlags::MSF_FLUSHDATA`] set, all of the record data will be packed
/// into miniSEED records even though the last one will probably be smaller than requested or, in
/// the case of miniSEED v2, unfilled.
/// If `flags` has [`MSControlFlags::MSF_PACKVER2`] set records are packed as miniSEED v2.
#[allow(dead_code)]
pub fn pack_record<F>(
    msr: &MSRecord,
    mut record_handler: F,
    flags: MSControlFlags,
) -> MSResult<(usize, usize)>
where
    F: FnMut(&[u8]),
{
    let mut cnt_samples: i64 = 0;
    let cnt_samples_ptr: *mut i64 = &mut cnt_samples;

    let cnt_records = unsafe {
        check(raw::msr3_pack(
            msr.get_raw(),
            Some(rh_wrapper::<F>),
            (&mut record_handler) as *mut _ as *mut c_void,
            cnt_samples_ptr,
            flags.bits(),
            0,
        ))?
    };

    Ok((cnt_records as usize, cnt_samples as usize))
}

///  Repack a parsed miniSEED record into a version 3 record.
///
///  Pack the parsed header into a version 3 header and copy the raw encoded data from the original
///  record. Returns on success the record length in bytes.
///
///  Note that this can be used to efficiently convert format versions or modify header values
///  without unpacking the data samples.
///
///  # Examples
#[allow(dead_code)]
pub fn repack_mseed3(msr: &MSRecord, buf: &mut [u8]) -> MSResult<usize> {
    Ok(unsafe {
        check(raw::msr3_repack_mseed3(
            msr.get_raw(),
            buf.as_mut_ptr() as *mut _,
            buf.len() as c_uint,
            0,
        ))? as usize
    })
}

/// Pack a miniSEED version 3 header into the specified buffer.
///
/// Returns on success the size of the header (fixed and extra) in bytes.
#[allow(dead_code)]
pub fn pack_header3(msr: &MSRecord, buf: &mut [u8]) -> MSResult<usize> {
    Ok(unsafe {
        check(raw::msr3_pack_header3(
            msr.get_raw(),
            buf.as_mut_ptr() as *mut _,
            buf.len() as c_uint,
            0,
        ))? as usize
    })
}

/// Pack a miniSEED version 2 header into the specified buffer.
///
/// Returns on success the size of the header (fixed and blockettes) in bytes.
#[allow(dead_code)]
pub fn pack_header2(msr: &MSRecord, buf: &mut [u8]) -> MSResult<usize> {
    Ok(unsafe {
        check(raw::msr3_pack_header2(
            msr.get_raw(),
            buf.as_mut_ptr() as *mut _,
            buf.len() as c_uint,
            0,
        ))? as usize
    })
}
