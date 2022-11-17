//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::event_node::EventNode;
use crate::oneshot_rt::OneshotId;
use crate::reactor::{EventId, Reactor};
use crate::runtime::Runtime;
use crate::tracer::Tracer;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

// -----------------------------------------------------------------------------------------------
// Public oneshot() API

/// Creates a new oneshot channel and returns the pair of (sender, receiver).
///
/// The created channel is bounded, whenever a sender sends a data it is suspended in await
/// point until either receiver had the data received or oneshot channel got disconnected.
///
/// Neither sender nor receiver can be cloned, it is single use, single producer, single consumer
/// communication channel.
pub fn oneshot<'runtime, T, ReactorT: Reactor>(
    rt: &'runtime Runtime<ReactorT>,
) -> (
    SenderOnce<'runtime, T, ReactorT>,
    RecverOnce<'runtime, T, ReactorT>,
) {
    let oneshot_id = rt.oneshots().create();
    (
        SenderOnce::new(rt, oneshot_id),
        RecverOnce::new(rt, oneshot_id),
    )
}

/// Error type returned by Receiver: the only possible error is oneshot channel closed
/// on sender's side.
#[derive(Debug)] // Debug required for Result.unwrap()
pub struct RecvError;

// -----------------------------------------------------------------------------------------------
// RuntimeOneshot: it is often used here: runtime and oneshot_id coupled together.
struct RuntimeOneshot<'runtime, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    oneshot_id: OneshotId,
}

impl<'runtime, ReactorT: Reactor> RuntimeOneshot<'runtime, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, oneshot_id: OneshotId) -> Self {
        RuntimeOneshot { rt, oneshot_id }
    }

    fn reg_sender(&self, sender_event_id: EventId, pointer: *mut ()) {
        self.rt
            .oneshots()
            .reg_sender(self.oneshot_id, sender_event_id, pointer);
    }

    fn reg_receiver(&self, receiver_event_id: EventId, pointer: *mut ()) {
        self.rt
            .oneshots()
            .reg_receiver(self.oneshot_id, receiver_event_id, pointer);
    }

    unsafe fn exchange<T>(&self) -> bool {
        self.rt.oneshots().exchange::<T>(self.oneshot_id)
    }

    fn cancel_sender(&self) {
        self.rt.oneshots().cancel_sender(self.oneshot_id);
    }

    fn cancel_receiver(&self) {
        self.rt.oneshots().cancel_receiver(self.oneshot_id);
    }

    fn oneshot_id(&self) -> OneshotId {
        self.oneshot_id
    }
}

// -----------------------------------------------------------------------------------------------
/// The sending half of the oneshot channel created by [oneshot()] function.
pub struct SenderOnce<'runtime, T, ReactorT: Reactor> {
    inner: SenderInner<'runtime, T, ReactorT>, // use inner to hide enum internals
}

// Possible state of the SenderOnce: before and after .send() is invoked
enum SenderInner<'runtime, T, ReactorT: Reactor> {
    Created(RuntimeOneshot<'runtime, ReactorT>),
    Sent(PhantomData<T>), // Type required for Future
}

impl<'runtime, T, ReactorT: Reactor> SenderOnce<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, oneshot_id: OneshotId) -> Self {
        SenderOnce {
            inner: SenderInner::Created(RuntimeOneshot::new(rt, oneshot_id)),
        }
    }

    /// Sends value to the receiver side of the channel. If receiver end is already closed,
    /// the original value returned as error in result.
    pub async fn send(&mut self, value: T) -> Result<(), T> {
        let prev = std::mem::replace(&mut self.inner, SenderInner::Sent(PhantomData));

        // TODO: perhaps send should receive (self,..) instead of (&mut self)?
        match prev {
            SenderInner::Sent(_) => panic!(concat!(
                "aiur: oneshot::SenderOnce::send() invoked twice.",
                "Oneshot channel can be only used for one transfer."
            )),

            SenderInner::Created(ref rc) => SenderFuture::new(rc, value).await,
        }
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for SenderOnce<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        if let SenderInner::Created(ref runtime_channel) = self.inner {
            runtime_channel.cancel_sender();
        }
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Debug)]
enum PeerFutureState {
    Created,
    Exchanging,
    Closed,
}

// -----------------------------------------------------------------------------------------------
struct SenderFuture<'runtime, T, ReactorT: Reactor> {
    runtime_channel: RuntimeOneshot<'runtime, ReactorT>,
    event_node: EventNode,
    data: Option<T>,
    state: PeerFutureState,
}

impl<'runtime, T, ReactorT: Reactor> SenderFuture<'runtime, T, ReactorT> {
    fn new(rc: &RuntimeOneshot<'runtime, ReactorT>, value: T) -> Self {
        SenderFuture {
            runtime_channel: RuntimeOneshot::new(rc.rt, rc.oneshot_id),
            event_node: EventNode::new(),
            data: Some(value),
            state: PeerFutureState::Created,
        }
    }

