//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use crate::runtime::Awoken;

pub(crate) enum Task2Poll {
    Ready,
    Pending,
    Frozen,
}

// This is for the access the future using a vtable
pub(crate) trait ITask {
    // todo: &mut self -> &self
    fn poll(&mut self) -> Task2Poll;
}

pub(crate) struct Task<FutT, ResT>
where
    FutT: Future<Output = ResT>,
{
    waker_data: WakerData,
    future: RefCell<FutT>,
    result: Option<ResT>,
}

struct WakerData {
    awoken: *const Awoken,
    itask_ptr: Option<*mut dyn ITask>,
}

impl<FutT, ResT> Task<FutT, ResT>
where
    FutT: Future<Output = ResT>,
{
    pub fn new(future: FutT, awoken: *const Awoken) -> Self {
        Self {
            future: RefCell::new(future),
            result: None,
            waker_data: WakerData {
                awoken,
                itask_ptr: None,
            },
        }
    }

    // Returns true if the task has been completed. When the task is completed its result 
    // can be extracted by [take_result()].
    pub fn is_completed(self: &Pin<&mut Self>) -> bool {
        // Having something self.result means that future has been completed
        self.as_ref().get_ref().result.is_some()
    }

    pub fn start(self: &mut Pin<&mut Self>) {
        self.save_itask_ptr();
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        this.poll();
    }

    fn save_itask_ptr(self: &mut Pin<&mut Self>) {
        let this = unsafe { self.as_mut().get_unchecked_mut() }; // projection

        // We need to erase Future lifetime bound. This is up to the nested_loop() to ensure
        // that references do not outlive the objects.
        let static_bounded_itask_trait_ptr = unsafe {
            std::mem::transmute::<*mut (dyn ITask + '_), *mut (dyn ITask + 'static)>(&mut *this)
        };

        this.waker_data.itask_ptr = Some(static_bounded_itask_trait_ptr);
    }

    // Returns the result of completed future. Panics if future is not completed or if result
    // has been already taken.
    pub fn take_result(self: &mut Pin<&mut Self>) -> ResT {
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        this.result.take().unwrap()
    }

    // Make std::task::Waker from Task, which is basically a pointer to header.
    fn as_waker(&self) -> std::task::Waker {
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
            let waker_data = &*(raw_waker_ptr as *const WakerData);
            let awoken: &Awoken = &*waker_data.awoken;
            // Runtime receives the pointer to future that was waken
            awoken.set_awoken_task(waker_data.itask_ptr.unwrap());
        }

        // Drop the waker
        unsafe fn drop_impl(_raw_waker_ptr: *const ()) {}

        const AIUR_WAKER_VTABLE: RawWakerVTable =
            RawWakerVTable::new(clone_impl, wake_impl, wake_by_ref_impl, drop_impl);

        let raw_waker = RawWaker::new(
            &self.waker_data as *const WakerData as *const (),
            &AIUR_WAKER_VTABLE,
        );

        unsafe { Waker::from_raw(raw_waker) }
    }
}

impl<FutT, ResT> ITask for Task<FutT, ResT>
where
    FutT: Future<Output = ResT>,
{
    fn poll(&mut self) -> Task2Poll {
        if self.result.is_some() {
            // Future has been completed.
            return Task2Poll::Ready;
        }

        match self.future.try_borrow_mut() {
            Err(_) => Task2Poll::Frozen,
            Ok(mut future) => {
                // we have borrowed the future, so it is now "frozen" by this scope. Once we
                // reenter this function (e.g. by nested_loop()), it would return Frozen.
                let waker = self.as_waker();
                let mut ctx = Context::from_waker(&waker);
                let future = unsafe { Pin::new_unchecked(&mut *future) };
                match future.poll(&mut ctx) {
                    Poll::Ready(res) => {
                        self.result = Some(res);
                        Task2Poll::Ready
                    }
                    Poll::Pending => Task2Poll::Pending,
                }
            }
        }
    }
}
