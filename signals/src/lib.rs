use core::borrow::Borrow;
use core::mem;
use std::{
    mem::MaybeUninit,
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd},
    ptr,
};

use libc::{sigprocmask, sigset_t};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Signal {
    SIGHUP = libc::SIGHUP,
    SIGINT = libc::SIGINT,
    SIGQUIT = libc::SIGQUIT,
    SIGILL = libc::SIGILL,
    SIGABRT = libc::SIGABRT,
    SIGFPE = libc::SIGFPE,
    SIGKILL = libc::SIGKILL,
    SIGSEGV = libc::SIGSEGV,
    SIGPIPE = libc::SIGPIPE,
    SIGALRM = libc::SIGALRM,
    SIGTERM = libc::SIGTERM,
}

impl Signal {
    fn get_mask<I, T>(input: I) -> Result<sigset_t, std::io::Error>
    where
        I: IntoIterator<Item = T>,
        T: Borrow<Signal>,
    {
        let mut rv: mem::MaybeUninit<sigset_t> = mem::MaybeUninit::uninit();
        if unsafe { libc::sigemptyset(rv.as_mut_ptr()) } == -1 {
            return Err(std::io::Error::last_os_error());
        }

        for i in input {
            if unsafe { libc::sigaddset(rv.as_mut_ptr(), (*(i.borrow())) as libc::c_int) } == -1 {
                return Err(std::io::Error::last_os_error());
            }
        }

        Ok(unsafe { rv.assume_init() })
    }

    /// # Errors
    /// Returns an error if `sigprocmask` or signal set construction fails.
    pub fn block<I, T>(signals: I) -> Result<(), std::io::Error>
    where
        I: IntoIterator<Item = T>,
        T: Borrow<Signal>,
    {
        let mask = Self::get_mask(signals)?;
        let res = unsafe { sigprocmask(libc::SIG_BLOCK, &raw const mask, ptr::null_mut()) };
        if res == -1{
            Err(std::io::Error::last_os_error())
        }else{
            Ok(())
        }
    }
}

pub struct SignalFD {
    fd: OwnedFd,
}

impl SignalFD {
    /// # Errors
    /// Returns an error if signal set construction or `signalfd(2)` fails.
    pub fn new<I, T>(signals: I) -> Result<Self, std::io::Error>
    where
        I: IntoIterator<Item = T>,
        T: Borrow<Signal>,
    {
        let signal_mask = Signal::get_mask(signals)?;
        let rv = unsafe { libc::signalfd(-1, &raw const signal_mask, libc::SFD_CLOEXEC) };
        if rv == -1 {
            // OwnedFd's invariant is that it holds an open fd, and -1 is its niche.
            return Err(std::io::Error::last_os_error());
        }

        Ok(SignalFD {
            fd: unsafe { OwnedFd::from_raw_fd(rv) },
        })
    }
    
    #[must_use]
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// # Errors
    /// Returns an error if the underlying `read(2)` syscall fails.
    pub fn read(&mut self) -> Result<libc::signalfd_siginfo, std::io::Error> {
        let mut info: MaybeUninit<libc::signalfd_siginfo> = MaybeUninit::uninit();
        let read = unsafe {
            libc::read(
                self.fd.as_raw_fd(),
                info.as_mut_ptr().cast(),
                size_of::<libc::signalfd_siginfo>(),
            )
        };

        if read < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(unsafe { info.assume_init() })
        }
    }
}

impl AsFd for SignalFD {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl poll::Pollable for SignalFD {
    fn pollable_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

