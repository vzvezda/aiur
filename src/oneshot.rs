//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::marker::{PhantomData, PhantomPinned};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use crate::oneshot_rt::OneshotId;
use crate::reactor::{EventId, GetEventId, Reactor};
use crate::runtime::Runtime;

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
// RuntimeOneshot: it is commonly used here: runtime and oneshot_id coupled together.
struct RuntimeOneshot<'runtime, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    oneshot_id: OneshotId,
}

impl<'runtime, ReactorT: Reactor> RuntimeOneshot<'runtime, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, oneshot_id: OneshotId) -> Self {
        RuntimeOneshot { rt, oneshot_id }
    }

    fn reg_sender(&self, waker: &Waker, sender_event_id: EventId, pointer: *mut ()) {
        self.rt
            .oneshots()
            .reg_sender(self.oneshot_id, waker.clone(), sender_event_id, pointer);
    }

    fn reg_receiver(&self, waker: &Waker, receiver_event_id: EventId, pointer: *mut ()) {
        self.rt
            .oneshots()
            .reg_receiver(self.oneshot_id, waker.clone(), receiver_event_id, pointer);
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
    data: Option<T>,
    state: PeerFutureState,
    _pin: PhantomPinned, // we need the &data to be stable while pinned
}

// This just adds the get_event_id() method to SenderFuture
impl<'runtime, T, ReactorT: Reactor> GetEventId for SenderFuture<'runtime, T, ReactorT> {}

impl<'runtime, T, ReactorT: Reactor> SenderFuture<'runtime, T, ReactorT> {
    fn new(rc: &RuntimeOneshot<'runtime, ReactorT>, value: T) -> Self {
        SenderFuture {
            runtime_channel: RuntimeOneshot::new(rc.rt, rc.oneshot_id),
            data: Some(value),
            state: PeerFutureState::Created,
            _pin: PhantomPinned,
        }
    }

    fn set_state(&mut self, new_state: PeerFutureState) {
        modtrace!(
            "Oneshot/SenderFuture: {:?} state {:?} -> {:?}",
            self.runtime_channel.oneshot_id(),
            self.state,
            new_state
        );
        self.state = new_state;
    }

    fn transmit(&mut self, waker: &Waker, event_id: EventId) -> Poll<Result<(), T>> {
        self.set_state(PeerFutureState::Exchanging);

        self.runtime_channel.reg_sender(
            waker,
            event_id,
            (&mut self.data) as *mut Option<T> as *mut (),
        );

        Poll::Pending
    }

    fn close(&mut self, event_id: EventId) -> Poll<Result<(), T>> {
        if !self.runtime_channel.rt.is_awoken(event_id) {
            return Poll::Pending; // not our event, ignore the poll
        }

        self.set_state(PeerFutureState::Closed);

        return if unsafe { self.runtime_channel.exchange::<T>() } {
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Err(self.data.take().unwrap()))
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for SenderFuture<'runtime, T, ReactorT> {
    type Output = Result<(), T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        modtrace!("Oneshot/SenderFuture::poll() {:?}", self.runtime_channel.oneshot_id());
        let event_id = self.get_event_id();

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            PeerFutureState::Created => this.transmit(&ctx.waker(), event_id), // always Pending
            PeerFutureState::Exchanging => this.close(event_id),
            PeerFutureState::Closed => {
                panic!("aiur: oneshot::SenderFuture was polled after completion.")
            }
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for SenderFuture<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        modtrace!("Oneshot/SenderFuture::drop()");
        self.runtime_channel.cancel_sender();
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
    state: PeerFutureState,
    data: Option<T>,
    _pin: PhantomPinned, // we need the &data to be stable while pinned
}

impl<'runtime, T, ReactorT: Reactor> GetEventId for RecverOnce<'runtime, T, ReactorT> {}

impl<'runtime, T, ReactorT: Reactor> RecverOnce<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, oneshot_id: OneshotId) -> Self {
        RecverOnce {
            runtime_channel: RuntimeOneshot::new(rt, oneshot_id),
            state: PeerFutureState::Created,
            data: None,
            _pin: PhantomPinned,
        }
    }

    fn set_state(&mut self, new_state: PeerFutureState) {
        modtrace!(
            "Oneshot/ReceiverFuture: state {:?} -> {:?}",
            self.state,
            new_state
        );
        self.state = new_state;
    }

    fn transmit(&mut self, waker: &Waker, event_id: EventId) -> Poll<Result<T, RecvError>> {
        self.set_state(PeerFutureState::Exchanging);
        self.runtime_channel.reg_receiver(
            waker,
            event_id,
            (&mut self.data) as *mut Option<T> as *mut (),
        );

        Poll::Pending
    }

    fn close(&mut self, event_id: EventId) -> Poll<Result<T, RecvError>> {
        if !self.runtime_channel.rt.is_awoken(event_id) {
            return Poll::Pending;
        }

        self.set_state(PeerFutureState::Closed);
        return if unsafe { self.runtime_channel.exchange::<T>() } {
            Poll::Ready(Ok(self.data.take().unwrap()))
        } else {
            Poll::Ready(Err(RecvError))
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for RecverOnce<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        modtrace!("Oneshot/ReceiverFuture::drop()");
        self.runtime_channel.cancel_receiver();
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for RecverOnce<'runtime, T, ReactorT> {
    type Output = Result<T, RecvError>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let event_id = self.get_event_id();
        modtrace!("Oneshot/ReceiverFuture::poll()");

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            PeerFutureState::Created => this.transmit(&ctx.waker(), event_id), // always Pending
            PeerFutureState::Exchanging => this.close(event_id),
            PeerFutureState::Closed => {
                panic!("aiur: oneshot::ReceiverFuture was polled after completion.")
            }
        };
    }
}