    fn set_state(&mut self, new_state: PeerFutureState) {
        modtrace!(
            self.tracer(),
            "oneshot_sender_future: {:?} state {:?} -> {:?}",
            self.runtime_channel.oneshot_id(),
            self.state,
            new_state
        );
        self.state = new_state;
    }

    fn transmit(&mut self, event_id: EventId) -> Poll<Result<(), T>> {
        self.set_state(PeerFutureState::Exchanging);

        self.runtime_channel
            .reg_sender(event_id, (&mut self.data) as *mut Option<T> as *mut ());

        Poll::Pending
    }

    fn close(&mut self) -> Poll<Result<(), T>> {
        if !self.event_node.is_awoken_for(self.runtime_channel.rt) {
            return Poll::Pending; // not our event, ignore the poll
        }

        self.set_state(PeerFutureState::Closed);

        return if unsafe { self.runtime_channel.exchange::<T>() } {
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Err(self.data.take().unwrap()))
        };
    }

    fn tracer(&self) -> &Tracer {
        self.runtime_channel.rt.tracer()
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for SenderFuture<'runtime, T, ReactorT> {
    type Output = Result<(), T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        modtrace!(
            self.tracer(),
            "oneshot_sender_future: in the poll() {:?}",
            self.runtime_channel.oneshot_id()
        );

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            PeerFutureState::Created => {
                let event_id = unsafe { this.event_node.on_pin(ctx) };
                this.transmit(event_id) // always returns Pending
            }
            PeerFutureState::Exchanging => this.close(),
            PeerFutureState::Closed => {
                panic!("aiur/oneshot_sender_future: was polled after completion.")
            }
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for SenderFuture<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        modtrace!(self.tracer(), "oneshot_sender_future: drop()");
        self.runtime_channel.cancel_sender();
        let _ = self.event_node.on_cancel(); // remove the events from frozen list
    }
}

// -----------------------------------------------------------------------------------------------
// RecverOnce (Future)
//
// Receiver has a lot of copy paste with SenderFuture, but unification produced more code and
// less clarity.
//
/// The receiving half of the oneshot channel created by [oneshot()] function.
///
/// It implements the [std::future::Future], so app code just awaits on this object to receive
/// the value from sender.
pub struct RecverOnce<'runtime, T, ReactorT: Reactor> {
    runtime_channel: RuntimeOneshot<'runtime, ReactorT>,
    event_node: EventNode,
    state: PeerFutureState,
    data: Option<T>,
}

impl<'runtime, T, ReactorT: Reactor> RecverOnce<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, oneshot_id: OneshotId) -> Self {
        RecverOnce {
            runtime_channel: RuntimeOneshot::new(rt, oneshot_id),
            event_node: EventNode::new(),
            state: PeerFutureState::Created,
            data: None,
        }
    }

    fn set_state(&mut self, new_state: PeerFutureState) {
        modtrace!(
            self.tracer(),
            "oneshot_recver_future: state {:?} -> {:?}",
            self.state,
            new_state
        );
        self.state = new_state;
    }

    fn transmit(&mut self, event_id: EventId) -> Poll<Result<T, RecvError>> {
        self.set_state(PeerFutureState::Exchanging);
        self.runtime_channel
            .reg_receiver(event_id, (&mut self.data) as *mut Option<T> as *mut ());

        Poll::Pending
    }

    fn close(&mut self) -> Poll<Result<T, RecvError>> {
        if !self.event_node.is_awoken_for(self.runtime_channel.rt) {
            return Poll::Pending;
        }

        self.set_state(PeerFutureState::Closed);
        return if unsafe { self.runtime_channel.exchange::<T>() } {
            Poll::Ready(Ok(self.data.take().unwrap()))
        } else {
            Poll::Ready(Err(RecvError))
        };
    }

    fn tracer(&self) -> &Tracer {
        self.runtime_channel.rt.tracer()
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for RecverOnce<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        modtrace!(self.tracer(), "oneshot_recver_future: in the drop()");
        self.runtime_channel.cancel_receiver();
        let _ = self.event_node.on_cancel(); // remove the events from frozen list
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for RecverOnce<'runtime, T, ReactorT> {
    type Output = Result<T, RecvError>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        modtrace!(self.tracer(), "oneshot_recver_future: in the poll()");

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            PeerFutureState::Created => {
                let event_id = unsafe { this.event_node.on_pin(ctx) };
                this.transmit(event_id) // always returns Pending
            }
            PeerFutureState::Exchanging => this.close(),
            PeerFutureState::Closed => {
                panic!("aiur/oneshot_recver_future: was polled after completion.")
            }
        };
    }
}
