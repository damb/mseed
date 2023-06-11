use std::fmt;
use std::ptr;
use std::slice::from_raw_parts;

use raw::MS3Record;

use crate::error::{check, check_nst};
use crate::{raw, util, MSControlFlags, MSError, MSResult};

/// An enumeration of possible sample types.
#[repr(i8)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum MSSampleType {
    Text = 116,      // t
    Integer32 = 105, // i
    Float32 = 102,   // f
    Float64 = 100,   // d
}

impl MSSampleType {
    /// Create a `MSSampleType` from the given `ch`.
    pub fn from_char(ch: i8) -> MSResult<Self> {
        match ch {
            116 => Ok(Self::Text),      // t
            105 => Ok(Self::Integer32), // i
            102 => Ok(Self::Float32),   // f
            100 => Ok(Self::Float64),   // d
            other => Err(MSError::from_str(&format!(
                "invalid sample type: {}",
                other
            ))),
        }
    }
}

/// An enumeration of possible data encodings.
#[repr(i8)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum MSDataEncoding {
    Text = raw::DE_TEXT as i8,
    Integer16 = raw::DE_INT16 as i8,
    Integer32 = raw::DE_INT32 as i8,
    Float32 = raw::DE_FLOAT32 as i8,
    Float64 = raw::DE_FLOAT64 as i8,
    Steim1 = raw::DE_STEIM1 as i8,
    Steim2 = raw::DE_STEIM2 as i8,
    GeoScope24 = raw::DE_GEOSCOPE24 as i8,
    GeoScope163 = raw::DE_GEOSCOPE163 as i8,
    GeoScope164 = raw::DE_GEOSCOPE164 as i8,
    CDSN = raw::DE_CDSN as i8,
    SRO = raw::DE_SRO as i8,
    DWWSSN = raw::DE_DWWSSN as i8,
}

