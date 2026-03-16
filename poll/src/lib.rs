use std::os::fd::{AsRawFd, BorrowedFd};

pub trait Pollable {
    fn pollable_fd(&self) -> BorrowedFd<'_>;
}

pub enum PollAction {
    Continue,
    Stop,
}

pub struct Poller<'a> {
    fds: Vec<libc::pollfd>,
    sources: Vec<&'a dyn Pollable>,
    handlers: Vec<Box<dyn FnMut() -> Result<PollAction, std::io::Error> + 'a>>,
}

impl<'a> Poller<'a> {
    pub fn new() -> Self {
        Self {
            fds: Vec::new(),
            sources: Vec::new(),
            handlers: Vec::new(),
        }
    }

    pub fn register<P>(&mut self, source: &'a P, handler: impl FnMut() -> Result<PollAction, std::io::Error> + 'a)
    where
        P: Pollable,
    {
        let fd = source.pollable_fd().as_raw_fd();
        self.fds.push(libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        });
        self.sources.push(source);
        self.handlers.push(Box::new(handler));
    }

    pub fn poll_once(&mut self) -> Result<PollAction, std::io::Error> {
        let ret = unsafe {
            libc::poll(
                self.fds.as_mut_ptr(),
                self.fds.len() as libc::nfds_t,
                -1,
            )
        };
        if ret == -1 {
            return Err(std::io::Error::last_os_error());
        }
        for i in 0..self.fds.len() {
            if self.fds[i].revents & libc::POLLIN != 0 {
                self.fds[i].revents = 0;
                match (self.handlers[i])()? {
                    PollAction::Stop => return Ok(PollAction::Stop),
                    PollAction::Continue => {}
                }
            }
        }
        Ok(PollAction::Continue)
    }

    pub fn run(&mut self) -> Result<(), std::io::Error> {
        loop {
            match self.poll_once()? {
                PollAction::Stop => return Ok(()),
                PollAction::Continue => {}
            }
        }
    }
}

impl Default for Poller<'_> {
    fn default() -> Self {
        Self::new()
    }
}
