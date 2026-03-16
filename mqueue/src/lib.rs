use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::{
    ffi::CStr,
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd},
};

use std::ptr;

pub struct MQueueReader<'a, T> {
    fd: OwnedFd,
    name: &'a CStr,
    _phantom: PhantomData<T>,
}

impl<'a, T> MQueueReader<'a, T> {
    pub fn new(path: &'a CStr) -> Result<Self, std::io::Error> {
        let mut attr:libc::mq_attr = unsafe{core::mem::zeroed()};
        attr.mq_msgsize=size_of::<T>() as i64;
        attr.mq_maxmsg=50;
        
        let ret = unsafe {
            libc::mq_open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_CREAT,
                0o666u32,
                &attr,
            )
        };
        if ret == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            unsafe { libc::fchmod(ret, 0o666) };
            Ok(Self {
                fd: unsafe { OwnedFd::from_raw_fd(ret) },
                name: path,
                _phantom: PhantomData,
            })
        }
    }

    pub fn read(&self) -> Result<T, MQError> {
        let mut rv: MaybeUninit<T> = MaybeUninit::uninit();
        let size = size_of::<T>();
        let ret = unsafe {
            libc::mq_receive(
                self.fd.as_fd().as_raw_fd(),
                rv.as_mut_ptr() as *mut i8,
                size,
                ptr::null_mut(),
            )
        };
        if ret == -1 {
            Err(MQError::IO(std::io::Error::last_os_error()))
        } else if ret as usize != size {
            Err(MQError::WrongSize)
        } else {
            Ok(unsafe { rv.assume_init() })
        }
    }
}

impl<T> AsFd for MQueueReader<'_, T> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl<T> poll::Pollable for MQueueReader<'_, T> {
    fn pollable_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl<'a, T> Drop for MQueueReader<'a, T> {
    fn drop(&mut self) {
        unsafe {
            // OwnedFd handles mq_close on drop; we only need mq_unlink here.
            libc::mq_unlink(self.name.as_ptr());
        }
    }
}

pub enum MQError {
    IO(std::io::Error),
    WrongSize,
}

impl core::error::Error for MQError {}

impl core::fmt::Display for MQError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MQError::IO(e) => write!(f, "IO Error: {e}"),
            MQError::WrongSize => write!(f, "Incorrect mqueue size read"),
        }
    }
}

impl core::fmt::Debug for MQError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MQError::IO(e) => write!(f, "IO Error: {e:?}"),
            MQError::WrongSize => write!(f, "Incorrect mqueue size read"),
        }
    }
}
