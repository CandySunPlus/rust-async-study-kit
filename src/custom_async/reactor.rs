use std::collections::HashMap;
use std::mem;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::task::Waker;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::future::TaskState;

#[derive(Debug)]
enum Event {
    Close,
    Timeout(u64, usize),
}

pub(crate) struct Reactor {
    dispatcher: Sender<Event>,
    handle: Option<JoinHandle<()>>,
    pub tasks: HashMap<usize, TaskState>,
}

impl Reactor {
    pub(crate) fn new() -> Arc<Mutex<Box<Self>>> {
        let (tx, rx) = channel::<Event>();
        let reactor = Arc::new(Mutex::new(Box::new(Reactor {
            dispatcher: tx,
            handle: None,
            tasks: Default::default(),
        })));

        let reactor_weak_rc = Arc::downgrade(&reactor);

        let handle = thread::spawn(move || {
            let mut handles = vec![];

            for event in rx {
                let reactor = reactor_weak_rc.clone();
                match event {
                    Event::Close => break,
                    Event::Timeout(duration, id) => {
                        // create new handle for new task,
                        // 1. sleep the thread, the duration is send by the event argument.
                        // 2. wake task via task's waker.
                        let event_handle = thread::spawn(move || {
                            thread::sleep(Duration::from_secs(duration));
                            let reactor = reactor.upgrade().unwrap();
                            reactor.lock().map(|mut r| r.wake(id)).unwrap();
                        });
                        handles.push(event_handle);
                    }
                }
            }

            // These event handle threads does not live longer then the reactor thread, so we need
            // to wait for all `event_handle` to finish executing, when the `Event::Close` event is
            // received or the `Reactor` gets dropped.
            handles.into_iter().for_each(|h| h.join().unwrap());
        });

        reactor.lock().map(|mut r| r.handle = Some(handle)).unwrap();

        reactor
    }

    /// wake task via waker for the task with corresponding id.
    pub(crate) fn wake(&mut self, id: usize) {
        let state = self.tasks.get_mut(&id).unwrap();

        match mem::replace(state, TaskState::Ready) {
            TaskState::NotReady(waker) => waker.wake(),
            TaskState::Finished => panic!("Called 'wake' twice on task: {id}"),
            _ => unreachable!(),
        }
    }

    /// register a new task with the reactor
    pub(crate) fn register(&mut self, duration: u64, waker: Waker, id: usize) {
        if self.tasks.insert(id, TaskState::NotReady(waker)).is_some() {
            panic!("Tired to insert a task with id: '{id}' twice!");
        }
        self.dispatcher.send(Event::Timeout(duration, id)).unwrap();
    }

    pub(crate) fn close(&mut self) {
        self.dispatcher.send(Event::Close).unwrap();
    }

    pub(crate) fn is_ready(&self, id: usize) -> bool {
        self.tasks
            .get(&id)
            .map(|t| matches!(t, TaskState::Ready))
            .unwrap_or(false)
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        self.handle.take().map(|h| h.join().unwrap()).unwrap();
    }
}
