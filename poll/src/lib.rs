use slab::Slab;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::ptr;

pub trait Pollable {
    fn pollable_fd(&self) -> BorrowedFd<'_>;
}

pub enum PollAction {
    /// Leave the source registered and keep polling.
    Continue,
    /// Tear down the whole polling loop.
    Stop,
    /// Deregister just this source; the remaining sources keep polling.
    Remove,
}

/// `C` is the context handed to every handler when it runs. Handlers only ever run one at a
/// time, so passing it down per call lets several of them share one `&mut` without a cell.
trait HandleTrait<C> {
    fn call_func(&mut self, ctx: &mut C) -> Result<PollAction, std::io::Error>;
}

struct HandleMut<'a, P, F> {
    r: &'a mut P,
    func: F,
}

impl<C, P, F> HandleTrait<C> for HandleMut<'_, P, F>
where
    F: FnMut(&mut P, &mut C) -> Result<PollAction, std::io::Error>,
{
    fn call_func(&mut self, ctx: &mut C) -> Result<PollAction, std::io::Error> {
        (self.func)(self.r, ctx)
    }
}

struct Source<'a, C> {
    /// Kept so the source can be handed to `EPOLL_CTL_DEL` on removal. The `&'a mut P` inside
    /// `handler` keeps the owner alive, so this stays valid for as long as it is registered.
    fd: RawFd,
    handler: Box<dyn HandleTrait<C> + 'a>,
}

pub struct Poller<'a, C> {
    fd: std::os::fd::OwnedFd,
    sources: Slab<Source<'a, C>>,
}

fn epoll_create() -> Result<OwnedFd, std::io::Error> {
    let r = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
    if r == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(r) })
    }
}

impl<'a, C> Poller<'a, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            fd: epoll_create().expect("Failed to open epoll"),
            sources: Slab::new(),
        }
    }

    pub fn register_mut<P>(
        &mut self,
        source: &'a mut P,
        handler: impl FnMut(&mut P, &mut C) -> Result<PollAction, std::io::Error> + 'a,
    ) where
        P: Pollable + 'a,
    {
        let fd = source.pollable_fd().as_raw_fd();
        let slab_pointer:u64 = self.sources.insert(Source {
            fd,
            handler: Box::new(HandleMut {
                r: source,
                func: handler,
            }),
        }).try_into().expect("usize can't convert to u64");

        // EPOLLERR and EPOLLHUP do not need requesting; epoll reports them regardless.
        let mut event = libc::epoll_event{
            events:libc::EPOLLIN as u32,
            u64:slab_pointer
        };

        let res = unsafe{libc::epoll_ctl(self.fd.as_raw_fd(),libc::EPOLL_CTL_ADD,fd,&raw mut event)};
        assert!(res!=-1,"Failed to add fd to epoll\n{:?}",std::io::Error::last_os_error());
    }


    /// Drops a source from the epoll set and the slab. Idempotent for keys already gone.
    ///
    /// # Errors
    /// Returns an error if `epoll_ctl(2)` fails to remove the fd.
    fn deregister(&mut self, key: usize) -> Result<(), std::io::Error> {
        let Some(source) = self.sources.try_remove(key) else {
            return Ok(());
        };
        let res = unsafe {
            libc::epoll_ctl(
                self.fd.as_raw_fd(),
                libc::EPOLL_CTL_DEL,
                source.fd,
                ptr::null_mut(),
            )
        };
        if res == -1 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// # Errors
    /// Returns an error if `poll(2)` fails, if deregistering a source fails, or if a handler
    /// returns an error. A handler error on a source that has hung up is treated as incidental
    /// and only removes that source.
    ///
    /// # Panics
    /// Panics if `epoll_wait(2)` reports a ready fd whose key is not in the slab.
    pub fn poll_once(&mut self, ctx: &mut C) -> Result<PollAction, std::io::Error> {
        const COUNT: u8 = 10;
        // EPOLLERR and EPOLLHUP are reported unconditionally and, because epoll is level
        // triggered, stay set forever once raised. Such a source can never become readable
        // again, so it has to come out of the set or epoll_wait spins returning it. Still run
        // the handler first: EPOLLIN can be set alongside them, and even on its own a hangup
        // leaves already-buffered data readable.
        const HANGUP: u32 = (libc::EPOLLERR | libc::EPOLLHUP) as u32;
        const READABLE: u32 = libc::EPOLLIN as u32;

        // epoll_event is plain old data, so a zeroed array is a valid starting value and
        // avoids having to reason about which entries the kernel actually filled in.
        let mut events = [libc::epoll_event { events: 0, u64: 0 }; COUNT as usize];
        // Any signal not routed through a signalfd — a profiler tick, SIGWINCH, a debugger
        // attaching — interrupts the wait. Nothing is ready yet, so just wait again.
        let ready = loop {
            let ret = unsafe {
                libc::epoll_wait(self.fd.as_raw_fd(), events.as_mut_ptr(), COUNT.into(), -1)
            };
            if ret >= 0 {
                break ret.cast_unsigned() as usize;
            }
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::Interrupted {
                return Err(err);
            }
        };

        for event in &events[..ready] {
            let flags = event.events;
            if flags & (READABLE | HANGUP) == 0 {
                continue;
            }
            let key = usize::try_from(event.u64).expect("slab key does not fit in a usize");
            let hangup = flags & HANGUP != 0;

            let result = self
                .sources
                .get_mut(key)
                .expect("Somehow got wrong slab index")
                .handler
                .call_func(ctx);

            let action = match result {
                Ok(action) => action,
                // A handler that knows nothing about hangups will just fail its read here.
                // The source is being dropped either way, so that incidental error should
                // not take the loop down with it.
                Err(_) if hangup => PollAction::Remove,
                Err(e) => return Err(e),
            };

            match action {
                PollAction::Stop => return Ok(PollAction::Stop),
                PollAction::Remove => self.deregister(key)?,
                // A handler that wants to keep going does not get a say when the fd is dead.
                PollAction::Continue => {
                    if hangup {
                        self.deregister(key)?;
                    }
                }
            }
        }

        // epoll_wait on an empty set would block forever with no way to be woken.
        if self.sources.is_empty() {
            return Ok(PollAction::Stop);
        }
        Ok(PollAction::Continue)
    }

    /// # Errors
    /// Returns an error if `poll(2)` fails or if a registered handler returns an error.
    pub fn run(&mut self, ctx: &mut C) -> Result<(), std::io::Error> {
        loop {
            // poll_once resolves Remove itself, so only these two reach here.
            match self.poll_once(ctx)? {
                PollAction::Stop => return Ok(()),
                PollAction::Continue => {}
                PollAction::Remove => {
                    unreachable!("poll_once never surfaces per-source actions")
                }
            }
        }
    }
}

