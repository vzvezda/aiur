//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::time::Duration;

use crate::event_node::EventNode;

/// Reactor API for Executor.
///
/// External crate implements the trait to create a Runtime with both executor and reactor.
pub trait Reactor {
    /// The only method Runtime needs from the reactor is to wait for I/O to complete.
    fn wait(&self) -> EventId;
}

/// Reactor with a very basic timers.
pub trait TemporalReactor: Reactor {
    /// Timer API has a limit about its max duration (24 hour).
    const MAX_TIMER_DURATION_MS: u32 = 24 * 60 * 60 * 1000;

    fn schedule_timer(&self, event_id: EventId, duration: Duration);
    fn cancel_timer(&self, event_id: EventId);
}

/// EventId is a reactor's id for the event that was awoken.
///
/// The same waker can be scheduled to awake on many i/o events (for example with join!) and 
/// this id is used by the leaf futures to verify if executor has invoked the poll() method 
/// because this future was awoken.
///
/// EventId is created from EventNode.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct EventId(pub(crate) *const ());

impl EventId {
    pub fn null() -> Self {
        EventId(std::ptr::null())
    }

    pub fn as_event_node(&self) -> &EventNode {
        assert!(self.0 != std::ptr::null());
        unsafe { &*(self.0 as *const EventNode) }
    }
}
