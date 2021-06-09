//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::pin::Pin;
use std::task::Waker;
use std::time::Duration;

/// Reactor API for Executor
pub trait Reactor {
    fn wait(&self) -> EventId;
}

/// Reactor with a very basic timers.
pub trait TemporalReactor : Reactor {
    /// Timer API has a limit about its max duration (24 hour).
    const MAX_TIMER_DURATION_MS: u32 = 24 * 60 * 60 * 1000;

    fn schedule_timer(&self, waker: Waker, event_id: EventId, duration: Duration);
    fn cancel_timer(&self, event_id: EventId);
}

/// EventId is a reactor's id for the event that was awoken.
///
/// The same waker can be scheduled to awake on many i/o events (typically with select!/join! macro
/// or by other means) and this id is kind of cheap method for leaf future to verify if execultor
/// has invoked the poll() method because this future was awoken.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct EventId(pub(crate) *const ());

impl EventId {
    pub fn null() -> Self {
        EventId(std::ptr::null())
    }
}

// Mixin for use in leaf futures to make the EventId out of the address of pinned
// future, which should be sufficient unique id without much extra cost.
pub trait GetEventId {
    // EventId is safe to obtain from a pinned object, because it is guarantied that
    fn get_event_id(self: &Pin<&mut Self>) -> EventId {
        let this = self.as_ref().get_ref();
        EventId(this as *const Self as *const ())
    }

    // Sometimes it is required to get the EventId from unpinned self, for example for
    // implementing Drop trait. It only safe to invoke object actually pinned.
    unsafe fn get_event_id_unchecked(&self) -> EventId {
        EventId(self as *const Self as *const ())
    }
}
