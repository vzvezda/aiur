//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use crate::runtime::Awoken;

// Untyped version of std::task::Poll
#[derive(PartialEq)]
pub(crate) enum Completion {
    Working,
    Done,
}

// This is for the access the future using a vtable
pub(crate) trait ITask {
    fn poll(&mut self) -> Completion;
    fn on_pinned(&mut self);
    // for scope.spawn()
    fn is_completed(&self) -> bool;
    fn on_completed(&mut self);
}

// This is the struct to where the data of RawWaker is pointed to.
struct TaskHeader {
    awoken: *const Awoken,
    itask_ptr: Option<*mut dyn ITask>, // TODO: into the Pin
}

// The storage for task which is the block of data that contains everything: the Waker and the
// Future.
struct TaskImpl<FutureT, ResT>
where
    FutureT: Future<Output = ResT>,
{
    header: TaskHeader,
    result: *mut ResT,
    completed: bool,
    future: FutureT,
}

impl<FutureT, ResT> TaskImpl<FutureT, ResT>
where
    FutureT: Future<Output = ResT>,
{
    // TODO: into the pin

    // Make std::task::Waker from TaskImpl, which is basically a pointer to header.
    fn to_waker(&mut self) -> std::task::Waker {
        // Cloning the waker returns just the copy of the pointer.
        unsafe fn clone_impl(raw_waker_ptr: *const ()) -> RawWaker {
            // Just copy the pointer, which is works as a clone
            RawWaker::new(raw_waker_ptr, &AIUR_WAKER_VTABLE)
        }

        // Wake the future
        unsafe fn wake_impl(raw_waker_ptr: *const ()) {
            wake_by_ref_impl(raw_waker_ptr);
        }

        // Wake the future by ref
        unsafe fn wake_by_ref_impl(raw_waker_ptr: *const ()) {
            let header = &*(raw_waker_ptr as *const TaskHeader);
            let awoken: &Awoken = &*header.awoken;
            // Runtime receives the pointer to future that was waken
            awoken.set_awoken_task(header.itask_ptr.unwrap());
        }

        // Drop the waker
        unsafe fn drop_impl(_raw_waker_ptr: *const ()) {}

        const AIUR_WAKER_VTABLE: RawWakerVTable =
            RawWakerVTable::new(clone_impl, wake_impl, wake_by_ref_impl, drop_impl);

        let raw_waker = RawWaker::new(
            &self.header as *const TaskHeader as *const (),
            &AIUR_WAKER_VTABLE,
        );

        unsafe { Waker::from_raw(raw_waker) }
    }
}

impl<FutureT, ResT> ITask for TaskImpl<FutureT, ResT>
where
    FutureT: Future<Output = ResT>,
{
    fn poll(&mut self) -> Completion {
        let waker = self.to_waker();
        let mut ctx = Context::from_waker(&waker);
        let future = unsafe { Pin::new_unchecked(&mut self.future) };

        match future.poll(&mut ctx) {
            Poll::Pending => Completion::Working,
            Poll::Ready(res) => {
                unsafe { *self.result = res };
                Completion::Done
            }
        }
    }

    fn on_pinned(&mut self) {
        // ok, we have this unsafe
        let static_bounded_itask_trait_ptr = unsafe {
            std::mem::transmute::<*mut (dyn ITask + '_), *mut (dyn ITask + 'static)>(&mut *self)
        };

        self.header.itask_ptr = Some(static_bounded_itask_trait_ptr);
    }

    fn is_completed(&self) -> bool {
        self.completed 
    }

    fn on_completed(&mut self) {
        self.completed = true;
    }
}

// Constructs the task
pub(crate) fn construct_task<'runtime, FutureT, ResT>(
    awoken: &'runtime Awoken,
    future: FutureT,
    res_ptr: *mut ResT,
) -> impl ITask + 'runtime
where
    FutureT: Future<Output = ResT> + 'runtime,
    ResT: 'runtime,
{
    TaskImpl {
        header: TaskHeader {
            awoken: &*awoken,
            itask_ptr: None, // address is not ready yet
        },

        completed: false,
        result: res_ptr,
        future: future,
    }
}

pub(crate) fn allocate_void_task<'runtime, 'future, FutureT>(
    awoken: &'runtime Awoken,
    future: FutureT,
) -> *mut (dyn ITask + 'future)
where
    FutureT: Future<Output = ()> + 'future,
    'runtime: 'future,
{
    let boxed: Box<dyn ITask> = Box::new(TaskImpl {
        header: TaskHeader {
            awoken: &*awoken,
            itask_ptr: None, // address is not ready yet
        },
        completed: false,
        result: std::ptr::null_mut(),
        future: future,
    });

    Box::into_raw(boxed)
}
