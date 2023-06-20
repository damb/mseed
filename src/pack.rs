use std::ffi::{c_char, c_double, c_int, c_long, c_uchar, c_ushort, c_void, CString};
use std::mem;
use std::ptr;
use std::slice;

use crate::{
    error::check, raw, util, MSControlFlags, MSDataEncoding, MSError, MSResult, MSSampleType,
};
use raw::MS3Record;

/// Struct providing miniSEED record packing information.
pub struct PackInfo {
    /// FDSN source identifier.
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
    pub extra_headers: Option<CString>,
}

impl PackInfo {
    /// Creates a new `PackInfo`.
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

    /// Creates a new `PackInfo` with configured `sample_rate`.
    pub fn with_sample_rate<T>(sid: T, sample_rate: c_double) -> MSResult<Self>
    where
        T: Into<Vec<u8>>,
    {
        let mut rv = Self::new(sid)?;
        rv.sample_rate = sample_rate;

        Ok(rv)
    }

    /// Returns a reference to the FDSN source identifier.
    pub fn sid(&self) -> &CString {
        &self.sid
    }

    /// Sets the FDSN source identifier.
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

/// Low level function that packs `data_samples` into miniSEED records.
///
/// `start_time` is the time of the first data sample. Buffers containing the packed miniSEED
/// records are passed to the `record_handler` closure. Returns the number of totally packed
/// samples.
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
/// mseed::pack(&mut payload, &start_time, record_handler, &pack_info, flags).unwrap();
///
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
/// let mut pack_info = PackInfo::new("FDSN:XX_TEST__X_Y_Z").unwrap();
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
/// mseed::pack(
///     &mut data_samples,
///     &start_time,
///     record_handler,
///     &pack_info,
///     MSControlFlags::MSF_FLUSHDATA,
/// )
/// .unwrap();
/// ```
pub fn pack<T, F>(
    data_samples: &mut [T],
    start_time: &time::OffsetDateTime,
    mut record_handler: F,
    info: &PackInfo,
    flags: MSControlFlags,
) -> MSResult<c_long>
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
        (*msr).encoding = info.encoding as c_char;
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
        (*msr).numsamples = c_long::try_from(data_samples.len()).map_err(|e| {
            MSError::from_str(&format!("invalid data sample length ({})", e.to_string()))
        })?;
        (*msr).datasamples = data_samples.as_mut_ptr() as *mut _ as *mut c_void;
        (*msr).datasize = mem::size_of_val(data_samples);
    }

    let mut extra_headers_ptr = ptr::null_mut();
    if let Some(extra_headers) = &info.extra_headers {
        let cloned = extra_headers.clone();
        unsafe {
            (*msr).extralength =
                c_ushort::try_from(cloned.as_bytes_with_nul().len()).map_err(|e| {
                    MSError::from_str(&format!("invalid extra header length ({})", e.to_string()))
                })?;
        }
        extra_headers_ptr = cloned.into_raw();
    }

    let mut rv: c_long = 0;
    let rv_ptr = &mut rv as *mut _;

    unsafe {
        check(raw::msr3_pack(
            msr,
            Some(rh_wrapper::<F>),
            (&mut record_handler) as *mut _ as *mut c_void,
            rv_ptr,
            flags.bits(),
            0,
        ))?
    };

    if !extra_headers_ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(extra_headers_ptr);
        }
    }

    Ok(rv)
}

extern "C" fn rh_wrapper<F>(rec: *mut c_char, rec_len: c_int, out: *mut c_void)
where
    F: FnMut(&[u8]),
{
    let rec = unsafe { slice::from_raw_parts(rec as *mut u8, rec_len as usize) };
    let callback = unsafe { &mut *(out as *mut F) };

    callback(rec);
}
