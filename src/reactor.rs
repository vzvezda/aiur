//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::pin::Pin;
use std::task::Waker;
use std::time::Duration;

/// Reactor API for Executor.
///
/// External crate implements the trait to create a Runtime with both executor and reactor.
pub trait Reactor {
    /// The only method Runtime needs from the reactor is to wait for I/O to complete.
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
/// or by other means) and this id is kind of cheap method for leaf future to verify if executor
/// has invoked the poll() method because this future was awoken.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct EventId(pub(crate) *const ());

impl EventId {
    pub fn null() -> Self {
        EventId(std::ptr::null())
    }
}

/// Mixin to produce the EventId from the address.
///
/// Reactor crate can use this cheap method to produce unique [EventId] by using the address 
/// of pinned leaf future. Just declare the `impl GetEventId for MyLeafFuture` to have 
/// [GetEventId::get_event_id()] implementation for `MyLeafFuture`. 
///
/// By having EventId based on memory address, you don't have to store it in your futures.
pub trait GetEventId {
    /// Makes the [EventId] from the address of pinned object. 
    ///
    /// It should be logically safe: once pinned address cannot be used by another object until 
    /// destruction and destruction should remove EventId from reactor. With [Unpin] future 
    /// it should be possible to break this and have invalid EventId - this is up to reactor 
    /// crate to care about it.
    fn get_event_id(self: &Pin<&mut Self>) -> EventId {
        let this = self.as_ref().get_ref();
        EventId(this as *const Self as *const ())
    }

    /// Makes the [EventId] from unpinned object.
    ///
    /// Sometimes it is required to get the [EventId] from unpinned self, for example for
    /// implementing the Drop trait, which is due to historical reasons has `&mut self` spec. 
    /// It does not makes anything unsafe in Rust terms, but caller should ensure that 
    /// address of the object is consistent with previous or future invocations.
    unsafe fn get_event_id_unchecked(&self) -> EventId {
        EventId(self as *const Self as *const ())
    }
}
