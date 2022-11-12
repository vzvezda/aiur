//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use crate::Runtime;
use crate::TemporalReactor;
use crate::{EventNode, EventId};

/// Performs the async sleep.
///
/// This function requires reactor with timer operations implemented according to
/// [TemporalReactor] trait. Sleeping used intensively in tests of this crate.
///
/// Panics if provided duration exceeds the maximum value (MAX_TIMER_DURATION_MS)
pub async fn sleep<ReactorT: TemporalReactor>(rt: &Runtime<ReactorT>, duration: Duration) {
    TimerFuture::new(rt, duration).await
}

// The rest code is private TimerFuture implementation

// Possible states for the timer future.
enum TimerState {
    Created { duration: Duration },
    Scheduled,
    Done,
}

// Leaf future for timer.
struct TimerFuture<'runtime, ReactorT: TemporalReactor> {
    rt: &'runtime Runtime<ReactorT>,
    state: TimerState,
    event_node: EventNode,
}

impl<'rt, ReactorT: TemporalReactor> TimerFuture<'rt, ReactorT> {
    fn new(rt: &'rt Runtime<ReactorT>, duration: Duration) -> Self {
        TimerFuture {
            rt,
            state: TimerState::Created { duration },
            event_node: EventNode::new(),
        }
    }

    // Schedules the timer in the reactor.
    fn schedule(&mut self, event_id: EventId, duration: Duration) -> Poll<()> {
        // Timer in has to be in "Created" state, so we cannot schedule the timer twice.
        debug_assert!(matches!(self.state, TimerState::Created { .. }));

        self.state = TimerState::Scheduled;
        self.rt.io().schedule_timer(event_id, duration);
        Poll::Pending // always pending
    }

    // Verifies in reactor if given timer event is ready
    fn verify(&mut self, event_id: EventId) -> Poll<()> {
        // Timer in has to be in "Scheduled" state
        debug_assert!(matches!(self.state, TimerState::Scheduled));

        if self.rt.is_awoken_for(event_id) {
            self.state = TimerState::Done;
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}


// Cancels timer event in the reactor.
impl<'rt, ReactorT: TemporalReactor> Drop for TimerFuture<'rt, ReactorT> {
    fn drop(&mut self) {
        match self.state {
            // Unsafe usage: the object is a Future that has to be pinned to be
            // in Scheduled state, so we are getting the correct event id here.
            TimerState::Scheduled => self
                .rt
                .io()
                .cancel_timer(self.event_node.get_event_id()),
            _ => (),
        }
    }
}

impl<'rt, ReactorT: TemporalReactor> Future for TimerFuture<'rt, ReactorT> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::get_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            TimerState::Created { duration } => {
                let event_id = this.event_node.on_pin(ctx);
                this.schedule(event_id, duration)
            }
            TimerState::Scheduled => this.verify(this.event_node.get_event_id()),
            TimerState::Done => Poll::Ready(()), // perhaps we should panic instead
        };
    }
}
