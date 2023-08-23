use std::ffi::{c_char, c_double, c_long, c_uchar, c_uint, c_ulong, c_ushort, CStr};
use std::fmt;
use std::ptr;
use std::slice::from_raw_parts;
use std::str;

use serde_json::Value;

use raw::MS3Record;

use crate::error::{check, check_nst};
use crate::{raw, util, MSControlFlags, MSError, MSResult, MSSubSeconds, MSTimeFormat};

/// Structure returned by [`detect()`].
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RecordDetection {
    /// Major version of the format detected.
    pub format_version: c_uchar,
    /// Size of the record in bytes. `None` if the record length is unknown.
    pub rec_len: Option<usize>,
}

/// Detect miniSEED record in buffer.
///
/// Determine if the buffer contains a miniSEED data record by verifying known signatures (fields
/// with known limited values).
pub fn detect<T: AsRef<[u8]>>(buf: T) -> MSResult<RecordDetection> {
    let mut format_version: c_uchar = 0;
    let format_version_ptr = (&mut format_version) as *mut _;

    let buf = buf.as_ref();
    let rec_len = unsafe {
        check(raw::ms3_detect(
            buf.as_ptr() as *const _,
            (buf.len() as c_ulong).into(),
            format_version_ptr,
        ))
    }?;

    let rec_len = if rec_len == 0 {
        None
    } else {
        Some(rec_len as usize)
    };

    Ok(RecordDetection {
        format_version,
        rec_len,
    })
}

/// An enumeration of possible sample types.
#[repr(u8)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum MSSampleType {
    /// Unknown data sample type.
    Unknown = 0, // \0
    /// Text data samples (UTF-8).
    Text = 116, // t
    /// 32-bit integer data samples.
    Integer32 = 105, // i
    /// 32-bit float (IEEE) data samples.
    Float32 = 102, // f
    /// 64-bit float (IEEE) data samples.
    Float64 = 100, // d
}

impl MSSampleType {
    /// Creates a `MSSampleType` from the given `ch`.
    pub fn from_char(ch: u8) -> Self {
        match ch {
            116 => Self::Text,      // t
            105 => Self::Integer32, // i
            102 => Self::Float32,   // f
            100 => Self::Float64,   // d
            _ => Self::Unknown,
        }
    }
}

/// An enumeration of possible data encodings.
#[repr(u8)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum MSDataEncoding {
    /// Text encoding (UTF-8)
    Text = raw::DE_TEXT as u8,
    /// 16-bit integer encoding
    Integer16 = raw::DE_INT16 as u8,
    /// 32-bit integer encoding
    Integer32 = raw::DE_INT32 as u8,
    /// 32-bit floating point encoding (IEEE)
    Float32 = raw::DE_FLOAT32 as u8,
    /// 64-bit floating point encoding (IEEE)
    Float64 = raw::DE_FLOAT64 as u8,
    /// Steim-1 compressed integer encoding
    Steim1 = raw::DE_STEIM1 as u8,
    /// Steim-2 compressed integer encoding
    Steim2 = raw::DE_STEIM2 as u8,
    /// **Legacy**: GEOSCOPE 24-bit integer encoding
    GeoScope24 = raw::DE_GEOSCOPE24 as u8,
    /// **Legacy**: GEOSCOPE 16-bit gain ranged, 3-bit exponent
    GeoScope163 = raw::DE_GEOSCOPE163 as u8,
    /// **Legacy**: GEOSCOPE 16-bit gain ranged, 4-bit exponent
    GeoScope164 = raw::DE_GEOSCOPE164 as u8,
    /// **Legacy**: CDSN 16-bit gain ranged
    CDSN = raw::DE_CDSN as u8,
    /// **Legacy**: SRO 16-bit gain ranged
    SRO = raw::DE_SRO as u8,
    /// **Legacy**: DWWSSN 16-bit gain ranged
    DWWSSN = raw::DE_DWWSSN as u8,
}

