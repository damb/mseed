use std::ffi::{c_char, c_double, c_float, c_int, c_long, c_uchar, c_uint};
use std::ptr;
use std::slice::from_raw_parts;

use crate::{error::check, raw, util, MSControlFlags, MSError, MSRecord, MSResult, MSSampleType};
use time::OffsetDateTime;

use raw::{MS3TraceID, MS3TraceList, MS3TraceSeg};

/// A container for a trace identifier composed by [`MSTraceSegment`]s.
#[derive(Debug)]
pub struct MSTraceId(*mut MS3TraceID);

impl MSTraceId {
    fn ptr(&self) -> MS3TraceID {
        unsafe { *self.0 }
    }

    pub(crate) fn get_raw(&self) -> *const MS3TraceID {
        self.0
    }

    pub(crate) unsafe fn get_raw_mut(&self) -> *mut MS3TraceID {
        self.0
    }

    /// Returns the FDSN source identifier.
    ///
    /// FDSN source identifiers are defined at:
    /// `<https://docs.fdsn.org/projects/source-identifiers/>`
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
        util::nstime_to_time(self.ptr().earliest)
    }

    /// Returns the time of the the last sample.
    pub fn end_time(&self) -> MSResult<OffsetDateTime> {
        util::nstime_to_time(self.ptr().latest)
    }

    /// Returns the number of [`MSTraceSegment`]s for this trace identifier.
    pub fn len(&self) -> c_uint {
        self.ptr().numsegments
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
    trace_id: &'id MSTraceId,

    inner: *mut MS3TraceSeg,
}

impl<'id> MSTraceSegment<'id> {
    fn ptr(&self) -> MS3TraceSeg {
        unsafe { *self.inner }
    }

    /// Returns the time of the the first sample.
    pub fn start_time(&self) -> MSResult<OffsetDateTime> {
        util::nstime_to_time(self.ptr().starttime)
    }

    /// Returns the time of the the last sample.
    pub fn end_time(&self) -> MSResult<OffsetDateTime> {
        util::nstime_to_time(self.ptr().endtime)
    }

    /// Returns the nominal sample rate as samples per second (`Hz`)
    pub fn sample_rate_hz(&self) -> c_double {
        self.ptr().samprate
    }

    /// Returns the number of samples in trace coverage.
    pub fn sample_cnt(&self) -> c_long {
        self.ptr().samplecnt
    }

    /// Returns the data samples of the trace segment.
    ///
    /// Note that the data samples must have been unpacked, previously. Deferred unpacking of data
    /// samples from the internal record list is currently not implemented.
    pub fn data_samples<T: DataSampleType>(&mut self) -> MSResult<&[T]> {
        if !self.is_data_unpacked() {
            return Err(MSError::from_str("data samples must be unpacked"));
        }

        <T as DataSampleType>::convert_into(self.inner, false)?;

        let rv = unsafe {
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
        self.ptr().numsamples
    }

    /// Returns the trace segment sample type.
    pub fn sample_type(&self) -> MSSampleType {
        MSSampleType::from_char(self.ptr().sampletype)
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
    fn convert_into(seg: *mut MS3TraceSeg, truncate: bool) -> MSResult<()>;
}

impl DataSampleType for c_uchar {
    fn convert_into(_seg: *mut MS3TraceSeg, _truncate: bool) -> MSResult<()> {
        Ok(())
    }
}

impl DataSampleType for c_int {
    fn convert_into(seg: *mut MS3TraceSeg, truncate: bool) -> MSResult<()> {
        let rv = unsafe {
            check(raw::mstl3_convertsamples(
                seg,
                MSSampleType::Integer32 as i8,
                truncate as i8,
            ))
        };

        match rv {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

impl DataSampleType for c_float {
    fn convert_into(seg: *mut MS3TraceSeg, truncate: bool) -> MSResult<()> {
        let rv = unsafe {
            check(raw::mstl3_convertsamples(
                seg,
                MSSampleType::Float32 as i8,
                truncate as i8,
            ))
        };

        match rv {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

impl DataSampleType for c_double {
    fn convert_into(seg: *mut MS3TraceSeg, truncate: bool) -> MSResult<()> {
        let rv = unsafe {
            check(raw::mstl3_convertsamples(
                seg,
                MSSampleType::Float64 as i8,
                truncate as i8,
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
            trace_id: self.trace_id,
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
            trace_id: self.trace_id,
            inner: self.prev,
        });
        self.next = self.prev;
        self.prev = unsafe { (*self.prev).prev };
        rv
    }
}

/// A container for [`MSTraceId`]s.
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
/// let mstl_iter = mstl.iter();
/// for tid in mstl_iter {
///     let tid_iter = tid.iter();
///     for tseg in tid_iter {
///         // do something with `tseg`
///     }
/// }
/// ```
#[derive(Debug)]
pub struct MSTraceList {
    inner: *mut MS3TraceList,
}

impl MSTraceList {
    fn ptr(&self) -> MS3TraceList {
        unsafe { *self.inner }
    }

    pub(crate) fn get_raw(&mut self) -> *const MS3TraceList {
        self.inner
    }

    pub(crate) unsafe fn get_raw_mut(&mut self) -> *mut MS3TraceList {
        self.inner
    }

    /// Creates a new [`MSTraceList`] container
    pub fn new() -> MSResult<Self> {
        let mstl: *mut MS3TraceList = ptr::null_mut();
        let mstl = unsafe { raw::mstl3_init(mstl) };
        if mstl.is_null() {
            return Err(MSError::from_str("failed to initialize trace list"));
        }

        Ok(Self { inner: mstl })
    }

    /// Returns the length of the list.
    pub fn len(&self) -> c_uint {
        self.ptr().numtraceids
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
                autoheal as c_char,
                MSControlFlags::empty().bits(),
                ptr::null_mut(),
            )
        };

        if rv.is_null() {
            return Err(MSError::from_str("failed to insert record"));
        }

        Ok(())
    }
}

impl Drop for MSTraceList {
    fn drop(&mut self) {
        unsafe { raw::mstl3_free((&mut self.inner) as *mut *mut MS3TraceList, 1) };
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use pretty_assertions::assert_eq;
    use time::format_description::well_known::Iso8601;

    use crate::{test, MSReader, MSSampleType};

    fn foo() {
        use std::fs::File;

        use crate::{MSControlFlags, MSReader, MSTraceList};

        let mut mstl = MSTraceList::new().unwrap();

        let mut reader =
            MSReader::new_with_flags("path/to/data.mseed", MSControlFlags::MSF_UNPACKDATA).unwrap();

        while let Some(res) = reader.next() {
            let msr = res.unwrap();

            if msr.network().unwrap() == "NET" && msr.station().unwrap() == "STA" {
                mstl.insert(msr, true).unwrap();
            }
        }

        let mstl_iter = mstl.iter();
        for tid in mstl_iter {
            let tid_iter = tid.iter();
            for tseg in tid_iter {
                // do something with `tseg`
            }
        }
    }

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
