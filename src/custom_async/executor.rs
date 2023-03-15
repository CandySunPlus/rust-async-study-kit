use std::future::Future;
use std::sync::{Condvar, Mutex};

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

fn block_on<F: Future>(mut future: F) -> F::Output {
    todo!()
}
