use crate::{PerfConfig, sys};
use crate::perf_event_config;
use core::ffi::{CStr, c_void};
use core::{error, fmt, ptr};
use std::mem;
use std::sync::LazyLock;

fn pfm_strerror(code: sys::pfm_err_t) -> Option<&'static CStr> {
    let raw = unsafe { sys::pfm_strerror(code) };
    if raw.is_null(){
        None
    }else{
        Some(unsafe {CStr::from_ptr(raw)})
    }
    
}

pub static PFM: LazyLock<Result<PFMInterior, Error>> = LazyLock::new(|| {
    let ret = unsafe { sys::pfm_initialize() };
    if ret != sys::PFM_SUCCESS as i32 {
        Err(Error(ret))
    } else {
        Ok(PFMInterior::default())
    }
});

#[derive(Default)]
pub struct PFMInterior {}

impl PFMInterior {
    fn get_perf_attr_array(&self, event: impl AsRef<CStr>) -> Result<sys::perf_event_attr, Error> {
        let e = event.as_ref();
        let mut rv = unsafe { mem::zeroed() };
        let mut pfm_arg: sys::pfm_perf_encode_arg_t = sys::pfm_perf_encode_arg_t {
            attr: ptr::addr_of_mut!(rv),
            fstr: ptr::null_mut(),
            size: size_of::<sys::pfm_perf_encode_arg_t>(),
            idx: 0,
            cpu: 0,
            flags: 0,
            pad0: 0,
        };
        let ret = unsafe {
            sys::pfm_get_os_event_encoding(
                e.as_ptr(),
                (sys::PFM_PLM3 | sys::PFM_PLM0 | sys::PFM_PLMH) as i32,
                sys::pfm_os_t_PFM_OS_PERF_EVENT,
                ptr::addr_of_mut!(pfm_arg) as *mut c_void,
            )
        };

        if ret != sys::PFM_SUCCESS as i32 {
            Err(Error(ret))
        } else {
            Ok(rv)
        }
    }
}

impl perf_event_config::PerfConfig {
    pub fn from_pfm_string(event: impl AsRef<CStr>) -> Result<PerfConfig, Error> {
        Ok(PerfConfig(
            PFM.as_ref()
                .map_err(Clone::clone)?
                .get_perf_attr_array(event)?,
        ))
    }
}

#[derive(Copy, Clone)]
pub struct Error(sys::pfm_err_t);

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = self.0;
        let error =
            pfm_strerror(code).map(|x| x.to_str().unwrap_or("Error CStr contained invalid UTF-8"));
        match error {
            None => write!(f, "libpfm ERROR:pfm_strerror failed on code {code}"),
            Some(s) => write!(f, "libpfm ERROR: {s}"),
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as fmt::Display>::fmt(self, f)
    }
}
