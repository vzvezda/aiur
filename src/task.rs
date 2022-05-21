//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use crate::runtime::Awoken;

// Unlike std::task::Poll there is also have a Frozen state meaning that task cannot
// be polled right now because it mutually borrowed somewhere. Also the Ready result
// does not have value, it should be extracted by take_result() in a right level of
// nested_loop().
pub(crate) enum PollResult {
    Ready,
    Pending,
    Frozen,
}

// This is for the access the future using a vtable
pub(crate) trait ITask {
    fn poll(&self) -> PollResult;
    fn unfrozen_ancestor(&self) -> *const dyn ITask;
    fn is_frozen(&self) -> bool;

    // navigation
    fn get_parent(&self) -> Option<*const dyn ITask>;
}

pub(crate) struct Task<FutT>
where
    FutT: Future,
{
    waker_data: WakerData,
    future: RefCell<FutT>, // being borrowed means the task is frozen
    result: RefCell<Option<FutT::Output>>, // can have UnsafeCell here
    parent: Cell<Option<*const dyn ITask>>, //
    _pinned: std::marker::PhantomPinned, // because of self-referential
}

struct WakerData {
    awoken: *const Awoken,
    itask_ptr: Cell<Option<*const dyn ITask>>,
}

impl<FutT> Task<FutT>
where
    FutT: Future,
{
    pub fn new(future: FutT, awoken: *const Awoken) -> Self {
        Self {
            waker_data: WakerData {
                awoken,
                itask_ptr: Cell::new(None),
            },
            future: RefCell::new(future),
            result: RefCell::new(None),
            parent: Cell::new(None),
            _pinned: std::marker::PhantomPinned,
        }
    }

    // Returns true if the task has been completed. When the task is completed its result
    // can be extracted by [take_result()].
    pub fn is_completed(&self) -> bool {
        // Having something self.result means that future has been completed
        self.result.borrow().is_some()
    }

    // Setup some self-references which used to get pointer to future from the Waker
    pub fn on_pinned(self: &mut Pin<&mut Self>) {
        let this = unsafe { self.as_mut().get_unchecked_mut() }; // projection

        // We need to erase Future lifetime bound. This is up to the nested_loop() to ensure
        // that references in Future do not outlive the objects.
        let static_bounded_itask_trait_ptr = unsafe {
            std::mem::transmute::<*const (dyn ITask + '_), *const (dyn ITask + 'static)>(&*this)
        };

        this.waker_data
            .itask_ptr
            .set(Some(static_bounded_itask_trait_ptr));
    }

    // Returns the result of completed future. Panics if future is not completed or if result
    // has been already taken.
    pub fn take_result(&self) -> FutT::Output {
        self.result.borrow_mut().take().unwrap()
    }

    // When task-based any_of or join makes a poll of inner task, it provides its context and
    // the child future can store parent task.

    // Extracts the ITask from context and assign this as parent
    pub fn assign_parent(&self, ctx: &Context<'_>) -> bool {
        if self.parent.get().is_none() {
            self.parent.set(Some(self.get_task_from_context(ctx)));

            // We need to erase Future lifetime bound. This is up to the nested_loop() to ensure
            // that references in Future do not outlive the objects.
            let static_bounded_itask_trait_ptr = unsafe {
                std::mem::transmute::<*const (dyn ITask + '_), *const (dyn ITask + 'static)>(&*self)
            };

            self.waker_data
                .itask_ptr
                .set(Some(static_bounded_itask_trait_ptr));

            true // parent has been assigned
        } else {
            false // parent already assigned
        }
    }

    fn get_task_from_context(&self, ctx: &Context<'_>) -> *const dyn ITask {
        // Invoking waker.wake() makes saves ITask into Awoken
        ctx.waker().wake_by_ref();
        unsafe { (*(self.waker_data.awoken)).get_itask_ptr().unwrap() }
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
            awoken.set_awoken_task(waker_data.itask_ptr.get().unwrap());
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

impl<FutT> ITask for Task<FutT>
where
    FutT: Future,
{
    fn poll(&self) -> PollResult {
        if self.result.borrow().is_some() {
            // Future has been completed.
            return PollResult::Ready;
        }

        // Borrowed the future in RefCell, so it is now "frozen" by this scope. Once we
        // reenter this function (e.g. by nested_loop()), it would return Frozen.
        match self.future.try_borrow_mut() {
            Err(_) => PollResult::Frozen,
            Ok(mut future) => {
                let waker = self.as_waker();
                let mut ctx = Context::from_waker(&waker);
                let future = unsafe { Pin::new_unchecked(&mut *future) };
                match future.poll(&mut ctx) {
                    Poll::Ready(res) => {
                        *self.result.borrow_mut() = Some(res);
                        PollResult::Ready
                    }
                    Poll::Pending => PollResult::Pending,
                }
            }
        }
    }

    fn unfrozen_ancestor(&self) -> *const dyn ITask {
        debug_assert!(
            !self.is_frozen(),
            "unfrozen_ancestor(): don't use it on frozen tasks"
        );

        // "does not live long enough" when self as *const dyn ITask
        let mut cur = unsafe {
            std::mem::transmute::<*const (dyn ITask + '_), *const (dyn ITask + 'static)>(&*self)
        };

        // Going up in hierarchy to find either frozen parent or root task. Implemented
        // as iteration instead of recursion because of performance.
        //
        // TODO: avoid virtual calls: get_parent()/is_frozen()
        unsafe {
            loop {
                match (*cur).get_parent() {
                    None => return cur,
                    Some(next) => {
                        if (*next).is_frozen() {
                            return cur;
                        } else {
                            cur = next;
                        }
                    }
                }
            }
        }
    }

    fn is_frozen(&self) -> bool {
        match self.future.try_borrow_mut() {
            Err(_) => true,
            Ok(_) => false,
        }
    }

    fn get_parent(&self) -> Option<*const dyn ITask> {
        self.parent.get()
    }
}
