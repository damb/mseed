use std::ffi::{c_double, c_float, c_int, c_long, c_uchar, c_uint};
use std::fmt;
use std::ptr;
use std::slice::from_raw_parts;

use crate::{
    error::check, raw, util, MSControlFlags, MSError, MSRecord, MSResult, MSSampleType,
    MSSubSeconds, MSTimeFormat,
};
use time::OffsetDateTime;

use raw::{MS3TraceID, MS3TraceList, MS3TraceSeg};

/// A container for a trace identifier composed by [`MSTraceSegment`]s.
#[derive(Debug)]
pub struct MSTraceId(*mut MS3TraceID);

impl MSTraceId {
    fn ptr(&self) -> MS3TraceID {
        unsafe { *self.0 }
    }

    #[allow(dead_code)]
    pub(crate) fn get_raw(&self) -> *const MS3TraceID {
        self.0
    }

    #[allow(dead_code)]
    pub(crate) unsafe fn get_raw_mut(&mut self) -> *mut MS3TraceID {
        self.0
    }

    /// Returns the [FDSN source identifier](https://docs.fdsn.org/projects/source-identifiers/).
    pub fn sid(&self) -> MSResult<String> {
        let nslc = util::NetStaLocCha::from_sid(&self.ptr().sid)?;
        Ok(nslc.to_string())
    }

    /// Returns the largest contributing publication version.
    pub fn pub_version(&self) -> c_uchar {
        self.ptr().pubversion
    }

    /// Returns the time of the the first sample.
    pub fn start_time(&self) -> MSResult<OffsetDateTime> {
        util::nstime_to_time(self.ptr().earliest as _)
    }

    /// Returns the time of the the last sample.
    pub fn end_time(&self) -> MSResult<OffsetDateTime> {
        util::nstime_to_time(self.ptr().latest as _)
    }

    /// Returns the number of [`MSTraceSegment`]s for this trace identifier.
    pub fn len(&self) -> c_uint {
        self.ptr().numsegments
    }

    /// Returns whether the trace identifier is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over the trace identifiers' trace segments.
    pub fn iter(&self) -> MSTraceSegmentIter {
        MSTraceSegmentIter {
            trace_id: self,

            next: self.ptr().first,
            prev: ptr::null_mut(),
        }
    }
}

/// An iterator for [`MSTraceId`].
#[derive(Debug)]
pub struct MSTraceIdIter {
    next: *mut MS3TraceID,
}

impl Iterator for MSTraceIdIter {
    type Item = MSTraceId;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next.is_null() {
            return None;
        }

        let rv = Some(MSTraceId(self.next));
        self.next = unsafe { (*self.next).next[0] };
        rv
    }
}

/// A container for a continuous trace segment.
#[derive(Debug)]
pub struct MSTraceSegment<'id> {
    _trace_id: &'id MSTraceId,

    inner: *mut MS3TraceSeg,
}

impl<'id> MSTraceSegment<'id> {
    fn ptr(&self) -> MS3TraceSeg {
        unsafe { *self.inner }
    }

    /// Returns the time of the the first sample.
    pub fn start_time(&self) -> MSResult<OffsetDateTime> {
        util::nstime_to_time(self.ptr().starttime as _)
    }

    /// Returns the time of the the last sample.
    pub fn end_time(&self) -> MSResult<OffsetDateTime> {
        util::nstime_to_time(self.ptr().endtime as _)
    }

    /// Returns the nominal sample rate as samples per second (`Hz`)
    pub fn sample_rate_hz(&self) -> c_double {
        self.ptr().samprate
    }

    /// Returns the number of samples in trace coverage.
    pub fn sample_cnt(&self) -> c_long {
        self.ptr().samplecnt as _
    }