impl<C> Default for Poller<'_, C> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{PollAction, Pollable, Poller};
    use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

    struct PipeEnd(OwnedFd);

    impl Pollable for PipeEnd {
        fn pollable_fd(&self) -> BorrowedFd<'_> {
            self.0.as_fd()
        }
    }

    impl PipeEnd {
        fn read(&mut self) -> Result<usize, std::io::Error> {
            let mut buf = [0u8; 16];
            let n = unsafe { libc::read(self.0.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if n < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(n.cast_unsigned() as usize)
        }
    }

    fn pipe() -> (PipeEnd, OwnedFd) {
        let mut fds = [0 as RawFd; 2];
        assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
        unsafe { (PipeEnd(OwnedFd::from_raw_fd(fds[0])), OwnedFd::from_raw_fd(fds[1])) }
    }

    /// A handler that knows nothing about hangups and always says Continue must not spin:
    /// the poller drops the dead source itself and, with nothing left, ends the loop.
    #[test]
    fn hangup_removes_source_despite_continue() {
        let (mut source, writer) = pipe();
        drop(writer);

        let mut calls = 0;
        {
            let mut poller = Poller::new();
            poller.register_mut(&mut source, |s, ()| {
                calls += 1;
                // Guard so a regression fails the assert instead of hanging the suite.
                if calls > 5 {
                    return Ok(PollAction::Stop);
                }
                s.read()?;
                Ok(PollAction::Continue)
            });
            poller.run(&mut ()).expect("hangup should unwind cleanly");
        }
        assert_eq!(calls, 1, "hung-up source should be handled once, then dropped");
    }

    /// The same, but the handler fails its read. That error is incidental to the hangup and
    /// must not take down the loop.
    #[test]
    fn hangup_swallows_handler_error() {
        let (mut source, writer) = pipe();
        drop(writer);

        let mut calls = 0;
        {
            let mut poller = Poller::new();
            poller.register_mut(&mut source, |_, ()| {
                calls += 1;
                if calls > 5 {
                    return Ok(PollAction::Stop);
                }
                Err(std::io::Error::other("handler blew up"))
            });
            poller.run(&mut ()).expect("an error on a dead fd should not escape");
        }
        assert_eq!(calls, 1);
    }

    /// With the fd still healthy, a handler error is real and must propagate.
    #[test]
    fn handler_error_propagates_on_live_fd() {
        let (mut source, writer) = pipe();
        assert_eq!(unsafe { libc::write(writer.as_raw_fd(), b"x".as_ptr().cast(), 1) }, 1);

        let mut poller = Poller::new();
        poller.register_mut(&mut source, |_, ()| Err(std::io::Error::other("handler blew up")));
        let err = poller.run(&mut ()).expect_err("live-fd errors must propagate");
        assert_eq!(err.to_string(), "handler blew up");
    }

    /// The point of the context parameter: two sources mutating one value, with no cell and
    /// no second `&mut` borrow.
    #[test]
    fn two_sources_share_one_context() {
        let (mut first, first_writer) = pipe();
        let (mut second, second_writer) = pipe();
        assert_eq!(
            unsafe { libc::write(first_writer.as_raw_fd(), b"a".as_ptr().cast(), 1) },
            1
        );
        assert_eq!(
            unsafe { libc::write(second_writer.as_raw_fd(), b"b".as_ptr().cast(), 1) },
            1
        );

        let mut seen: Vec<&str> = Vec::new();
        let mut poller = Poller::new();
        poller.register_mut(&mut first, |s, seen: &mut Vec<&str>| {
            s.read()?;
            seen.push("first");
            Ok(PollAction::Remove)
        });
        poller.register_mut(&mut second, |s, seen: &mut Vec<&str>| {
            s.read()?;
            seen.push("second");
            Ok(PollAction::Remove)
        });
        poller.run(&mut seen).unwrap();

        seen.sort_unstable();
        assert_eq!(seen, ["first", "second"]);
    }

    /// An explicit Remove drops just that source; the loop ends once the set is empty.
    #[test]
    fn remove_deregisters_only_that_source() {
        let (mut source, writer) = pipe();
        assert_eq!(unsafe { libc::write(writer.as_raw_fd(), b"x".as_ptr().cast(), 1) }, 1);

        let mut calls = 0;
        {
            let mut poller = Poller::new();
            poller.register_mut(&mut source, |s, ()| {
                calls += 1;
                s.read()?;
                Ok(PollAction::Remove)
            });
            poller.run(&mut ()).expect("removing the last source should end the loop");
        }
        assert_eq!(calls, 1);
    }
}
