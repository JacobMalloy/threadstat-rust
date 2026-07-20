use std::io::ErrorKind;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::ptr;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Clock {
    /// Counts up from an unspecified point and is unaffected by clock adjustments. This is
    /// what you want for intervals.
    Monotonic = libc::CLOCK_MONOTONIC,
    /// Wall clock, so a settimeofday or an NTP step moves the expiry.
    Realtime = libc::CLOCK_REALTIME,
}

const ZERO: libc::timespec = libc::timespec {
    tv_sec: 0,
    tv_nsec: 0,
};

fn too_large() -> std::io::Error {
    std::io::Error::new(ErrorKind::InvalidInput, "duration exceeds timespec")
}

fn to_timespec(d: Duration) -> Result<libc::timespec, std::io::Error> {
    Ok(libc::timespec {
        tv_sec: libc::time_t::try_from(d.as_secs()).map_err(|_| too_large())?,
        // Infallible where c_long is 64-bit, but not on a 32-bit target, so keep try_from.
        #[allow(clippy::unnecessary_fallible_conversions)]
        tv_nsec: libc::c_long::try_from(d.subsec_nanos()).map_err(|_| too_large())?,
    })
}

pub struct TimerFD {
    fd: OwnedFd,
}

impl TimerFD {
    /// Creates a disarmed timer. Arm it with [`TimerFD::arm_periodic`] or
    /// [`TimerFD::arm_oneshot`] before expecting it to fire.
    ///
    /// # Errors
    /// Returns an error if `timerfd_create(2)` fails.
    pub fn new(clock: Clock) -> Result<Self, std::io::Error> {
        let rv = unsafe { libc::timerfd_create(clock as libc::clockid_t, libc::TFD_CLOEXEC) };
        if rv == -1 {
            // OwnedFd's invariant is that it holds an open fd, and -1 is its niche.
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self {
            fd: unsafe { OwnedFd::from_raw_fd(rv) },
        })
    }

    fn settime(&mut self, spec: &libc::itimerspec) -> Result<(), std::io::Error> {
        let res = unsafe { libc::timerfd_settime(self.fd.as_raw_fd(), 0, spec, ptr::null_mut()) };
        if res == -1 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// Fires once after `period` and every `period` thereafter, replacing any current arming.
    ///
    /// # Errors
    /// Returns an error if `period` is zero or does not fit a `timespec`, or if
    /// `timerfd_settime(2)` fails.
    pub fn arm_periodic(&mut self, period: Duration) -> Result<(), std::io::Error> {
        if period.is_zero() {
            // it_value of zero means "disarm" to the kernel, which is not what a caller
            // asking for a period could possibly want.
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "a zero period would disarm the timer; call disarm() instead",
            ));
        }
        let ts = to_timespec(period)?;
        self.settime(&libc::itimerspec {
            it_interval: ts,
            it_value: ts,
        })
    }

    /// Fires once after `delay` and then stays quiet, replacing any current arming.
    ///
    /// # Errors
    /// Returns an error if `delay` is zero or does not fit a `timespec`, or if
    /// `timerfd_settime(2)` fails.
    pub fn arm_oneshot(&mut self, delay: Duration) -> Result<(), std::io::Error> {
        if delay.is_zero() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "a zero delay would disarm the timer; call disarm() instead",
            ));
        }
        self.settime(&libc::itimerspec {
            it_interval: ZERO,
            it_value: to_timespec(delay)?,
        })
    }

    /// Stops the timer. It will not fire again until re-armed.
    ///
    /// # Errors
    /// Returns an error if `timerfd_settime(2)` fails.
    pub fn disarm(&mut self) -> Result<(), std::io::Error> {
        self.settime(&libc::itimerspec {
            it_interval: ZERO,
            it_value: ZERO,
        })
    }

    /// Consumes the pending expirations and returns how many there were, which is greater
    /// than one if the reader fell behind a periodic timer. Blocks until the timer fires.
    ///
    /// # Errors
    /// Returns an error if `read(2)` fails or returns a short count.
    pub fn read(&mut self) -> Result<u64, std::io::Error> {
        let mut expirations: u64 = 0;
        let read = unsafe {
            libc::read(
                self.fd.as_raw_fd(),
                (&raw mut expirations).cast(),
                size_of::<u64>(),
            )
        };
        if read < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if read.cast_unsigned() as usize != size_of::<u64>() {
            return Err(std::io::Error::new(
                ErrorKind::UnexpectedEof,
                "short read from timerfd",
            ));
        }
        Ok(expirations)
    }
}

