use crate::read_structs;
use crate::sys::{
    PERF_FLAG_FD_CLOEXEC, perf_event_attr, perf_event_read_format_PERF_FORMAT_GROUP,
    perf_event_read_format_PERF_FORMAT_ID, perf_event_read_format_PERF_FORMAT_TOTAL_TIME_ENABLED,
    perf_event_read_format_PERF_FORMAT_TOTAL_TIME_RUNNING,
};

use crate::perf_event_config::PerfConfig;
use crate::sys;

use core::ffi::c_void;
use libc::{c_int, c_long, c_ulong, pid_t};
use non_empty::{MaybeNonEmpty, NonEmpty};
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use zerocopy::FromBytes;

pub trait LazyOwned<U> {
    fn owned(self) -> U;
    fn borrowed(&self) -> &U;
}

impl<U> LazyOwned<U> for U {
    fn owned(self) -> U {
        self
    }
    fn borrowed(&self) -> &U {
        self
    }
}

impl<U: Clone> LazyOwned<U> for &U {
    fn owned(self) -> U {
        self.clone()
    }
    fn borrowed(&self) -> &U {
        self
    }
}

pub struct PerfEvent<NameType> {
    fd: OwnedFd,
    name: NameType,
}

fn perf_event_open(
    attr: &sys::perf_event_attr,
    pid: pid_t,
    cpu: c_int,
    group_fd: RawFd,
    flags: c_ulong,
) -> Result<OwnedFd, std::io::Error> {
    let fd = unsafe {
        libc::syscall(
            sys::__NR_perf_event_open as c_long,
            attr as *const perf_event_attr,
            pid,
            cpu,
            group_fd,
            flags,
        ) as i32
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

impl<NameType> PerfEvent<NameType> {
    pub fn new(config: &PerfConfig, pid: pid_t, name: NameType) -> Result<Self, io::Error> {
        Ok(PerfEvent {
            fd: perf_event_open(&(config.0), pid, -1, -1, PERF_FLAG_FD_CLOEXEC as u64)?,
            name,
        })
    }
}

pub struct PerfEventGroup<T> {
    fds: NonEmpty<PerfEvent<T>>,
}

impl<T> PerfEventGroup<T> {
    pub fn new<V: LazyOwned<PerfConfig>, I: IntoIterator<Item = (V, T)>>(
        input: I,
        pid: pid_t,
    ) -> Result<Self, crate::error::Error> {
        let mut it = input.into_iter();
        let (first_config, first_name) = it.next().ok_or(crate::error::Error::empty())?;
        let mut first_config: PerfConfig = first_config.owned();
        first_config.0.read_format = (perf_event_read_format_PERF_FORMAT_GROUP
            | perf_event_read_format_PERF_FORMAT_TOTAL_TIME_ENABLED
            | perf_event_read_format_PERF_FORMAT_TOTAL_TIME_RUNNING
            | perf_event_read_format_PERF_FORMAT_ID) as u64;
        let first_fd = perf_event_open(&first_config.0, pid, -1, -1, PERF_FLAG_FD_CLOEXEC as u64)?;
        let first_raw = first_fd.as_raw_fd();

        let rest_iterator = it.map(|(config, name)| {
            <Result<PerfEvent<T>, std::io::Error>>::Ok(PerfEvent {
                fd: perf_event_open(
                    &config.borrowed().0,
                    pid,
                    -1,
                    first_raw,
                    PERF_FLAG_FD_CLOEXEC as u64,
                )?,
                name,
            })
        });

        let collected: Result<MaybeNonEmpty<PerfEvent<T>>, std::io::Error> =
            core::iter::once(Ok(PerfEvent {
                fd: first_fd,
                name: first_name,
            }))
            .chain(rest_iterator)
            .collect();

        Ok(Self {
            fds: collected?
                .into_option()
                .ok_or(crate::error::Error::empty())?,
        })
    }

    pub fn len(&self) -> usize {
        self.fds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fds.is_empty()
    }

    pub fn leader_fd(&'_ self) -> BorrowedFd<'_> {
        self.fds.first().fd.as_fd()
    }

    pub fn read<'a>(
        &'_ self,
        buffer: &'a mut [u8],
    ) -> Result<
        (
            &'a read_structs::PerfGroupReadHeader,
            &'a [read_structs::PerfGroupReadEntry],
        ),
        crate::error::Error,
    > {
        let len = self.fds.len();
        let read_val = unsafe {
            libc::read(
                self.leader_fd().as_raw_fd(),
                buffer.as_mut_ptr() as *mut c_void,
                buffer.len(),
            )
        };
        if read_val < 0 {
            return Err(std::io::Error::last_os_error().into());
        } else if read_val as usize != buffer.len() {
            unreachable!();
        }

        let (header, remaining) =
            read_structs::PerfGroupReadHeader::ref_from_prefix(buffer).unwrap();
        debug_assert_eq!(header.nr, len as u64);
        let events =
            <[read_structs::PerfGroupReadEntry]>::ref_from_bytes_with_elems(remaining, len)
                .unwrap();
        Ok((header, events))
    }

    pub fn names(&self) -> impl Iterator<Item = &T> {
        self.fds.iter().map(|x| &x.name)
    }
}