impl MSDataEncoding {
    /// Create a `MSDataEncoding` from the given `ch`.
    pub fn from_char(ch: u8) -> MSResult<Self> {
        match ch as u32 {
            raw::DE_TEXT => Ok(Self::Text),
            raw::DE_INT16 => Ok(Self::Integer16),
            raw::DE_INT32 => Ok(Self::Integer32),
            raw::DE_FLOAT32 => Ok(Self::Float32),
            raw::DE_FLOAT64 => Ok(Self::Float64),
            raw::DE_STEIM1 => Ok(Self::Steim1),
            raw::DE_STEIM2 => Ok(Self::Steim2),
            raw::DE_GEOSCOPE24 => Ok(Self::GeoScope24),
            raw::DE_GEOSCOPE163 => Ok(Self::GeoScope163),
            raw::DE_GEOSCOPE164 => Ok(Self::GeoScope164),
            raw::DE_CDSN => Ok(Self::CDSN),
            raw::DE_SRO => Ok(Self::SRO),
            raw::DE_DWWSSN => Ok(Self::DWWSSN),
            other => Err(MSError::from_str(&format!(
                "invalid data encoding type: {}",
                other
            ))),
        }
    }
}

impl fmt::Display for MSDataEncoding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            let encoding = CStr::from_ptr(raw::ms_encodingstr(
                (*self as c_char)
                    .try_into()
                    .map_err(|_| fmt::Error)
                    .unwrap(),
            ))
            .to_string_lossy();
            write!(f, "{}", encoding)
        }
    }
}

/// miniSEED record structure.
#[derive(Debug)]
pub struct MSRecord(*mut MS3Record);

impl MSRecord {
    fn ptr(&self) -> MS3Record {
        unsafe { *self.0 }
    }

    pub(crate) fn get_raw(&self) -> *const MS3Record {
        self.0
    }

    #[allow(dead_code)]
    pub(crate) unsafe fn get_raw_mut(&mut self) -> *mut MS3Record {
        self.0
    }

    /// Parses a `MSRecord` from a slice of bytes.
    pub fn parse(buf: &[u8], flags: MSControlFlags) -> MSResult<Self> {
        let msr: *mut MS3Record = ptr::null_mut();
        let mut msr = unsafe { raw::msr3_init(msr) };
        if msr.is_null() {
            return Err(MSError::from_str("failed to initialize record"));
        }

        unsafe {
            let buf = &*(buf as *const [_] as *const [c_char]);
            check(raw::msr3_parse(
                buf.as_ptr(),
                (buf.len() as c_ulong).into(),
                (&mut msr) as *mut *mut MS3Record,
                flags.bits(),
                0,
            ))?
        };

        Ok(Self(msr))
    }

    /// Creates a `MSRecord` from a raw pointer. Takes ownership.
    ///
    /// # Safety
    ///
    /// Takes ownership of a raw `MS3Record` pointer that was allocated by foreign code.
    pub unsafe fn from_raw(ptr: *mut MS3Record) -> Self {
        Self(ptr)
    }

    /// Consumes the MSRecord and transfers ownership of the record to a C caller.
    pub fn into_raw(mut self) -> *mut MS3Record {
        let rv = self.0;
        self.0 = ptr::null_mut();
        rv
    }

    /// Unpacks the packed data of the record and returns the number of unpacked samples.
    ///
    /// If the data is already unpacked, the number of previously unpacked samples is returned.
    pub fn unpack_data(&mut self) -> MSResult<c_long> {
        if !self.ptr().datasamples.is_null() {
            return Ok(self.num_samples());
        }
        unsafe { check(raw::msr3_unpack_data(self.0, 0).try_into().unwrap()) }
    }

    /// Returns the [FDSN source identifier](https://docs.fdsn.org/projects/source-identifiers/).
    pub fn sid(&self) -> MSResult<String> {
        let nslc = util::NetStaLocCha::from_sid(&self.ptr().sid)?;
        Ok(nslc.to_string())
    }

    /// Returns a lossy version of the [FDSN source identifier](https://docs.fdsn.org/projects/source-identifiers/).
    pub fn sid_lossy(&self) -> String {
        util::to_string(&(self.ptr().sid))
    }

    /// Returns the network code identifier of the record.
    pub fn network(&self) -> MSResult<String> {
        let nslc = util::NetStaLocCha::from_sid(&self.ptr().sid)?;
        Ok(nslc.net)
    }

    /// Returns the station code identifier of the record.
    pub fn station(&self) -> MSResult<String> {
        let nslc = util::NetStaLocCha::from_sid(&self.ptr().sid)?;
        Ok(nslc.sta)
    }