impl AsFd for TimerFD {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl poll::Pollable for TimerFD {
    fn pollable_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, TimerFD};
    use poll::{PollAction, Poller};
    use std::time::{Duration, Instant};

    #[test]
    fn oneshot_fires_once_after_the_delay() {
        let mut timer = TimerFD::new(Clock::Monotonic).unwrap();
        let start = Instant::now();
        timer.arm_oneshot(Duration::from_millis(20)).unwrap();

        assert_eq!(timer.read().unwrap(), 1);
        assert!(start.elapsed() >= Duration::from_millis(20));
    }

    #[test]
    fn periodic_keeps_firing() {
        let mut timer = TimerFD::new(Clock::Monotonic).unwrap();
        timer.arm_periodic(Duration::from_millis(5)).unwrap();

        let mut ticks = 0;
        for _ in 0..3 {
            ticks += timer.read().unwrap();
        }
        assert!(ticks >= 3, "expected at least one expiration per read");
    }

    #[test]
    fn falling_behind_reports_the_backlog() {
        let mut timer = TimerFD::new(Clock::Monotonic).unwrap();
        timer.arm_periodic(Duration::from_millis(2)).unwrap();
        std::thread::sleep(Duration::from_millis(50));

        assert!(
            timer.read().unwrap() > 1,
            "a missed periodic timer should report every expiration, not just the last"
        );
    }

    /// The count resets on every successful read, so it is a delta and never a running total.
    #[test]
    fn read_counts_expirations_since_the_last_read() {
        let mut timer = TimerFD::new(Clock::Monotonic).unwrap();
        timer.arm_periodic(Duration::from_millis(2)).unwrap();

        std::thread::sleep(Duration::from_millis(40));
        let first = timer.read().unwrap();
        assert!(first >= 10, "40ms of a 2ms timer should back up, got {first}");

        std::thread::sleep(Duration::from_millis(10));
        let second = timer.read().unwrap();
        assert!(
            second < first,
            "read must reset the counter, not accumulate: {first} then {second}"
        );
    }

    #[test]
    fn zero_duration_is_rejected_rather_than_silently_disarming() {
        let mut timer = TimerFD::new(Clock::Monotonic).unwrap();
        assert!(timer.arm_periodic(Duration::ZERO).is_err());
        assert!(timer.arm_oneshot(Duration::ZERO).is_err());
    }

    #[test]
    fn disarm_stops_a_periodic_timer() {
        let mut timer = TimerFD::new(Clock::Monotonic).unwrap();
        timer.arm_periodic(Duration::from_millis(5)).unwrap();
        timer.read().unwrap();
        timer.disarm().unwrap();

        // Nothing should be pending now; a fresh oneshot is the only thing that wakes it.
        timer.arm_oneshot(Duration::from_millis(10)).unwrap();
        assert_eq!(timer.read().unwrap(), 1);
    }

    #[test]
    fn drives_a_poller() {
        let mut timer = TimerFD::new(Clock::Monotonic).unwrap();
        timer.arm_periodic(Duration::from_millis(5)).unwrap();

        let mut ticks: u64 = 0;
        let mut poller = Poller::new();
        poller.register_mut(&mut timer, |t, ticks: &mut u64| {
            *ticks += t.read()?;
            if *ticks >= 3 {
                return Ok(PollAction::Stop);
            }
            Ok(PollAction::Continue)
        });
        poller.run(&mut ticks).unwrap();

        assert!(ticks >= 3);
    }
}
