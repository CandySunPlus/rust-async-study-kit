use std::pin::Pin;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::Duration;

use futures::future::BoxFuture;
use futures::task::ArcWake;
use futures::Future;

struct Inner {
    completed: bool,
    waker: Option<Waker>,
}

pub struct TimerFuture {
    inner: Arc<Mutex<Inner>>,
}

impl TimerFuture {
    pub fn new(dur: Duration) -> Self {
        let inner = Arc::new(Mutex::new(Inner {
            completed: false,
            waker: None,
        }));
        let weak_inner_rc = Arc::downgrade(&inner);
        thread::spawn(move || {
            thread::sleep(dur);
            let inner = weak_inner_rc.upgrade().unwrap();
            let mut inner_guard = inner.lock().unwrap();
            inner_guard.completed = true;
            // After `Future` has been polled once, it is set `waker`
            if let Some(waker) = inner_guard.waker.take() {
                waker.wake()
            }
        });

        Self { inner }
    }
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared_data = self.inner.lock().unwrap();
        if shared_data.completed {
            Poll::Ready(())
        } else {
            shared_data.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

pub struct Task {
    pub future: Mutex<Option<BoxFuture<'static, ()>>>,
    // send task to executor
    pub sync_sender: SyncSender<Arc<Task>>,
}

impl Task {
    pub fn new(
        futrue: Mutex<Option<BoxFuture<'static, ()>>>,
        sync_sender: SyncSender<Arc<Task>>,
    ) -> Self {
        Self {
            future: futrue,
            sync_sender,
        }
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let cloned = arc_self.clone();
        arc_self
            .sync_sender
            .send(cloned)
            .expect("too many tasks queued");
    }
}