    /// Returns the location code identifier of the record.
    pub fn location(&self) -> MSResult<String> {
        let nslc = util::NetStaLocCha::from_sid(&self.ptr().sid)?;
        Ok(nslc.loc)
    }

    /// Returns the channel code identifier of the record.
    pub fn channel(&self) -> MSResult<String> {
        let nslc = util::NetStaLocCha::from_sid(&self.ptr().sid)?;
        Ok(nslc.cha)
    }

    /// Returns the raw miniSEED record, if available.
    pub fn raw(&self) -> Option<&[c_uchar]> {
        if self.ptr().record.is_null() || self.ptr().reclen == 0 {
            return None;
        }

        let ret = unsafe {
            from_raw_parts(
                self.ptr().record as *mut c_uchar,
                self.ptr().reclen as usize,
            )
        };
        Some(ret)
    }

    /// Returns the major format version of the underlying record.
    pub fn format_version(&self) -> c_uchar {
        self.ptr().formatversion
    }

    /// Returns the record level bit flags.
    pub fn flags(&self) -> c_uchar {
        self.ptr().flags
    }

    /// Returns the start time of the record (i.e. the time of the first sample).
    pub fn start_time(&self) -> MSResult<time::OffsetDateTime> {
        util::nstime_to_time(self.ptr().starttime.try_into().unwrap())
    }

    /// Calculates the end time of the last sample in the record.
    pub fn end_time(&self) -> MSResult<time::OffsetDateTime> {
        unsafe { util::nstime_to_time(check_nst(raw::msr3_endtime(self.0).try_into().unwrap())?) }
    }

    /// Returns the nominal sample rate as samples per second (`Hz`)
    pub fn sample_rate_hz(&self) -> c_double {
        unsafe { raw::msr3_sampratehz(&mut self.ptr() as *mut MS3Record) }
    }

    /// Returns the data encoding format of the record.
    pub fn encoding(&self) -> MSResult<MSDataEncoding> {
        MSDataEncoding::from_char(self.ptr().encoding as _)
    }

    /// Returns the record publication version.
    pub fn pub_version(&self) -> c_uchar {
        self.ptr().pubversion
    }

    /// Returns the number of data samples as indicated by the raw record.
    pub fn sample_cnt(&self) -> c_long {
        self.ptr().samplecnt.try_into().unwrap()
    }

    /// Returns the CRC of the record.
    pub fn crc(&self) -> c_uint {
        self.ptr().crc
    }

    /// Returns the length of the data payload in bytes.
    pub fn data_length(&self) -> c_ushort {
        self.ptr().datalength
    }

    /// Returns the records' extra headers, if available.
    pub fn extra_headers(&self) -> Option<&[c_uchar]> {
        if self.ptr().extra.is_null() || self.ptr().extralength == 0 {
            return None;
        }

        let ret = unsafe {
            from_raw_parts(
                self.ptr().extra as *const c_uchar,
                self.ptr().extralength as usize,
            )
        };
        Some(ret)
    }

    /// Returns the (unpacked) data samples of the record if available.
    pub fn data_samples<T>(&self) -> Option<&[T]> {
        if self.ptr().datasamples.is_null() {
            return None;
        }

        Some(unsafe {
            from_raw_parts(
                self.ptr().datasamples as *mut T,
                self.ptr().samplecnt as usize,
            )
        })
    }

    /// Returns the size of the (unpacked) data samples in bytes.
    pub fn data_size(&self) -> usize {
        self.ptr().datasize
    }

    /// Returns the number of (unpacked) data samples.
    pub fn num_samples(&self) -> c_long {
        self.ptr().numsamples.try_into().unwrap()
    }

    /// Returns the record sample type.
    pub fn sample_type(&self) -> MSSampleType {
        MSSampleType::from_char(self.ptr().sampletype as _)
    }

    /// Creates a new independently owned [`MSRecord`] from the underlying record.
    pub fn try_clone(&self) -> MSResult<Self> {
        let rv = unsafe { raw::msr3_duplicate(self.0, true as _) };

        if rv.is_null() {
            return Err(MSError::from_str("failed to duplicate"));
        }

        Ok(Self(rv))
    }

