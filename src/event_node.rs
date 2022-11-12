//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
//use std::pin::Pin;
use std::marker::PhantomPinned;
use std::task::Context;

// Perhaps EventNode and these imports below should be elsewhere
use crate::task::waker_as_task_ptr;
use crate::task::ITask;
use crate::EventId;

/// X
#[derive(Debug)]
pub struct EventNode {
    next: *mut EventNode,
    prev: *mut EventNode,
    task_ptr: Option<*const dyn ITask>,
    // make any future that has EventNode to be !Unpin
    _pin: PhantomPinned,
}

impl EventNode {
    pub fn new() -> Self {
        EventNode {
            next: std::ptr::null_mut(),
            prev: std::ptr::null_mut(),
            task_ptr: None,
            _pin: PhantomPinned,
        }
    }

    pub fn on_pin(&mut self, ctx: &Context) -> EventId {
        self.task_ptr = Some(waker_as_task_ptr(&ctx.waker()));
        EventId(self as *const Self as *const ())
    }

    pub fn get_event_id(&self) -> EventId {
        EventId(self as *const Self as *const ())
    }

    // pub fn is_awoken_for<>() {}

    pub(crate) fn get_itask_ptr(&self) -> *const dyn ITask {
        self.task_ptr.unwrap()
    }

    pub(crate) unsafe fn push_back(&mut self, event_id: EventId) {
        let mut cur = self;
        while cur.next != std::ptr::null_mut() {
            cur = &mut *(cur.next);
        }

        // event_id is actually a pointer to some EventNode, so we can convert back
        let node = event_id.0 as *mut EventNode;
        // Now we register this node in the list of frozen events
        (*node).prev = cur;
        (*cur).next = node;
    }

    pub(crate) unsafe fn find_unfrozen(&mut self) -> Option<EventId> {
        let mut cur = self;
        while cur.next != std::ptr::null_mut() {
            cur = &mut *(cur.next);
            // Verify if event task is no longer frozen
            if !(*(cur.task_ptr.unwrap())).is_frozen() {
                // Remove the EventNode from the list be updating prev/next entries
                (*(cur.prev)).next = cur.next;
                // Check if this is the last element. It cannot be the first one.
                if cur.next != std::ptr::null_mut() {
                    (*(cur.next)).prev = cur.prev;
                }

                return Some(cur.get_event_id());
            }
        }

        None
    }
}

impl Drop for EventNode {
    fn drop(&mut self) {}
}