    /// Returns the data samples of the trace segment.
    ///
    /// Note that the data samples must have been unpacked, previously. Deferred unpacking of data
    /// samples from the internal record list is currently not implemented.
    pub fn data_samples<T: DataSampleType>(&mut self) -> MSResult<&[T]> {
        if !self.is_data_unpacked() {
            return Err(MSError::from_str("data samples must be unpacked"));
        }

        let rv = unsafe {
            <T as DataSampleType>::convert_into(self.inner, false)?;
            from_raw_parts(
                self.ptr().datasamples as *mut T,
                self.ptr().samplecnt as usize,
            )
        };

        Ok(rv)
    }

    /// Returns the size of the (unpacked) data samples in bytes.
    pub fn data_size(&self) -> usize {
        self.ptr().datasize
    }

    /// Returns the number of (unpacked) data samples.
    pub fn num_samples(&self) -> c_long {
        self.ptr().numsamples as _
    }

    /// Returns the trace segment sample type.
    pub fn sample_type(&self) -> MSSampleType {
        MSSampleType::from_char(self.ptr().sampletype as _)
    }

    /// Returns whether the data samples are unpacked.
    pub fn is_data_unpacked(&self) -> bool {
        self.sample_cnt() == self.num_samples()
            && self.data_size() > 0
            && !self.ptr().datasamples.is_null()
    }

    ///// Unpacks data samples of the trace segment and returns the number of unpacked samples.
    /////
    ///// If the data is already unpacked, the number of previously unpacked samples is returned.
    //pub fn unpack_data(&mut self) -> MSResult<c_long> {
    //    todo!();

    //    if !self.ptr().datasamples.is_null() {
    //        return Ok(self.num_samples());
    //    }

    //    unsafe {
    //        check(raw::mstl3_unpack_recordlist(
    //            self.trace_id.get_raw_mut(),
    //            self.inner,
    //            ptr::null_mut(),
    //            0,
    //            0,
    //        ))
    //    }
    //}
}

pub trait DataSampleType {
    /// Converts the trace segments' samples
    ///
    /// # Safety
    ///
    /// `seg` must not be a null pointer.
    unsafe fn convert_into(seg: *mut MS3TraceSeg, truncate: bool) -> MSResult<()>;
}

impl DataSampleType for c_uchar {
    unsafe fn convert_into(_seg: *mut MS3TraceSeg, _truncate: bool) -> MSResult<()> {
        Ok(())
    }
}

