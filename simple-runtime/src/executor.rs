use std::cell::RefCell;
use std::collections::VecDeque;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, RawWaker, Waker};

use futures::future::LocalBoxFuture;
use futures::{Future, FutureExt};
use scoped_tls::scoped_thread_local;
use waker_fn::waker_fn;

use crate::helper::Helper;
use crate::reactor::Reactor;

scoped_thread_local!(pub(crate) static EX: Executor);

pub struct Task {
    future: RefCell<LocalBoxFuture<'static, ()>>,
}

#[derive(Default)]
pub struct TaskQueue {
    queue: RefCell<VecDeque<Rc<Task>>>,
}

impl TaskQueue {
    /// Pushes a task onto the queue.
    ///
    /// # Parameters
    ///
    /// * `runnable` - The task to be pushed onto the queue.
    ///
    /// # Returns
    ///
    /// Nothing.
    pub(crate) fn push(&self, runnable: Rc<Task>) {
        self.queue.borrow_mut().push_back(runnable);
    }

    /// Pops a task off the queue.
    ///
    /// # Parameters
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// * `Some(Rc<Task>)` - The task that was popped off the queue.
    /// * `None` - If the queue is empty.
    pub(crate) fn pop(&self) -> Option<Rc<Task>> {
        self.queue.borrow_mut().pop_front()
    }
}

impl Task {
    /// Wakes up the task by calling `wake_by_ref_` on the `Rc<Self>`
    pub fn wake_(self: Rc<Self>) {
        Self::wake_by_ref_(&self);
    }

    /// Wakes up the task by calling `wake_by_ref_` on the `Rc<Self>`
    pub fn wake_by_ref_(self: &Rc<Self>) {
        EX.with(|ex| ex.local_queue.push(self.clone()))
    }
}

fn waker(wake: Rc<Task>) -> Waker {
    let ptr = Rc::into_raw(wake) as *const ();
    let vtable = &Helper::VTABLE;
    unsafe { Waker::from_raw(RawWaker::new(ptr, vtable)) }
}

pub struct Executor {
    local_queue: TaskQueue,
    pub(crate) reactor: Rc<RefCell<Reactor>>,
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    pub fn new() -> Self {
        Self {
            local_queue: Default::default(),
            reactor: Rc::new(RefCell::new(Reactor::default())),
        }
    }

    pub fn spawn<F>(fut: F)
    where
        F: Future<Output = ()> + 'static,
    {
        let t = Rc::new(Task {
            future: RefCell::new(fut.boxed_local()),
        });

        EX.with(|ex| ex.local_queue.push(t));
    }

    pub fn block_on<F, T, O>(&self, f: F) -> O
    where
        F: Fn() -> T,
        T: Future<Output = O> + 'static,
    {
        let _waker = waker_fn(|| {});
        let cx = &mut Context::from_waker(&_waker);

        EX.set(self, || {
            let fut = &mut f();
            let mut fut = unsafe { Pin::new_unchecked(fut) };
            loop {
                // return if the outer future is ready
                if let Poll::Ready(t) = fut.as_mut().poll(cx) {
                    break t;
                }

                // consum all tasks
                while let Some(t) = self.local_queue.pop() {
                    let future = t.future.borrow_mut();
                    let w = waker(t.clone());
                    let ctx = &mut Context::from_waker(&w);
                    let _ = Pin::new(future).as_mut().poll(ctx);
                }

                // no task to execute now, outer future may ready
                if let Poll::Ready(t) = fut.as_mut().poll(cx) {
                    break t;
                }

                // block for IO
                self.reactor.borrow_mut().wait();
            }
        })
    }
}
