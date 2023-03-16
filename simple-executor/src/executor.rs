use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::task::Context;

use futures::task::waker_ref;
use futures::{Future, FutureExt};

use crate::timer_future::Task;

pub struct Spawner {
    sync_sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        let future = future.boxed();
        let task = Arc::new(Task::new(
            Mutex::new(Some(future)),
            self.sync_sender.clone(),
        ));
        self.sync_sender.send(task).expect("too many tasks queued");
    }
}

pub struct Executor {
    receiver: Receiver<Arc<Task>>,
}

impl Executor {
    pub fn run(&self) {
        while let Ok(task) = self.receiver.recv() {
            let mut future_guard = task.future.lock().unwrap();
            if let Some(mut future) = future_guard.take() {
                let waker = waker_ref(&task);
                let cx = &mut Context::from_waker(&waker);
                if future.as_mut().poll(cx).is_pending() {
                    *future_guard = Some(future);
                }
            }
        }
    }
}

pub fn new_exector_and_spawner() -> (Executor, Spawner) {
    const MAX_QUEUED_TASKS: usize = 10_000;
    let (sync_sender, receiver) = sync_channel(MAX_QUEUED_TASKS);
    (Executor { receiver }, Spawner { sync_sender })
}
