//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::marker::PhantomPinned;
use std::task::Context;

// Perhaps EventNode and these imports below should be elsewhere
use crate::task::waker_as_task_ptr;
use crate::task::ITask;
use crate::{EventId, Reactor, Runtime};

/// Data structure required to schedule IO in reactor. In order to schedule any event
/// in reactor the [`EventId`](crate::EventId) is required and the only way to create
/// [`EventId`](crate::EventId) is to create it from this structure.
///
/// `EventNode` implements `!Unpin` which makes the type that contains it to be `!Unpin` by default.
///
/// In fact the `EventId` is actually a pointer to `EventNode` struct and therefore in order
/// to work correctly the location of `EventNode` must be pinned in memory once
/// the `EventId` is obtained for the first time. There is still work in progress how to
/// reflect this fact in API, the unsafe methods (current version) or re-implement methods
/// to require `Pin<&mut EventNode>` as `self`. The later looks like a correct way,
/// but there is a concern of code ergonomic in leaf futures.
#[derive(Debug)]
pub struct EventNode {
    // Intrusive linked list to store Frozen Events
    next: *mut EventNode,
    prev: *mut EventNode,
    // Reference to the task that scheduled this event
    task_ptr: Option<*const dyn ITask>,
    // make any future that has EventNode to be !Unpin
    _pin: PhantomPinned,
}

impl EventNode {
    /// Construction
    pub fn new() -> Self {
        EventNode {
            next: std::ptr::null_mut(),
            prev: std::ptr::null_mut(),
            task_ptr: None,
            _pin: PhantomPinned,
        }
    }

    /// Future invokes this method to get EventId once it pinned (usually on first `poll()`).
    /// Returns the [`EventId`](crate::EventId) that can be used to schedule I/O in reactor.
    pub unsafe fn on_pin(&mut self, ctx: &Context) -> EventId {
        self.task_ptr = Some(waker_as_task_ptr(&ctx.waker()));
        self.get_event_id()
    }

    /// Returns true if runtime woke up the future for this particular event.
    pub fn is_awoken_for<ReactorT: Reactor>(&self, rt: &Runtime<ReactorT>) -> bool {
        rt.is_awoken_for(self.get_event_id())
    }

    /// Returns [`EventId`](crate::EventId) to run event cancellation in reactor. It usually for
    /// the leaf future's `drop()` implementation. When `None` is returned it means that event is no
    /// longer in reactor, so there is nothing to cancel there. This happens when reactor already
    /// emitted the event but it was not delivered to the future because it happened to be
    /// a frozen task.
    pub fn on_cancel(&mut self) -> Option<EventId> {
        if self.is_self_in_list() {
            unsafe { self.remove_self_from_list() }
            None
        } else {
            Some(self.get_event_id())
        }
    }

    /// Returns [`EventId`](crate::EventId).
    pub fn get_event_id(&self) -> EventId {
        EventId(self as *const Self as *const ())
    }

    // If the event is included into the frozen events list
    fn is_self_in_list(&self) -> bool {
        // If event is in list there must be a non-null prev, which is either a prev element
        // in list or head node of the list.
        self.prev != std::ptr::null_mut()
    }

    // Removes EventNode from the list by linking the next/prev pointer to each other. This
    // function expects that EventNode is in list, see is_self_in_list()
    unsafe fn remove_self_from_list(&mut self) {
        debug_assert!(self.is_self_in_list());

        // self.prev is always non-null when node is in the list
        (*(self.prev)).next = self.next;

        // next can be null
        if self.next != std::ptr::null_mut() {
            (*(self.next)).prev = self.prev;
        }
        self.next = std::ptr::null_mut();
        self.prev = std::ptr::null_mut();
    }

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

    // This supposed to be called on head event_node only
    pub(crate) unsafe fn find_unfrozen(&mut self) -> Option<EventId> {
        let mut cur = self;
        while cur.next != std::ptr::null_mut() {
            cur = &mut *(cur.next);
            // Verify if event task is no longer frozen
            if !(*(cur.task_ptr.unwrap())).is_frozen() {
                cur.remove_self_from_list();
                return Some(cur.get_event_id());
            }
        }

        None
    }
}

impl Drop for EventNode {
    fn drop(&mut self) {
        debug_assert!(
            !self.is_self_in_list(),
            "aiur/EventNode: {:?} is dropped while being in frozen list",
            self.get_event_id(),
        );
    }
}