    /// Returns an object that implements [`Display`] for printing a record with level `detail`.
    ///
    /// The `detail` flag controls the level of detail displayed:
    /// - `0`: print a single summary line
    /// - `1`: print most header details
    /// - `>1`: print all header details
    ///
    ///  [`Display`]: fmt::Display
    ///
    ///  # Examples
    ///
    ///  Print most header details (i.e. detail `1`) of miniSEED records from a file:
    ///
    ///  ```no_run
    ///  use mseed::MSReader;
    ///
    ///  let mut reader = MSReader::new("path/to/data.mseed").unwrap();
    ///
    ///  while let Some(msr) = reader.next() {
    ///     let msr = msr.unwrap();
    ///     print!("{}", msr.display(1));
    ///  }
    ///  ```
    pub fn display(&self, detail: i8) -> RecordDisplay<'_> {
        RecordDisplay { rec: self, detail }
    }
}

impl fmt::Display for MSRecord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let v = self.ptr();
        write!(
            f,
            "{}, {}, {}, {} samples, {} Hz, {}",
            self.sid_lossy(),
            v.pubversion,
            v.reclen,
            v.samplecnt,
            self.sample_rate_hz(),
            util::nstime_to_string(
                v.starttime.try_into().unwrap(),
                MSTimeFormat::IsoMonthDayDoyZ,
                MSSubSeconds::NanoMicro
            )
            .unwrap_or("invalid".to_string())
        )
    }
}

impl AsRef<[u8]> for MSRecord {
    fn as_ref(&self) -> &[u8] {
        unsafe {
            from_raw_parts(
                self.ptr().record as *mut c_uchar,
                self.ptr().reclen as usize,
            )
        }
    }
}

impl Drop for MSRecord {
    fn drop(&mut self) {
        unsafe {
            raw::ms3_readmsr(
                (&mut self.0) as *mut *mut MS3Record,
                ptr::null(),
                MSControlFlags::empty().bits(),
                0,
            );
        }
    }
}

/// Helper struct for printing `MSRecord` with [`format!`] and `{}`.
pub struct RecordDisplay<'a> {
    rec: &'a MSRecord,
    detail: i8,
}

impl fmt::Debug for RecordDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.rec, f)
    }
}

impl fmt::Display for RecordDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // XXX(damb): reimplements `msr3_print()`
        if self.detail > 0 {
            writeln!(
                f,
                "{}, version {}, {} bytes (format: {})",
                self.rec.sid_lossy(),
                self.rec.pub_version(),
                unsafe { (*self.rec.get_raw()).reclen },
                self.rec.format_version()
            )?;
            let start_time = unsafe { (*self.rec.get_raw()).starttime };
            let start_time = util::nstime_to_string(
                start_time.try_into().unwrap(),
                MSTimeFormat::IsoMonthDayDoyZ,
                MSSubSeconds::NanoMicro,
            )
            .unwrap_or("invalid".to_string());
            writeln!(f, "             start time: {}", start_time)?;
            writeln!(f, "      number of samples: {}", self.rec.sample_cnt())?;
            writeln!(f, "       sample rate (Hz): {}", self.rec.sample_rate_hz())?;
            let flags = self.rec.flags();
            if self.detail > 1 {
                writeln!(f, "                  flags: [{:08b}] 8 bits", flags)?;

                if flags & (1 << 0) != 0 {
                    writeln!(
                        f,
                        "                         [Bit 0] Calibration signals present"
                    )?;
                }
                if flags & (1 << 1) != 0 {
                    writeln!(
                        f,
                        "                         [Bit 1] Time tag is questionable"
                    )?;
                }
                if flags & (1 << 2) != 0 {
                    writeln!(f, "                         [Bit 2] Clock locked")?;
                }
                if flags & (1 << 3) != 0 {
                    writeln!(f, "                         [Bit 3] Undefined bit set")?;
                }
                if flags & (1 << 4) != 0 {
                    writeln!(f, "                         [Bit 4] Undefined bit set")?;
                }
                if flags & (1 << 5) != 0 {
                    writeln!(f, "                         [Bit 5] Undefined bit set")?;
                }
                if flags & (1 << 6) != 0 {
                    writeln!(f, "                         [Bit 6] Undefined bit set")?;
                }
                if flags & (1 << 7) != 0 {
                    writeln!(f, "                         [Bit 7] Undefined bit set")?;
                }
            }

            writeln!(f, "                    CRC: {:X}", self.rec.crc())?;
            let extra_headers = self.rec.extra_headers();
            let extra_headers_len = if let Some(extra_headers) = extra_headers {
                extra_headers.len()
            } else {
                0
            };
            writeln!(f, "    extra header length: {} bytes", extra_headers_len)?;
            writeln!(
                f,
                "    data payload length: {} bytes",
                self.rec.data_length()
            )?;
            let encoding = self.rec.encoding().map_err(|_| fmt::Error)?;
            writeln!(
                f,
                "       payload encoding: {} (val: {})",
                encoding, encoding as c_uchar
            )?;

            if self.detail > 1 {
                if let Some(extra_headers) = extra_headers {
                    writeln!(f, "       extra headers:")?;
                    let json_str = as_json_pretty(extra_headers).map_err(|_| fmt::Error)?;
                    for line in json_str.lines() {
                        writeln!(f, "                {}", line)?;
                    }
                }
            }

            Ok(())
        } else {
            writeln!(f, "{}", self.rec)
        }
    }
}