impl DataSampleType for c_int {
    unsafe fn convert_into(seg: *mut MS3TraceSeg, truncate: bool) -> MSResult<()> {
        let rv = unsafe {
            check(raw::mstl3_convertsamples(
                seg,
                MSSampleType::Integer32 as _,
                truncate as _,
            ))
        };

        match rv {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

impl DataSampleType for c_float {
    unsafe fn convert_into(seg: *mut MS3TraceSeg, truncate: bool) -> MSResult<()> {
        let rv = unsafe {
            check(raw::mstl3_convertsamples(
                seg,
                MSSampleType::Float32 as _,
                truncate as _,
            ))
        };

        match rv {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

impl DataSampleType for c_double {
    unsafe fn convert_into(seg: *mut MS3TraceSeg, truncate: bool) -> MSResult<()> {
        let rv = unsafe {
            check(raw::mstl3_convertsamples(
                seg,
                MSSampleType::Float64 as _,
                truncate as _,
            ))
        };

        match rv {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// An iterator for [`MSTraceSegment`].
#[derive(Debug)]
pub struct MSTraceSegmentIter<'id> {
    trace_id: &'id MSTraceId,

    next: *mut MS3TraceSeg,
    prev: *mut MS3TraceSeg,
}

impl<'id> Iterator for MSTraceSegmentIter<'id> {
    type Item = MSTraceSegment<'id>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next.is_null() {
            return None;
        }

        let rv = Some(MSTraceSegment {
            _trace_id: self.trace_id,
            inner: self.next,
        });
        self.prev = self.next;
        self.next = unsafe { (*self.next).next };
        rv
    }
}

impl<'id> DoubleEndedIterator for MSTraceSegmentIter<'id> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.prev.is_null() {
            return None;
        }

        let rv = Some(MSTraceSegment {
            _trace_id: self.trace_id,
            inner: self.prev,
        });
        self.next = self.prev;
        self.prev = unsafe { (*self.prev).prev };
        rv
    }
}

/// A container for [`MSTraceId`]s.
///
/// # Examples
///
/// Creating a `MSTraceList` from a file may be implemented as follows:
///
/// ```no_run
/// use std::fs::File;
///
/// use std::io::{Read, BufReader};
///
/// use mseed::{MSControlFlags, MSTraceList};
///
/// let file = File::open("path/to/data.mseed").unwrap();
/// let mut reader = BufReader::new(file);
///
/// let mut buf = Vec::new();
/// // read content of `data.mseed` into `buf`
/// reader.read_to_end(&mut buf).unwrap();
///
/// let mstl = MSTraceList::from_buffer(&buf, MSControlFlags::MSF_UNPACKDATA).unwrap();
/// ```
///
/// If controlling the records to be inserted is desired, using [`MSReader`] is required:
///
/// ```no_run
/// use std::fs::File;
///
/// use mseed::{MSControlFlags, MSReader, MSTraceList};
///
/// let mut mstl = MSTraceList::new().unwrap();
///
/// let mut reader =
///     MSReader::new_with_flags("path/to/data.mseed", MSControlFlags::MSF_UNPACKDATA).unwrap();
///
/// while let Some(res) = reader.next() {
///     let msr = res.unwrap();
///
///     if msr.network().unwrap() == "NET" && msr.station().unwrap() == "STA" {
///         mstl.insert(msr, true).unwrap();
///     }
/// }
///
/// // do something with `mstl`
/// let mstl_iter = mstl.iter();
/// for tid in mstl_iter {
///     let tid_iter = tid.iter();
///     for tseg in tid_iter {
///         // do something with `tseg`
///     }
/// }
/// ```
/// [`MSReader`]: crate::MSReader
#[derive(Debug)]
pub struct MSTraceList {
    inner: *mut MS3TraceList,
}

impl MSTraceList {
    fn ptr(&self) -> MS3TraceList {
        unsafe { *self.inner }
    }

    #[allow(dead_code)]
    pub(crate) fn get_raw(&self) -> *const MS3TraceList {
        self.inner
    }

    #[allow(dead_code)]
    pub(crate) unsafe fn get_raw_mut(&mut self) -> *mut MS3TraceList {
        self.inner
    }

    /// Creates a new [`MSTraceList`] container.
    pub fn new() -> MSResult<Self> {
        let mstl: *mut MS3TraceList = ptr::null_mut();
        let mstl = unsafe { raw::mstl3_init(mstl) };
        if mstl.is_null() {
            return Err(MSError::from_str("failed to initialize trace list"));
        }

        Ok(Self { inner: mstl })
    }

    /// Creates a new [`MSTraceList`] from a buffer.
    pub fn from_buffer(buf: &[u8], flags: MSControlFlags) -> MSResult<Self> {
        let mut rv = Self::new()?;

        unsafe {
            let buf = &*(buf as *const [_] as *const [_]);
            check(raw::mstl3_readbuffer(
                (&mut rv.get_raw_mut()) as *mut *mut _,
                buf.as_ptr(),
                buf.len() as _,
                0,
                flags.bits(),
                ptr::null_mut(),
                0,
            ))
        }?;

        Ok(rv)
    }

    /// Returns the length of the trace list.
    pub fn len(&self) -> c_uint {
        self.ptr().numtraceids
    }

    /// Returns whether the trace list is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a forward iterator over the trace lists' trace identifiers.
    pub fn iter(&self) -> MSTraceIdIter {
        MSTraceIdIter {
            next: self.ptr().traces.next[0],
        }
    }

    /// Inserts `rec` into the trace list.
    ///
    /// Note that currently [`MSTraceList`] does not implement deferred unpacking of data samples.
    /// Therefore, clients need to make sure that the `rec` inserted is unpacked, beforehand. If
    /// not doing so, the trace list will merely be a list of channels.
    pub fn insert(&mut self, rec: MSRecord, autoheal: bool) -> MSResult<()> {
        let rv = unsafe {
            raw::mstl3_addmsr_recordptr(
                self.inner,
                rec.into_raw(),
                ptr::null_mut(),
                0,
                autoheal as _,
                MSControlFlags::empty().bits(),
                ptr::null_mut(),
            )
        };

        if rv.is_null() {
            return Err(MSError::from_str("failed to insert record"));
        }

        Ok(())
    }

    /// Returns an object that implements [`Display`] for printing a trace list summary.
    ///
    /// By default only prints the [FDSN source
    /// identifier](https://docs.fdsn.org/projects/source-identifiers/), starttime and endtime for
    /// each trace. If `detail` is greater than zero the sample rate, number of samples and a total
    /// trace count is included.
    /// If `gap` is greater than zero and the previous trace matches both the FDSN source identifier
    /// and  the sample rate the gap between the endtime of the last trace and the starttime of the
    /// current trace is included.
    /// If `version` is greater than zero, the publication version is included.
    ///
    ///  [`Display`]: fmt::Display
    pub fn display(
        &self,
        time_format: MSTimeFormat,
        detail: i8,
        gap: i8,
        version: i8,
    ) -> TraceListDisplay<'_> {
        TraceListDisplay {
            mstl: self,
            time_format,
            detail,
            gap,
            version,
        }
    }
}

impl Drop for MSTraceList {
    fn drop(&mut self) {
        unsafe { raw::mstl3_free((&mut self.inner) as *mut *mut MS3TraceList, 1) };
    }
}

/// Helper struct for printing `MSTraceList` with [`format!`] and `{}`.
pub struct TraceListDisplay<'a> {
    mstl: &'a MSTraceList,
    time_format: MSTimeFormat,
    detail: i8,
    gap: i8,
    version: i8,
}

impl fmt::Debug for TraceListDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.mstl, f)
    }
}

