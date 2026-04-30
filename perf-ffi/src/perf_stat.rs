use crate::read_structs;
use crate::sys::{
    PERF_FLAG_FD_CLOEXEC, perf_event_read_format_PERF_FORMAT_GROUP,
    perf_event_read_format_PERF_FORMAT_ID, perf_event_read_format_PERF_FORMAT_TOTAL_TIME_ENABLED,
    perf_event_read_format_PERF_FORMAT_TOTAL_TIME_RUNNING,
};
use std::ptr;
use crate::perf_event_config::PerfConfig;
use crate::sys;

use core::mem::MaybeUninit;
use libc::{c_int, c_ulong, pid_t};
use non_empty::{MaybeNonEmpty, NonEmpty};
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use zerocopy::FromBytes;

pub struct PerfStat<NameType> {
    fd: OwnedFd,
    name: NameType,
}

impl<NameType> PerfStat<NameType> {
    fn get_id(&self) -> Result<u64, std::io::Error> {
        let mut return_value: u64 = 0;
        let ioctl_res =
            unsafe { libc::ioctl(self.fd.as_raw_fd(), sys::PERF_EVENT_IOC_ID_CONST, &mut return_value) };
        if ioctl_res != 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(return_value)
        }
    }
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
            sys::__NR_perf_event_open.cast_signed().into(),
            ptr::from_ref(attr),
            pid,
            cpu,
            group_fd,
            flags,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        #[allow(clippy::cast_possible_truncation)]
        let raw = fd as RawFd;
        Ok(unsafe { OwnedFd::from_raw_fd(raw) })
    }
}

pub struct PerfStatGroup<T> {
    fds: NonEmpty<PerfStat<T>>,
}

impl<T: Clone> PerfStatGroup<T> {
    /// # Errors
    /// Returns [`crate::error::Error::Empty`] if `input` is empty, or
    /// [`crate::error::Error::IO`] if any `perf_event_open(2)` syscall fails.
    pub fn new<V: AsRef<PerfConfig<T>>, I: IntoIterator<Item = V>>(
        input: I,
        pid: pid_t,
    ) -> Result<Self, crate::error::Error<'static>> {
        let mut it = input.into_iter();
        let first = it.next().ok_or(crate::error::Error::empty())?;
        let first_config = first.as_ref();
        let mut first_attr = first_config.attr;
        first_attr.read_format = (perf_event_read_format_PERF_FORMAT_GROUP
            | perf_event_read_format_PERF_FORMAT_TOTAL_TIME_ENABLED
            | perf_event_read_format_PERF_FORMAT_TOTAL_TIME_RUNNING
            | perf_event_read_format_PERF_FORMAT_ID).into();
        let first_fd = perf_event_open(&first_attr, pid, -1, -1, PERF_FLAG_FD_CLOEXEC.into())?;
        let first_raw = first_fd.as_raw_fd();
        let first_name = first_config.name.clone();

        let rest_iterator = it.map(|config| {
            let c = config.as_ref();
            <Result<PerfStat<T>, std::io::Error>>::Ok(PerfStat {
                fd: perf_event_open(&c.attr, pid, -1, first_raw, PERF_FLAG_FD_CLOEXEC.into())?,
                name: c.name.clone(),
            })
        });

        let collected: Result<MaybeNonEmpty<PerfStat<T>>, std::io::Error> =
            core::iter::once(Ok(PerfStat {
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
}

impl<T> PerfStatGroup<T> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.fds.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fds.is_empty()
    }

    #[must_use]
    pub fn leader_fd(&'_ self) -> BorrowedFd<'_> {
        self.fds.first().fd.as_fd()
    }

    /// # Errors
    /// Returns an error if the underlying `read(2)` fails or if the kernel data
    /// cannot be interpreted as the expected perf group read structures.
    pub fn read<'a>(
        &'_ self,
        buffer: &'a mut [MaybeUninit<u8>],
    ) -> Result<
        (
            &'a read_structs::PerfGroupReadHeader,
            &'a [read_structs::PerfGroupReadEntry],
        ),
        crate::error::Error<'a>,
    > {
        let len = self.fds.len();
        let read_val = unsafe {
            libc::read(
                self.leader_fd().as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            )
        };
        if read_val < 0 {
            return Err(std::io::Error::last_os_error().into());
        } else if read_val.cast_unsigned() != buffer.len() {
            debug_assert!(false);
        }
        let buffer: &[u8] = unsafe { buffer.assume_init_ref() };
        let (header, remaining) =
            read_structs::PerfGroupReadHeader::ref_from_prefix(buffer)?;
        debug_assert_eq!(header.nr, len as u64);
        let events =
            <[read_structs::PerfGroupReadEntry]>::ref_from_bytes_with_elems(remaining, len)?;
        Ok((header, events))
    }

    pub fn names(&self) -> impl Iterator<Item = &T> {
        self.fds.iter().map(|x| &x.name)
    }

    pub fn name_and_ids(&self) -> impl Iterator<Item = Result<(&T, u64), std::io::Error>> {
        self.fds.iter().map(|x| Ok((&x.name, x.get_id()?)))
    }
}