impl MSDataEncoding {
    /// Create a `MSDataEncoding` from the given `ch`.
    pub fn from_char(ch: i8) -> MSResult<Self> {
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

// TODO(damb): implement `Clone` trait
/// miniSEED record structure.
#[derive(Debug)]
pub struct MSRecord(*mut MS3Record);

impl MSRecord {
    fn ptr(&self) -> MS3Record {
        unsafe { *self.0 }
    }

    /// Parse a `MSRecord` from a slice of bytes.
    pub fn parse(buf: &mut [u8], flags: MSControlFlags) -> MSResult<Self> {
        let msr: *mut MS3Record = ptr::null_mut();
        let mut msr = unsafe { raw::msr3_init(msr) };
        if msr.is_null() {
            return Err(MSError::from_str("failed to initialize record"));
        }

        unsafe {
            let buf = &mut *(buf as *mut [u8] as *mut [i8]);
            check(raw::msr3_parse(
                buf.as_mut().as_mut_ptr(),
                buf.as_mut().len() as u64,
                (&mut msr) as *mut *mut MS3Record,
                flags.bits(),
                0,
            ))?
        };

        Ok(Self(msr))
    }

    /// Create a `MSRecord` from a raw pointer. Takes ownership.
    pub unsafe fn from_raw(ptr: *mut MS3Record) -> Self {
        Self(ptr)
    }

    /// Consumes the MSRecord and transfers ownership of the record to a C caller.
    pub fn into_raw(mut self) -> *mut MS3Record {
        let rv = self.0;
        self.0 = ptr::null_mut();
        rv
    }

    /// Unpacks data samples of the record and return the number of unpacked samples.
    ///
    /// If the data is already unpacked, the number of previously unpacked samples is returned.
    pub fn unpack_data(&mut self) -> MSResult<i64> {
        if !self.ptr().datasamples.is_null() {
            return Ok(self.num_samples());
        }
        unsafe {
            check(raw::msr3_unpack_data(
                (&mut self.ptr()) as *mut MS3Record,
                0,
            ))
        }
    }

    /// Returns the FDSN source identifier.
    ///
    /// FDSN Source Identifiers are defined at:
    /// https://docs.fdsn.org/projects/source-identifiers/
    pub fn sid(&self) -> MSResult<String> {
        let nslc = util::NSLC::from_sid(&self.ptr().sid)?;
        Ok(nslc.to_string())
    }

    /// Returns a lossy version of the FDSN source indentifier.
    pub fn sid_lossy(&self) -> String {
        util::i8_to_string(&(self.ptr().sid))
    }

    /// Returns the network code identifier of the record.
    pub fn network(&self) -> MSResult<String> {
        let nslc = util::NSLC::from_sid(&self.ptr().sid)?;
        Ok(nslc.net)
    }

    /// Returns the station code identifier of the record.
    pub fn station(&self) -> MSResult<String> {
        let nslc = util::NSLC::from_sid(&self.ptr().sid)?;
        Ok(nslc.sta)
    }

    /// Returns the location code identifier of the record.
    pub fn location(&self) -> MSResult<String> {
        let nslc = util::NSLC::from_sid(&self.ptr().sid)?;
        Ok(nslc.loc)
    }

    /// Returns the channel code identifier of the record.
    pub fn channel(&self) -> MSResult<String> {
        let nslc = util::NSLC::from_sid(&self.ptr().sid)?;
        Ok(nslc.cha)
    }

    /// Returns the raw miniSEED record, if available.
    pub fn raw(&self) -> Option<&[u8]> {
        if self.ptr().record.is_null() || self.ptr().reclen == 0 {
            return None;
        }

        let ret =
            unsafe { from_raw_parts(self.ptr().record as *mut u8, self.ptr().reclen as usize) };
        Some(ret)
    }

    /// Returns the major format version of the underlying record.
    pub fn format_version(&self) -> u8 {
        self.ptr().formatversion
    }

    /// Returns the start time of the record (i.e. the time of the first sample).
    pub fn start_time(&self) -> MSResult<time::OffsetDateTime> {
        util::nstime_to_time(self.ptr().starttime)
    }

    /// Calculates the end time of the last sample in the record.
    pub fn end_time(&self) -> MSResult<time::OffsetDateTime> {
        unsafe {
            util::nstime_to_time(check_nst(raw::msr3_endtime(
                &mut self.ptr() as *mut MS3Record
            ))?)
        }
    }

    /// Returns the nominal sample rate as samples per second (`Hz`)
    pub fn sample_rate_hz(&self) -> f64 {
        unsafe { raw::msr3_sampratehz(&mut self.ptr() as *mut MS3Record) }
    }

    /// Returns the data encoding format of the record.
    pub fn encoding(&self) -> MSResult<MSDataEncoding> {
        MSDataEncoding::from_char(self.ptr().encoding)
    }

    /// Returns the record publication version.
    pub fn pub_version(&self) -> u8 {
        self.ptr().pubversion
    }

    /// Returns the number of data samples as indicated by the raw record.
    pub fn sample_cnt(&self) -> i64 {
        self.ptr().samplecnt
    }

    /// Returns the CRC of the record.
    pub fn crc(&self) -> u32 {
        self.ptr().crc
    }

    /// Returns the length of the data payload in bytes.
    pub fn data_length(&self) -> u16 {
        self.ptr().datalength
    }

    /// Returns the record's extra headers, if available.
    pub fn extra_headers(&mut self) -> Option<&[u8]> {
        if self.ptr().extra.is_null() || self.ptr().extralength == 0 {
            return None;
        }

        let ret =
            unsafe { from_raw_parts(self.ptr().extra as *mut u8, self.ptr().extralength as usize) };
        Some(ret)
    }

    /// Returns the data samples of the record.
    ///
    /// Note that the data samples are unpacked, if required. An empty slice is returned if unpacking
    /// the data samples failed.
    pub fn data_samples<T>(&mut self) -> &[T] {
        if self.ptr().datasamples.is_null() && self.unpack_data().is_err() {
            return &[];
        }

        unsafe {
            from_raw_parts(
                self.ptr().datasamples as *mut T,
                self.ptr().samplecnt as usize,
            )
        }
    }

    /// Returns the size of the (unpacked) data samples in bytes.
    pub fn data_size(&self) -> usize {
        self.ptr().datasize
    }

    /// Returns the number of (unpacked) data samples.
    pub fn num_samples(&self) -> i64 {
        self.ptr().numsamples
    }

    /// Returns the record sample type.
    pub fn sample_type(&self) -> MSResult<MSSampleType> {
        MSSampleType::from_char(self.ptr().sampletype)
    }
}

impl fmt::Display for MSRecord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let v = self.ptr();
        write!(
            f,
            "{}, {}, {}, {} samples, {} Hz, {:?}",
            self.sid_lossy(),
            v.pubversion,
            v.reclen,
            v.samplecnt,
            v.samprate,
            util::nstime_to_string(v.starttime).unwrap_or("invalid".to_string())
        )
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

#[cfg(test)]
mod tests {

    use super::*;

    use std::fs::File;
    use std::io::{BufReader, Read};
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;
    use time::format_description::well_known::Iso8601;

    fn test_data_base_dir() -> PathBuf {
        let mut base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        base_dir.push("tests/data");

        base_dir
    }

    #[test]
    fn test_parse_signal_mseed3() {
        let mut p = test_data_base_dir();
        assert!(p.is_dir());

        p.push("testdata-3channel-signal.mseed3");

        let ifs = File::open(p).unwrap();
        let mut reader = BufReader::new(ifs);
        let mut buf: Vec<u8> = vec![];
        reader.read_to_end(&mut buf).unwrap();

        let mut msr = MSRecord::parse(&mut buf, MSControlFlags::MSF_UNPACKDATA).unwrap();
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

        assert_eq!(msr.sample_type().unwrap(), MSSampleType::Integer32);
        {
            let mut buf: Vec<i32> = vec![];
            buf.extend_from_slice(msr.data_samples());
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
        let mut p = test_data_base_dir();
        assert!(p.is_dir());

        p.push("testdata-3channel-signal.mseed2");

        let ifs = File::open(p).unwrap();
        let mut reader = BufReader::new(ifs);
        let mut buf: Vec<u8> = vec![];
        reader.read_to_end(&mut buf).unwrap();

        let mut msr = MSRecord::parse(&mut buf, MSControlFlags::MSF_UNPACKDATA).unwrap();
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

        assert_eq!(msr.sample_type().unwrap(), MSSampleType::Integer32);
        {
            let mut buf: Vec<i32> = vec![];
            buf.extend_from_slice(msr.data_samples());
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