impl fmt::Display for TraceListDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // XXX(damb): reimplements `mstl3_printtracelist()`
        if self.detail > 0 && self.gap > 0 {
            writeln!(f, "       SourceID                      Start sample                End sample           Gap  Hz  Samples")?;
        } else if self.detail <= 0 && self.gap > 0 {
            writeln!(f, "       SourceID                      Start sample                End sample           Gap")?;
        } else if self.detail > 0 && self.gap <= 0 {
            writeln!(f, "       SourceID                      Start sample                End sample           Hz  Samples")?;
        } else {
            writeln!(
                f,
                "       SourceID                      Start sample                End sample"
            )?;
        }

        let mut tid_cnt = 0;
        let mut tseg_cnt = 0;

        for tid in self.mstl.iter() {
            let sid = tid.sid().map_err(|_| fmt::Error)?;
            let sid = if self.version > 0 {
                format!("{}#{}", sid, tid.pub_version())
            } else {
                sid
            };
            for tseg in tid.iter() {
                let start_time = unsafe { (*tseg.inner).starttime };
                let start_time_str = util::nstime_to_string(
                    start_time as _,
                    self.time_format,
                    MSSubSeconds::NanoMicro,
                )
                .map_err(|_| fmt::Error)?;

                let end_time = unsafe { (*tseg.inner).endtime };
                let end_time_str = util::nstime_to_string(
                    end_time as _,
                    self.time_format,
                    MSSubSeconds::NanoMicro,
                )
                .map_err(|_| fmt::Error)?;

                if self.gap > 0 {
                    let mut gap: f64 = 0.0;
                    let mut no_gap = false;

                    let prev_tseg_ptr = unsafe { (*tseg.inner).prev };
                    if !prev_tseg_ptr.is_null() {
                        gap = (start_time - unsafe { (*prev_tseg_ptr).endtime }) as f64
                            / raw::NSTMODULUS as f64;
                    } else {
                        no_gap = true;
                    }

                    // Check that any overlap is not larger than the trace coverage
                    if gap < 0.0 {
                        let sample_rate = unsafe { (*tseg.inner).samprate };
                        let delta = if sample_rate != 0.0 {
                            1.0 / sample_rate
                        } else {
                            0.0
                        };
                        if gap * -1.0
                            > ((end_time - start_time) as f64 / raw::NSTMODULUS as f64 + delta)
                        {
                            gap = -1.0 * (end_time - start_time) as f64 / raw::NSTMODULUS as f64
                                + delta;
                        }
                    }

                    let gap_str = if no_gap {
                        " == ".to_string()
                    } else if gap >= 86400.0 || gap <= -86400.0 {
                        format!("{:<3.1}d", gap / 86400.0)
                    } else if gap >= 3600.0 || gap <= -3600.0 {
                        format!("{:<3.1}h", gap / 3600.0)
                    } else if gap == 0.0 {
                        "-0  ".to_string()
                    } else {
                        format!("{:<4.4}", gap)
                    };

                    if self.detail <= 0 {
                        writeln!(
                            f,
                            "{:<27} {:<28} {:<28} {:<4}",
                            sid, start_time_str, end_time_str, gap_str
                        )?;
                    } else {
                        writeln!(
                            f,
                            "{:<27} {:<28} {:<28} {:<} {:<3.3} {:<}",
                            sid,
                            start_time_str,
                            end_time_str,
                            gap_str,
                            tseg.sample_rate_hz(),
                            tseg.sample_cnt()
                        )?;
                    }
                } else if self.detail > 0 && self.gap <= 0 {
                    writeln!(
                        f,
                        "{:<27} {:<28} {:<28} {:<3.3} {:<}",
                        sid,
                        start_time_str,
                        end_time_str,
                        tseg.sample_rate_hz(),
                        tseg.sample_cnt()
                    )?;
                } else {
                    writeln!(f, "{:<27} {:<28} {:<28}", sid, start_time_str, end_time_str)?;
                }

                tseg_cnt += 1;
            }

            tid_cnt += 1;
        }

        if self.detail > 0 {
            writeln!(
                f,
                "Total: {} trace(s) with {} segment(s)",
                tid_cnt, tseg_cnt
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use pretty_assertions::assert_eq;
    use time::format_description::well_known::Iso8601;

    use crate::{test, MSReader, MSSampleType};

    #[test]
    fn test_read_unpack_mstl_mseed3() {
        let mut p = test::test_data_base_dir();
        assert!(p.is_dir());

        p.push("testdata-oneseries-mixedlengths-mixedorder.mseed3");

        let mut mstl = MSTraceList::new().unwrap();

        let flags = MSControlFlags::MSF_UNPACKDATA;
        let mut reader = MSReader::new_with_flags(p, flags).unwrap();

        while let Some(res) = reader.next() {
            let msr = res.unwrap();
            mstl.insert(msr, true).unwrap();
        }

        assert_eq!(mstl.len(), 1);
        let mut mstl_iter = mstl.iter();
        let trace_id = mstl_iter.next();
        assert!(trace_id.is_some());
        let trace_id = trace_id.unwrap();
        assert_eq!(&trace_id.sid().unwrap(), "FDSN:XX_TEST_00_L_H_Z");
        assert_eq!(trace_id.pub_version(), 1);
        assert_eq!(trace_id.len(), 1);
        assert_eq!(
            trace_id
                .start_time()
                .unwrap()
                .format(&Iso8601::DEFAULT)
                .unwrap(),
            "2010-02-27T06:50:00.069539000Z"
        );
        assert_eq!(
            trace_id
                .end_time()
                .unwrap()
                .format(&Iso8601::DEFAULT)
                .unwrap(),
            "2010-02-27T07:55:51.069539000Z"
        );
        let mut trace_id_iter = trace_id.iter();
        let trace_seg = trace_id_iter.next();
        assert!(trace_seg.is_some());
        let mut trace_seg = trace_seg.unwrap();
        assert_eq!(
            trace_seg
                .start_time()
                .unwrap()
                .format(&Iso8601::DEFAULT)
                .unwrap(),
            "2010-02-27T06:50:00.069539000Z"
        );
        assert_eq!(
            trace_seg
                .end_time()
                .unwrap()
                .format(&Iso8601::DEFAULT)
                .unwrap(),
            "2010-02-27T07:55:51.069539000Z"
        );
        assert_eq!(trace_seg.sample_cnt(), 3952);
        assert_eq!(trace_seg.sample_type(), MSSampleType::Integer32);
        assert_eq!(trace_seg.num_samples(), 3952);

        let data_samples: &[i32] = trace_seg.data_samples().unwrap();
        // Test last 4 decoded sample values
        assert_eq!(data_samples[3948], 28067);
        assert_eq!(data_samples[3949], -9565);
        assert_eq!(data_samples[3950], -71961);
        assert_eq!(data_samples[3951], -146622);

        assert!(trace_id_iter.next().is_none());
        assert!(mstl_iter.next().is_none());
    }

    #[test]
    fn test_read_unpack_mstl_mseed2() {
        let mut p = test::test_data_base_dir();
        assert!(p.is_dir());

        p.push("testdata-oneseries-mixedlengths-mixedorder.mseed2");

        let mut mstl = MSTraceList::new().unwrap();

        let flags = MSControlFlags::MSF_UNPACKDATA;
        let mut reader = MSReader::new_with_flags(p, flags).unwrap();

        while let Some(res) = reader.next() {
            let msr = res.unwrap();
            mstl.insert(msr, true).unwrap();
        }

        assert_eq!(mstl.len(), 1);
        let mut mstl_iter = mstl.iter();
        let trace_id = mstl_iter.next();
        assert!(trace_id.is_some());
        let trace_id = trace_id.unwrap();
        assert_eq!(&trace_id.sid().unwrap(), "FDSN:XX_TEST_00_L_H_Z");
        assert_eq!(trace_id.pub_version(), 1);
        assert_eq!(trace_id.len(), 1);
        assert_eq!(
            trace_id
                .start_time()
                .unwrap()
                .format(&Iso8601::DEFAULT)
                .unwrap(),
            "2010-02-27T06:50:00.069539000Z"
        );
        assert_eq!(
            trace_id
                .end_time()
                .unwrap()
                .format(&Iso8601::DEFAULT)
                .unwrap(),
            "2010-02-27T07:55:51.069539000Z"
        );
        let mut trace_id_iter = trace_id.iter();
        let trace_seg = trace_id_iter.next();
        assert!(trace_seg.is_some());
        let mut trace_seg = trace_seg.unwrap();
        assert_eq!(
            trace_seg
                .start_time()
                .unwrap()
                .format(&Iso8601::DEFAULT)
                .unwrap(),
            "2010-02-27T06:50:00.069539000Z"
        );
        assert_eq!(
            trace_seg
                .end_time()
                .unwrap()
                .format(&Iso8601::DEFAULT)
                .unwrap(),
            "2010-02-27T07:55:51.069539000Z"
        );
        assert_eq!(trace_seg.sample_cnt(), 3952);
        assert_eq!(trace_seg.sample_type(), MSSampleType::Integer32);
        assert_eq!(trace_seg.num_samples(), 3952);

        let data_samples: &[i32] = trace_seg.data_samples().unwrap();
        // Test last 4 decoded sample values
        assert_eq!(data_samples[3948], 28067);
        assert_eq!(data_samples[3949], -9565);
        assert_eq!(data_samples[3950], -71961);
        assert_eq!(data_samples[3951], -146622);

        assert!(trace_id_iter.next().is_none());
        assert!(mstl_iter.next().is_none());
    }
}
