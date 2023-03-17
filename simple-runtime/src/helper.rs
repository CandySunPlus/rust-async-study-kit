use std::mem;
use std::rc::Rc;
use std::task::{RawWaker, RawWakerVTable};

use crate::executor::Task;

pub struct Helper;

impl Helper {
    pub const VTABLE: RawWakerVTable = RawWakerVTable::new(
        Self::clone_waker,
        Self::wake,
        Self::wake_by_ref,
        Self::drop_waker,
    );

    unsafe fn clone_waker(data: *const ()) -> RawWaker {
        increase_refcount(data);
        let vtable = &Self::VTABLE;
        RawWaker::new(data, vtable)
    }

    unsafe fn wake(ptr: *const ()) {
        let rc = Rc::from_raw(ptr as *const Task);
        rc.wake_();
    }

    unsafe fn wake_by_ref(ptr: *const ()) {
        let rc = mem::ManuallyDrop::new(Rc::from_raw(ptr as *const Task));
        rc.wake_by_ref_()
    }

    unsafe fn drop_waker(ptr: *const ()) {
        drop(Rc::from_raw(ptr as *const Task));
    }
}

/// Increases the reference count of the given data.
///
/// # Safety
///
/// This function is unsafe because it takes a raw pointer.
///
/// # Parameters
///
/// * `data` - A raw pointer to the data to increase the reference count of.
///
/// # Panics
///
/// This function will panic if the given pointer is null.
unsafe fn increase_refcount(data: *const ()) {
    let rc = mem::ManuallyDrop::new(Rc::from_raw(data as *const Task));
    let _: mem::ManuallyDrop<_> = rc.clone();
}
