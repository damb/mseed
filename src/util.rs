use std::ffi::{c_char, c_long, CString};
use std::fmt;

use crate::error::{check, MSError};
use crate::{raw, MSResult};

/// Enumeration of time format identifiers.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MSTimeFormat {
    IsoMonthDay,
    IsoMonthDayZ,
    IsoMontDayDoy,
    IsoMonthDayDoyZ,
    IsoMonthDaySpace,
    IsoMonthDaySpaceZ,
    SeedOrdinal,
    UnixEpoch,
    NanoSecondEpoch,
}

impl MSTimeFormat {
    pub fn as_raw(&self) -> raw::ms_timeformat_t {
        use MSTimeFormat::*;

        match *self {
            IsoMonthDay => raw::ms_timeformat_t_ISOMONTHDAY,
            IsoMonthDayZ => raw::ms_timeformat_t_ISOMONTHDAY_Z,
            IsoMontDayDoy => raw::ms_timeformat_t_ISOMONTHDAY_DOY,
            IsoMonthDayDoyZ => raw::ms_timeformat_t_ISOMONTHDAY_DOY_Z,
            IsoMonthDaySpace => raw::ms_timeformat_t_ISOMONTHDAY_SPACE,
            IsoMonthDaySpaceZ => raw::ms_timeformat_t_ISOMONTHDAY_SPACE_Z,
            SeedOrdinal => raw::ms_timeformat_t_SEEDORDINAL,
            UnixEpoch => raw::ms_timeformat_t_UNIXEPOCH,
            NanoSecondEpoch => raw::ms_timeformat_t_NANOSECONDEPOCH,
        }
    }
}

/// Enumeration of subsecond format identifiers.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MSSubSeconds {
    None,
    Micro,
    Nano,
    MicroNone,
    NanoNone,
    NanoMicro,
    NanoMicroNone,
}

impl MSSubSeconds {
    pub fn as_raw(&self) -> raw::ms_subseconds_t {
        use MSSubSeconds::*;

        match *self {
            None => raw::ms_subseconds_t_NONE,
            Micro => raw::ms_subseconds_t_MICRO,
            Nano => raw::ms_subseconds_t_NANO,
            MicroNone => raw::ms_subseconds_t_MICRO_NONE,
            NanoNone => raw::ms_subseconds_t_NANO_NONE,
            NanoMicro => raw::ms_subseconds_t_NANO_MICRO,
            NanoMicroNone => raw::ms_subseconds_t_NANO_MICRO_NONE,
        }
    }
}

pub fn nstime_to_time(nst: c_long) -> MSResult<time::OffsetDateTime> {
    let mut year = 0;
    let mut yday = 0;
    let mut hour = 0;
    let mut min = 0;
    let mut sec = 0;
    let mut nsec = 0;
    unsafe {
        check(raw::ms_nstime2time(
            nst as _, &mut year, &mut yday, &mut hour, &mut min, &mut sec, &mut nsec,
        ))?
    };

    let date = time::Date::from_ordinal_date(year.into(), yday)
        .map_err(|e| MSError::from_str(&e.to_string()))?;
    let datetime = date
        .with_hms_nano(hour, min, sec, nsec)
        .map_err(|e| MSError::from_str(&e.to_string()))?;
    Ok(datetime.assume_utc())
}

/// Converts a nanosecond time into a time string.
pub fn nstime_to_string(
    nst: c_long,
    time_format: MSTimeFormat,
    subsecond_format: MSSubSeconds,
) -> MSResult<String> {
    let time = CString::new("                                     ")
        .unwrap()
        .into_raw();
    unsafe {
        if raw::ms_nstime2timestr(
            nst as _,
            time,
            time_format.as_raw(),
            subsecond_format.as_raw(),
        )
        .is_null()
        {
            return Err(MSError::from_str("failed to convert nstime to string"));
        }

        Ok(CString::from_raw(time).into_string().unwrap())
    }
}

pub fn time_to_nstime(t: &time::OffsetDateTime) -> i64 {
    t.unix_timestamp()
}

/// Utility function safely converting a slice of `c_char` values into a `String`.
pub(crate) fn to_string(buf: &[c_char]) -> String {
    let v: Vec<u8> = buf
        .iter()
        .map(|x| *x as u8) // cast i8 as u8
        .filter(|x| *x != 0u8) // remove null bytes
        .collect();

    String::from_utf8_lossy(&v).to_string()
}

