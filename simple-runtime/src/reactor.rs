use std::cell::RefCell;
use std::collections::HashMap;
use std::os::fd::RawFd;
use std::rc::Rc;
use std::task::{Context, Waker};

use nix::fcntl::FcntlArg::{F_GETFL, F_SETFL};
use nix::fcntl::{fcntl, OFlag};
use polling::{Event, Poller};

use crate::executor::EX;

#[inline]
pub(crate) fn get_reactor() -> Rc<RefCell<Reactor>> {
    EX.with(|ex| ex.reactor.clone())
}

pub struct Reactor {
    poller: Poller,
    waker_map: HashMap<u64, Waker>,
    buffer: Vec<Event>,
}

impl Reactor {
    fn new() -> Self {
        Self {
            poller: Poller::new().unwrap(),
            waker_map: Default::default(),
            buffer: Vec::with_capacity(2048),
        }
    }

    /// add fd to epoll, and interest none event.
    ///
    /// the `polling` crate use oneshot mode by default, so we need re-register each event every
    /// time after it is triggered.
    pub fn add(&mut self, fd: RawFd) {
        let mut flags = OFlag::from_bits(fcntl(fd, F_GETFL).unwrap()).unwrap();
        flags |= OFlag::O_NONBLOCK;
        // set fd as nonblock
        fcntl(fd, F_SETFL(flags)).unwrap();
        // add fd to epoll, and interest none event.
        self.poller.add(fd, Event::none(fd as _)).unwrap();
    }

    /// delete waker from `waker_map` with fd
    ///
    /// We use a clever way of using `fd * 2` as the token for readability and `fd * 2 + 1` as the
    /// token for writability.
    pub fn delete(&mut self, fd: RawFd) {
        // use fd * 2 as readable token, fd * 2 + 1 as writable token
        self.waker_map.remove(&(fd as u64 * 2));
        self.waker_map.remove(&(fd as u64 * 2 + 1));
    }

    /// Wait for an event to occur on a file descriptor.
    ///
    /// This function will block the current thread until an event occurs on one of the file descriptors
    /// registered with the `Poller`.
    ///
    /// We use a clever way of using `fd * 2` as the token for readability and `fd * 2 + 1` as the
    /// token for writability.
    pub fn wait(&mut self) {
        self.poller.wait(&mut self.buffer, None).unwrap();

        for _ in 0..self.buffer.len() {
            let event = self.buffer.swap_remove(0);

            if event.readable {
                if let Some(waker) = self.waker_map.remove(&(event.key as u64 * 2)) {
                    waker.wake();
                }
            }

            if event.writable {
                if let Some(waker) = self.waker_map.remove(&(event.key as u64 * 2 + 1)) {
                    waker.wake();
                }
            }
        }
    }

    /// interest readable event for fd, and save waker to waker_map
    ///
    /// We use a clever way of using `fd * 2` as the token for readability and `fd * 2 + 1` as the
    /// token for writability.
    pub fn interest_readable(&mut self, fd: RawFd, cx: &mut Context) {
        self.poller.modify(fd, Event::readable(fd as _)).unwrap();
        self.waker_map.insert(fd as u64 * 2, cx.waker().clone());
    }

    /// interest writable event for fd, and save waker to waker_map
    ///
    /// We use a clever way of using `fd * 2` as the token for readability and `fd * 2 + 1` as the
    /// token for writability.
    pub fn interest_writable(&mut self, fd: RawFd, cx: &mut Context) {
        self.poller.modify(fd, Event::writable(fd as _)).unwrap();
        self.waker_map.insert(fd as u64 * 2 + 1, cx.waker().clone());
    }
}

impl Default for Reactor {
    fn default() -> Self {
        Self::new()
    }
}
