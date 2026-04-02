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
    /// # Errors
    /// Returns an error if `mq_open(3)` fails (e.g. permission denied, invalid path).
    pub fn new(path: &'a CStr) -> Result<Self, std::io::Error> {
        let mut attr: libc::mq_attr = unsafe { core::mem::zeroed() };
        
        #[allow(clippy::cast_possible_wrap)]
        let msgsize = size_of::<T>() as i64;

        attr.mq_msgsize = msgsize;
        attr.mq_maxmsg = 50;
        let prev_umask = unsafe { libc::umask(0) };
        let ret = unsafe {
            libc::mq_open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_CREAT,
                0o666u32,
                &attr,
            )
        };
        unsafe {libc::umask(prev_umask)};
        if ret == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(Self {
                fd: unsafe { OwnedFd::from_raw_fd(ret) },
                name: path,
                _phantom: PhantomData,
            })
        }
    }

    /// # Errors
    /// Returns [`MQError::IO`] if `mq_receive(3)` fails, or [`MQError::WrongSize`] if the
    /// received message length does not match `size_of::<T>()`.
    pub fn read(&self) -> Result<T, MQError> {
        let mut rv: MaybeUninit<T> = MaybeUninit::uninit();
        let size = size_of::<T>();
        let ret = unsafe {
            libc::mq_receive(
                self.fd.as_fd().as_raw_fd(),
                rv.as_mut_ptr().cast(),
                size,
                ptr::null_mut(),
            )
        };
        if ret == -1 {
            Err(MQError::IO(std::io::Error::last_os_error()))
        } else if ret.cast_unsigned() == size {
            Ok(unsafe { rv.assume_init() })
        } else {
            Err(MQError::WrongSize)
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

impl<T> Drop for MQueueReader<'_, T> {
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