fn as_json_pretty(slice: &[u8]) -> MSResult<String> {
    str::from_utf8(slice)
        .ok()
        .and_then(|json_str| serde_json::from_str(json_str).ok())
        .and_then(|v: Value| serde_json::to_string_pretty(&v).ok())
        .ok_or_else(|| MSError::from_str("failed to pretty format JSON"))
}

#[cfg(test)]
mod tests {

    use super::*;

    use crate::test;

    use std::fs::File;
    use std::io::{BufReader, Read};

    use pretty_assertions::assert_eq;
    use time::format_description::well_known::Iso8601;

    #[test]
    fn test_detect() {
        let test_data = vec![
            (
                "reference-testdata-text.mseed2",
                RecordDetection {
                    format_version: 2,
                    rec_len: Some(512),
                },
            ),
            (
                "reference-testdata-text.mseed3",
                RecordDetection {
                    format_version: 3,
                    rec_len: Some(294),
                },
            ),
            (
                "reference-testdata-steim2.mseed2",
                RecordDetection {
                    format_version: 2,
                    rec_len: Some(512),
                },
            ),
            (
                "reference-testdata-steim2.mseed3",
                RecordDetection {
                    format_version: 3,
                    rec_len: Some(507),
                },
            ),
            (
                "testdata-detection.record.mseed2",
                RecordDetection {
                    format_version: 2,
                    rec_len: Some(512),
                },
            ),
            (
                "testdata-no-blockette1000-steim1.mseed2",
                RecordDetection {
                    format_version: 2,
                    rec_len: Some(4096),
                },
            ),
        ];

        for (f, expected) in &test_data {
            let mut p = test::test_data_base_dir();
            assert!(p.is_dir());
            p.push(f);

            let file = File::open(p).unwrap();
            let mut reader = BufReader::new(file);

            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();

            assert_eq!(&detect(buf).unwrap(), expected);
        }
    }

