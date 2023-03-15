use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::{mem, pin, task};

use crate::executor::Parker;
use crate::reactor::Reactor;

#[derive(Clone)]
struct MyWaker {
    parker: Arc<Parker>,
}

#[derive(Clone)]
struct Task {
    id: usize,
    reactor: Arc<Mutex<Box<Reactor>>>,
    data: u64,
}

#[derive(Debug)]
pub(crate) enum TaskState {
    Ready,
    NotReady(Waker),
    Finished,
}

fn mywaker_wake(s: &MyWaker) {
    let wake_arc = unsafe { Arc::from_raw(s) };
    wake_arc.parker.unpark();
}

const VTABLE: RawWakerVTable = unsafe {
    RawWakerVTable::new(
        |s| mywaker_clone(&*(s as *const _)),
        |s| mywaker_wake(&*(s as *const _)),
        |s| mywaker_wake(*(s as *const _)),
        |s| drop(Arc::from_raw(s)),
    )
};

fn mywaker_clone(s: &MyWaker) -> RawWaker {
    let arc = unsafe { Arc::from_raw(s) };
    // increase ref count, and don't drop when out scope
    mem::forget(arc.clone());
    RawWaker::new(Arc::into_raw(arc) as *const _, &VTABLE)
}

fn mywaker_into_waker(s: *const MyWaker) -> Waker {
    let raw_waker = RawWaker::new(s as *const _, &VTABLE);
    unsafe { Waker::from_raw(raw_waker) }
}

impl Task {
    fn new(reactor: Arc<Mutex<Box<Reactor>>>, data: u64, id: usize) -> Self {
        Task { id, reactor, data }
    }
}

impl Future for Task {
    type Output = usize;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut r = self.reactor.lock().unwrap();

        if r.is_ready(self.id) {
            *r.tasks.get_mut(&self.id).unwrap() = TaskState::Finished;
            Poll::Ready(self.id)
        } else if r.tasks.contains_key(&self.id) {
            // The future has already been polled, so we need to insert a new `TaskState` and
            // release the old one to ensure that it wakes up with the latest waker in the next
            // poll.
            r.tasks
                .insert(self.id, TaskState::NotReady(cx.waker().clone()));
            Poll::Pending
        } else {
            r.register(self.data, cx.waker().clone(), self.id);
            Poll::Pending
        }
    }
}
