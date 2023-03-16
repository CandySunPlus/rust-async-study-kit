use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll};

use futures::task::waker_ref;

use crate::future::MyWaker;

#[derive(Default)]
pub(crate) struct Parker(Mutex<bool>, Condvar);

impl Parker {
    /// park the thread
    pub(crate) fn park(&self) {
        let mut resumable = self.0.lock().unwrap();

        while !*resumable {
            // blocks the thread until the unpark method is called
            resumable = self.1.wait(resumable).unwrap();
        }

        *resumable = false;
    }

    /// unpark the thread
    pub(crate) fn unpark(&self) {
        *self.0.lock().unwrap() = true;
        self.1.notify_one();
    }
}

pub fn block_on<F: Future>(mut future: F) -> F::Output {
    let parker = Arc::new(Parker::default());
    let mywaker = Arc::new(MyWaker {
        parker: parker.clone(),
    });
    let waker = waker_ref(&mywaker);
    let mut cx = Context::from_waker(&waker);

    // SAFETY: we shadow `future` so it can't be assessed again.
    let mut future = unsafe { Pin::new_unchecked(&mut future) };

    loop {
        // we can use `as_mut()` for multiple call poll function, which consume the pinned type
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(val) => break val,
            Poll::Pending => parker.park(),
        }
    }
}
