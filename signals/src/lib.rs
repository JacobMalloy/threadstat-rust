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

        for i in input.into_iter() {
            if unsafe { libc::sigaddset(rv.as_mut_ptr(), (*(i.borrow())) as libc::c_int) } == -1 {
                return Err(std::io::Error::last_os_error());
            }
        }

        Ok(unsafe { rv.assume_init() })
    }

    pub fn block<I, T>(signals: I) -> Result<(), std::io::Error>
    where
        I: IntoIterator<Item = T>,
        T: Borrow<Signal>,
    {
        let mask = Self::get_mask(signals)?;
        let res = unsafe { sigprocmask(libc::SIG_BLOCK, &mask as *const sigset_t, ptr::null_mut()) };
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
    pub fn new<I, T>(signals: I) -> Result<Self, std::io::Error>
    where
        I: IntoIterator<Item = T>,
        T: Borrow<Signal>,
    {
        let signal_mask = Signal::get_mask(signals)?;
        let rv = unsafe { libc::signalfd(-1, &signal_mask as *const sigset_t, libc::SFD_CLOEXEC) };

        Ok(SignalFD {
            fd: unsafe { OwnedFd::from_raw_fd(rv) },
        })
    }

    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub fn read(&self) -> Result<libc::signalfd_siginfo, std::io::Error> {
        let mut info: MaybeUninit<libc::signalfd_siginfo> = MaybeUninit::uninit();
        let read = unsafe {
            libc::read(
                self.fd.as_raw_fd(),
                info.as_mut_ptr() as *mut libc::c_void,
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