/// A structure representing network, station, location, and channel identifiers.
#[derive(Debug, Clone)]
pub(crate) struct NetStaLocCha {
    /// Network identifier.
    pub net: String,
    /// Station identifier.
    pub sta: String,
    /// Location identifier.
    pub loc: String,
    /// Channel identifier stored as SEED 2.x channel identifier.
    pub cha: String,
}

impl NetStaLocCha {
    /// Creates a new `NSLC` structure from a [FDSN source
    /// identifier](https://docs.fdsn.org/projects/source-identifiers/) buffer slice.
    pub fn from_sid(sid: &[c_char]) -> MSResult<Self> {
        let s0 = "           ";
        let s1 = "                               ";
        let sid = CString::new(to_string(sid)).unwrap().into_raw();
        let xnet = CString::new(s0).unwrap().into_raw();
        let xsta = CString::new(s0).unwrap().into_raw();
        let xloc = CString::new(s0).unwrap().into_raw();
        let xcha = CString::new(s1).unwrap().into_raw();
        let rv = unsafe {
            check(raw::ms_sid2nslc(sid, xnet, xsta, xloc, xcha))?;
            let net = CString::from_raw(xnet).into_string().unwrap();
            let sta = CString::from_raw(xsta).into_string().unwrap();
            let loc = CString::from_raw(xloc).into_string().unwrap();
            let cha = CString::from_raw(xcha).into_string().unwrap();
            Self { net, sta, loc, cha }
        };

        Ok(rv)
    }
}

impl fmt::Display for NetStaLocCha {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let net = CString::new(self.net.as_str()).unwrap().into_raw();
        let sta = CString::new(self.sta.as_str()).unwrap().into_raw();
        let loc = CString::new(self.loc.as_str()).unwrap().into_raw();
        let cha = CString::new(self.cha.as_str()).unwrap().into_raw();

        let sid = CString::new(Vec::with_capacity(64)).unwrap().into_raw();
        let sid = unsafe {
            check(raw::ms_nslc2sid(sid, 64, 0, net, sta, loc, cha)).map_err(|_| fmt::Error)?;
            let sid = CString::from_raw(sid);
            sid.into_string().map_err(|_| fmt::Error)?
        };
        write!(f, "{}", sid)
    }
}

/// Converts an extended channel identifier to a SEED 2.x channel identifier.
///
/// See also [`seedchan2xchan`].
pub fn xchan2seedchan(xchan: &str) -> MSResult<String> {
    let xcha = CString::new(xchan)
        .map_err(|_| MSError::from_str(&format!("failed to convert xchan: {}", xchan)))?;

    let cha = CString::new(Vec::with_capacity(4))
        .map_err(|_| MSError::from_str(&format!("failed to convert xchan: {}", xchan)))?
        .into_raw();
    let cha = unsafe {
        check(raw::ms_xchan2seedchan(cha, xcha.as_ptr() as *const _))?;
        CString::from_raw(cha)
    };

    let rv = cha
        .into_string()
        .map_err(|_| MSError::from_str(&format!("failed to convert xchan: {}", xchan)))?;

    Ok(rv)
}

/// Converts a SEED 2.x channel identifier to an extended channel identifier.
///
/// See also [`xchan2seedchan`].
pub fn seedchan2xchan(seed_chan: &str) -> MSResult<String> {
    let cha = CString::new(seed_chan)
        .map_err(|_| MSError::from_str(&format!("failed to convert seed_chan: {}", seed_chan)))?;

    let xcha = CString::new(Vec::with_capacity(6))
        .map_err(|_| MSError::from_str(&format!("failed to convert seed_chan: {}", seed_chan)))?
        .into_raw();
    let xcha = unsafe {
        check(raw::ms_seedchan2xchan(xcha, cha.as_ptr() as *const _))?;
        CString::from_raw(xcha)
    };

    let rv = xcha
        .into_string()
        .map_err(|_| MSError::from_str(&format!("failed to convert seed_chan: {}", seed_chan)))?;

    Ok(rv)
}

#[cfg(test)]
mod tests {

    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn test_xchan2seedchan() {
        assert_eq!(xchan2seedchan("B_H_Z").unwrap(), "BHZ");
    }

    #[test]
    fn test_seedchan2xchan() {
        assert_eq!(seedchan2xchan("BHZ").unwrap(), "B_H_Z");
    }
}
