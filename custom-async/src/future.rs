use std::collections::hash_map::Entry;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use futures::task::ArcWake;

use crate::executor::Parker;
use crate::reactor::Reactor;

#[derive(Clone)]
pub(crate) struct MyWaker {
    pub parker: Arc<Parker>,
}

#[derive(Clone)]
pub struct Task {
    id: usize,
    reactor: Arc<Mutex<Box<Reactor>>>,
    data: u64,
}

#[derive(Debug)]
pub enum TaskState {
    Ready,
    NotReady(Waker),
    Finished,
}

impl Task {
    pub fn new(reactor: Arc<Mutex<Box<Reactor>>>, data: u64, id: usize) -> Self {
        Task { id, reactor, data }
    }
}

impl ArcWake for MyWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.parker.unpark();
    }
}

impl Future for Task {
    type Output = usize;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut r = self.reactor.lock().unwrap();

        if r.is_ready(self.id) {
            *r.tasks.get_mut(&self.id).unwrap() = TaskState::Finished;
            Poll::Ready(self.id)
        } else if let Entry::Occupied(mut e) = r.tasks.entry(self.id) {
            // The future has already been polled, so we need to insert a new `TaskState` and
            // release the old one to ensure that it wakes up with the latest waker in the next
            // poll.
            e.insert(TaskState::NotReady(cx.waker().clone()));
            Poll::Pending
        } else {
            r.register(self.data, cx.waker().clone(), self.id);
            Poll::Pending
        }
    }
}
