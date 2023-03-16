use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Context;
use std::thread;

use crossbeam::channel;
use futures::channel::oneshot;
use futures::future::BoxFuture;
use futures::task::{waker_ref, ArcWake};
use futures::Future;
use once_cell::sync::Lazy;

static QUEUE: Lazy<channel::Sender<Arc<Task>>> = Lazy::new(|| {
    let (sender, receiver) = channel::unbounded::<Arc<Task>>();
    for _ in 0..num_cpus::get().max(1) {
        let receiver = receiver.clone();
        thread::spawn(move || receiver.iter().for_each(|task| task.run()));
    }
    sender
});

type JoinHandler<R> = BoxFuture<'static, R>;

const WOKEN: usize = 0b01;
const RUNNING: usize = 0b10;

struct Task {
    // each `Waker` hold a reference to the corresponding task, so the task will shared in
    // different threads, and the `poll` method need a mutable future, so we need use `Mutex` to
    // provide this.
    future: Mutex<BoxFuture<'static, ()>>,
    state: AtomicUsize,
}

impl Task {
    fn run(self: Arc<Task>) {
        let waker = waker_ref(&self);
        self.state.store(RUNNING, Ordering::SeqCst);
        let cx = &mut Context::from_waker(&waker);
        let poll = self.future.try_lock().unwrap().as_mut().poll(cx);
        if poll.is_pending() {
            if self.state.fetch_and(!RUNNING, Ordering::SeqCst) == WOKEN | RUNNING {
                QUEUE.send(self).unwrap();
            }
        }
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        if arc_self.state.fetch_or(WOKEN, Ordering::SeqCst) == 0 {
            QUEUE.send(arc_self.clone()).unwrap();
        }
    }
}

pub fn spawn<F, R>(future: F) -> JoinHandler<R>
where
    F: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let (s, r) = oneshot::channel();

    // use oneshot channel to get future result to `JoinHandler`
    let future = async move {
        let _ = s.send(future.await);
    };

    let task = Arc::new(Task {
        future: Mutex::new(Box::pin(future)),
        state: AtomicUsize::default(),
    });

    QUEUE.send(task).unwrap();

    Box::pin(async { r.await.unwrap() })
}
