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

/// How deep we would like the queue to be, subject to what the kernel allows.
const DESIRED_MAXMSG: i64 = 50;

/// The kernel's ceiling on queue depth for an unprivileged `mq_open`. Falls back to the
/// documented default if `/proc` is not mounted.
fn kernel_maxmsg_limit() -> i64 {
    std::fs::read_to_string("/proc/sys/fs/mqueue/msg_max")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(10)
}

impl<'a, T> MQueueReader<'a, T> {
    fn open(path: &CStr, maxmsg: i64) -> Result<OwnedFd, std::io::Error> {
        let mut attr: libc::mq_attr = unsafe { core::mem::zeroed() };

        #[allow(clippy::cast_possible_wrap)]
        let msgsize = size_of::<T>() as i64;

        attr.mq_msgsize = msgsize;
        attr.mq_maxmsg = maxmsg;
        let prev_umask = unsafe { libc::umask(0) };
        let ret = unsafe {
            libc::mq_open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_CREAT,
                0o666u32,
                &attr,
            )
        };
        unsafe { libc::umask(prev_umask) };
        if ret == -1 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(unsafe { OwnedFd::from_raw_fd(ret) })
    }

    /// # Errors
    /// Returns an error if `mq_open(3)` fails (e.g. permission denied, invalid path).
    pub fn new(path: &'a CStr) -> Result<Self, std::io::Error> {
        let fd = match Self::open(path, DESIRED_MAXMSG) {
            // Without CAP_SYS_RESOURCE, a depth above fs.mqueue.msg_max is EINVAL rather than
            // being clamped, so drop to whatever this kernel is configured to allow.
            Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => {
                Self::open(path, kernel_maxmsg_limit())?
            }
            other => other?,
        };

        Ok(Self {
            fd,
            name: path,
            _phantom: PhantomData,
        })
    }

    /// # Errors
    /// Returns [`MQError::IO`] if `mq_receive(3)` fails, or [`MQError::WrongSize`] if the
    /// received message length does not match `size_of::<T>()`.
    pub fn read(&mut self) -> Result<T, MQError> {
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

#[cfg(test)]
mod tests {
    use super::{DESIRED_MAXMSG, MQueueReader, kernel_maxmsg_limit};

    /// On a stock kernel fs.mqueue.msg_max is 10, well under what we ask for, and an
    /// unprivileged mq_open answers that with EINVAL rather than clamping.
    #[test]
    fn opens_even_when_desired_depth_exceeds_the_kernel_limit() {
        let reader: MQueueReader<u64> = MQueueReader::new(c"/threadstat-mqueue-depth-test")
            .expect("should fall back to the kernel's msg_max");
        drop(reader);
    }

    #[test]
    fn reports_a_plausible_kernel_limit() {
        let limit = kernel_maxmsg_limit();
        assert!(limit > 0, "queue depth limit should be positive, got {limit}");
        assert!(
            DESIRED_MAXMSG > 0,
            "the desired depth is what we try before falling back"
        );
    }
}