    #[test]
    fn test_parse_signal_mseed3() {
        let mut p = test::test_data_base_dir();
        assert!(p.is_dir());

        p.push("testdata-3channel-signal.mseed3");

        let ifs = File::open(p).unwrap();
        let mut reader = BufReader::new(ifs);
        let mut buf: Vec<u8> = vec![];
        reader.read_to_end(&mut buf).unwrap();

        let msr = MSRecord::parse(&buf, MSControlFlags::MSF_UNPACKDATA).unwrap();
        assert_eq!(msr.network().unwrap(), "IU");
        assert_eq!(msr.station().unwrap(), "COLA");
        assert_eq!(msr.location().unwrap(), "00");
        assert_eq!(msr.channel().unwrap(), "LH1");

        assert_eq!(&msr.sid().unwrap(), "FDSN:IU_COLA_00_L_H_1");

        assert_eq!(msr.format_version(), 3);
        assert_eq!(
            msr.start_time().unwrap().format(&Iso8601::DEFAULT).unwrap(),
            "2010-02-27T06:50:00.069539000Z"
        );
        assert_eq!(
            msr.end_time().unwrap().format(&Iso8601::DEFAULT).unwrap(),
            "2010-02-27T06:52:14.069539000Z"
        );
        assert_eq!(msr.sample_rate_hz(), 1.0);
        assert_eq!(msr.encoding().unwrap(), MSDataEncoding::Steim2);
        assert_eq!(msr.pub_version(), 4);
        assert_eq!(msr.sample_cnt(), 135);
        assert_eq!(msr.crc(), 0x4F3EAB65);
        {
            let mut buf: Vec<u8> = vec![];
            buf.extend_from_slice(msr.extra_headers().unwrap());
            assert_eq!(
                String::from_utf8(buf).unwrap(),
                "{\"FDSN\":{\"Time\":{\"Quality\":100}}}"
            );
        }

        assert_eq!(msr.data_length(), 384);
        assert_eq!(msr.data_size(), 540);
        assert_eq!(msr.num_samples(), 135);

        assert_eq!(msr.sample_type(), MSSampleType::Integer32);
        {
            let mut buf: Vec<i32> = vec![];
            buf.extend_from_slice(msr.data_samples().unwrap());
            assert_eq!(buf.len(), 135);
            // Test first and last 4 decoded sample values
            assert_eq!(buf[0], -502676);
            assert_eq!(buf[1], -504105);
            assert_eq!(buf[2], -507491);
            assert_eq!(buf[3], -506991);

            assert_eq!(buf[131], -505212);
            assert_eq!(buf[132], -499533);
            assert_eq!(buf[133], -495590);
            assert_eq!(buf[134], -496168);
        }
    }

    #[test]
    fn test_parse_signal_mseed2() {
        let mut p = test::test_data_base_dir();
        assert!(p.is_dir());

        p.push("testdata-3channel-signal.mseed2");

        let ifs = File::open(p).unwrap();
        let mut reader = BufReader::new(ifs);
        let mut buf: Vec<u8> = vec![];
        reader.read_to_end(&mut buf).unwrap();

        let msr = MSRecord::parse(&buf, MSControlFlags::MSF_UNPACKDATA).unwrap();
        assert_eq!(msr.network().unwrap(), "IU");
        assert_eq!(msr.station().unwrap(), "COLA");
        assert_eq!(msr.location().unwrap(), "00");
        assert_eq!(msr.channel().unwrap(), "LH1");

        assert_eq!(&msr.sid().unwrap(), "FDSN:IU_COLA_00_L_H_1");

        assert_eq!(msr.format_version(), 2);
        assert_eq!(
            msr.start_time().unwrap().format(&Iso8601::DEFAULT).unwrap(),
            "2010-02-27T06:50:00.069539000Z"
        );
        assert_eq!(
            msr.end_time().unwrap().format(&Iso8601::DEFAULT).unwrap(),
            "2010-02-27T06:52:14.069539000Z"
        );
        assert_eq!(msr.sample_rate_hz(), 1.0);
        assert_eq!(msr.encoding().unwrap(), MSDataEncoding::Steim2);
        assert_eq!(msr.pub_version(), 4);
        assert_eq!(msr.sample_cnt(), 135);
        assert_eq!(msr.crc(), 0);
        {
            let mut buf: Vec<u8> = vec![];
            buf.extend_from_slice(msr.extra_headers().unwrap());
            assert_eq!(
                String::from_utf8(buf).unwrap(),
                "{\"FDSN\":{\"Time\":{\"Quality\":100}}}"
            );
        }

        assert_eq!(msr.data_length(), 448);
        assert_eq!(msr.data_size(), 540);
        assert_eq!(msr.num_samples(), 135);

        assert_eq!(msr.sample_type(), MSSampleType::Integer32);
        {
            let mut buf: Vec<i32> = vec![];
            buf.extend_from_slice(msr.data_samples().unwrap());
            assert_eq!(buf.len(), 135);
            // Test first and last 4 decoded sample values
            assert_eq!(buf[0], -502676);
            assert_eq!(buf[1], -504105);
            assert_eq!(buf[2], -507491);
            assert_eq!(buf[3], -506991);

            assert_eq!(buf[131], -505212);
            assert_eq!(buf[132], -499533);
            assert_eq!(buf[133], -495590);
            assert_eq!(buf[134], -496168);
        }
    }
}
