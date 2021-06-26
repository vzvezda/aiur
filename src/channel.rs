//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::marker::{PhantomData, PhantomPinned};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use crate::channel_rt::{ChannelId, ExchangeResult};
use crate::reactor::{EventId, GetEventId, Reactor};
use crate::runtime::Runtime;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

/// Creates a bounded channel and returns ther pair of (sender, receiver).
pub fn channel<'runtime, T, ReactorT: Reactor>(
    rt: &'runtime Runtime<ReactorT>,
) -> (
    Sender<'runtime, T, ReactorT>,
    Recver<'runtime, T, ReactorT>,
) {
    let channel_id = rt.channels().create();
    (
        Sender::new(rt, channel_id),
        Recver::new(rt, channel_id),
    )
}

/// Error type returned by Receiver: the only possible error is channel closed on sender's side.
#[derive(Debug)] // Debug required for Result.unwrap()
pub struct ChRecvError;

// -----------------------------------------------------------------------------------------------
// RuntimeChannel: it is commonly used here: runtime and channel_id coupled together.
struct RuntimeChannel<'runtime, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    channel_id: ChannelId,
}

impl<'runtime, ReactorT: Reactor> RuntimeChannel<'runtime, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, channel_id: ChannelId) -> Self {
        Self { rt, channel_id }
    }

    fn add_sender_fut(&self, waker: &Waker, sender_event_id: EventId, pointer: *mut ()) {
        self.rt
            .channels()
            .add_sender_fut(self.channel_id, waker.clone(), sender_event_id, pointer);
    }

    fn reg_receiver_fut(&self, waker: &Waker, receiver_event_id: EventId, pointer: *mut ()) {
        self.rt.channels().reg_receiver_fut(
            self.channel_id,
            waker.clone(),
            receiver_event_id,
            pointer,
        );
    }

    unsafe fn exchange_sender<T>(&self) -> ExchangeResult {
        self.rt.channels().exchange_sender::<T>(self.channel_id)
    }

    unsafe fn exchange_receiver<T>(&self) -> ExchangeResult {
        self.rt.channels().exchange_receiver::<T>(self.channel_id)
    }

    fn inc_sender(&self) {
        self.rt.channels().inc_sender(self.channel_id);
    }

    fn dec_sender(&self) {
        self.rt.channels().dec_sender(self.channel_id);
    }

    fn close_receiver(&self) {
        self.rt.channels().close_receiver(self.channel_id);
    }

    fn channel_id(&self) -> ChannelId {
        self.channel_id
    }

    fn cancel_sender_fut(&self, event_id: EventId) {
        self.rt
            .channels()
            .cancel_sender_fut(self.channel_id, event_id);
    }

    fn cancel_receiver_fut(&self) {
        self.rt.channels().cancel_receiver_fut(self.channel_id);
    }
}

// -----------------------------------------------------------------------------------------------
// Sender's end of the channel
pub struct Sender<'runtime, T, ReactorT: Reactor> {
    rc: RuntimeChannel<'runtime, ReactorT>,
    temp: PhantomData<T>,
}

impl<'runtime, T, ReactorT: Reactor> Sender<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, channel_id: ChannelId) -> Self {
        let rc = RuntimeChannel::new(rt, channel_id);
        rc.inc_sender();
        Self {
            rc,
            temp: PhantomData,
        }
    }

    pub async fn send(&mut self, value: T) -> Result<(), T> {
        ChSenderFuture::new(&self.rc, value).await
    }
}

// Sender is clonable: having many senders are ok
impl<'runtime, T, ReactorT: Reactor> Clone for Sender<'runtime, T, ReactorT> {
    fn clone(&self) -> Self {
        // new() also increments sender's counters
        Self::new(self.rc.rt, self.rc.channel_id)
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for Sender<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        // we have a -1 Sender. Question: is this possible to have a future:
        //    let sender = ...
        //    let fut = sender.send(5);
        //    drop(sender)  <-- counter is decremented to 0, but channel is still alive
        //    fut.await; <-- channel is released as soon as this future dropped
        // answer: not possible, 'fut' borrows 'sender', so it cannot be dropped.
        self.rc.dec_sender();
    }
}

// -----------------------------------------------------------------------------------------------
// Receiver's end of the channel
pub struct Recver<'runtime, T, ReactorT: Reactor> {
    rc: RuntimeChannel<'runtime, ReactorT>,
    temp: PhantomData<T>,
}

impl<'runtime, T, ReactorT: Reactor> Recver<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, channel_id: ChannelId) -> Self {
        Self {
            rc: RuntimeChannel::new(rt, channel_id),
            temp: PhantomData,
        }
    }

    pub async fn next(&mut self) -> Result<T, ChRecvError> {
        ChNextFuture::new(&self.rc).await
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for Recver<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        self.rc.close_receiver();
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
// Leaf Future returned by async fn send() in Sender
struct ChSenderFuture<'runtime, T, ReactorT: Reactor> {
    rc: RuntimeChannel<'runtime, ReactorT>,
    data: Option<T>,
    state: PeerFutureState,
    _pin: PhantomPinned, // we need the &data to be stable while pinned
}

// This just adds the get_event_id() method to SenderFuture
impl<'runtime, T, ReactorT: Reactor> GetEventId for ChSenderFuture<'runtime, T, ReactorT> {}

impl<'runtime, T, ReactorT: Reactor> ChSenderFuture<'runtime, T, ReactorT> {
    fn new(rc: &RuntimeChannel<'runtime, ReactorT>, value: T) -> Self {
        let rc = RuntimeChannel::new(rc.rt, rc.channel_id);
        Self {
            rc,
            data: Some(value),
            state: PeerFutureState::Created,
            _pin: PhantomPinned,
        }
    }

    fn set_state(&mut self, new_state: PeerFutureState) {
        modtrace!(
            "Channel/ChSenderFuture: {:?} state {:?} -> {:?}",
            self.rc.channel_id(),
            self.state,
            new_state
        );
        self.state = new_state;
    }

    fn set_state_closed(&mut self, exchange_result: ExchangeResult) {
        let new_state = PeerFutureState::Closed;
        modtrace!(
            "Channel/ChSenderFuture: {:?} state {:?} -> {:?}, exchange result: {:?}",
            self.rc.channel_id(),
            self.state,
            new_state,
            exchange_result
        );
        self.state = new_state;
    }

    fn transmit(&mut self, waker: &Waker, event_id: EventId) -> Poll<Result<(), T>> {
        self.set_state(PeerFutureState::Exchanging);

        self.rc.add_sender_fut(
            waker,
            event_id,
            (&mut self.data) as *mut Option<T> as *mut (),
        );

        Poll::Pending
    }

    fn close(&mut self, event_id: EventId) -> Poll<Result<(), T>> {
        if !self.rc.rt.is_awoken(event_id) {
            return Poll::Pending; // not our event, ignore the poll
        }

        // Let make the exchange. When both sender and receiver futures are registered,
        // the receiver would be first to awake and make the actual memory swap. Sender awakes
        // after receiver and it receives a value if value were delivered to receiver.
        // It can also happen that sender was awoken because receiver is dropped,
        // and it would receive disconnected event.
        return match unsafe { self.rc.exchange_sender::<T>() } {
            ExchangeResult::Done =>
            // exchange was perfect, return value to app
            {
                self.set_state_closed(ExchangeResult::Done);
                Poll::Ready(Ok(()))
            }
            ExchangeResult::Disconnected =>
            // receiver is gone, nothing can be sent to this channel anymore
            {
                self.set_state_closed(ExchangeResult::Disconnected);
                Poll::Ready(Err(self.data.take().unwrap()))
            }
            ExchangeResult::TryLater =>
            // receiver future gone but receiver channel object is still alive,
            // will wait for a new attempt.
            {
                // keep state same like self.set_state(PeerFutureState::Exchanging);
                modtrace!(
                    "Channel/ChNextFuture: {:?} state {:?} exchange result: {:?}",
                    self.rc.channel_id(),
                    self.state,
                    ExchangeResult::TryLater);

                Poll::Pending
            }
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for ChSenderFuture<'runtime, T, ReactorT> {
    type Output = Result<(), T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let event_id = self.get_event_id();
        modtrace!("Channel/ChSenderFuture::poll() {:?}", self.rc.channel_id());

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            PeerFutureState::Created => this.transmit(&ctx.waker(), event_id), // always Pending
            PeerFutureState::Exchanging => this.close(event_id),
            PeerFutureState::Closed => {
                panic!(
                    "aiur: channel::ChSenderFuture {:?} was polled after completion.",
                    this.rc.channel_id()
                );
            }
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for ChSenderFuture<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        if matches!(self.state, PeerFutureState::Exchanging) {
            modtrace!(
                "Channel/ChSenderFuture::drop() {:?} - cancel",
                self.rc.channel_id()
            );
            // unsafe: this object was pinned, so it is ok to invoke get_event_id_unchecked
            let event_id = unsafe { self.get_event_id_unchecked() };
            self.rc.cancel_sender_fut(event_id);
        } else {
            // Created: ChSenderFuture was not polled (so it was not pinned) and it
            // is not logically safe to invoke get_event_id_unchecked(). And we really
            // don't have to, because without poll there were not add_sender_fut() invoked.
            // Closed: there is no registration date in ChannelRt anymore
            modtrace!("Channel/ChSenderFuture::drop() {:?}", self.rc.channel_id());
        }
    }
}

// -----------------------------------------------------------------------------------------------
// Leaf Future returned by async fn next() in Recver
//
// Receiver's ChNextFuture has a lot of copy paste with SenderFuture, but unification
// produced more code and less clarity.
pub struct ChNextFuture<'runtime, T, ReactorT: Reactor> {
    rc: RuntimeChannel<'runtime, ReactorT>,
    state: PeerFutureState,
    data: Option<T>,
    _pin: PhantomPinned, // we need the &data to be stable while pinned
}

impl<'runtime, T, ReactorT: Reactor> GetEventId for ChNextFuture<'runtime, T, ReactorT> {}

impl<'runtime, T, ReactorT: Reactor> ChNextFuture<'runtime, T, ReactorT> {
    fn new(rc: &'runtime RuntimeChannel<'runtime, ReactorT>) -> Self {
        Self {
            rc: RuntimeChannel::new(rc.rt, rc.channel_id),
            state: PeerFutureState::Created,
            data: None,
            _pin: PhantomPinned,
        }
    }

    fn set_state(&mut self, new_state: PeerFutureState) {
        modtrace!(
            "Channel/NextFuture: {:?} state {:?} -> {:?}",
            self.rc.channel_id(),
            self.state,
            new_state
        );
        self.state = new_state;
    }

    // Same as set_state but different logging
    fn set_state_closed(&mut self, exchange_result: ExchangeResult) {
        let new_state = PeerFutureState::Closed;
        modtrace!(
            "Channel/NextFuture: {:?} state {:?} -> {:?}, exchange result: {:?}",
            self.rc.channel_id(),
            self.state,
            new_state,
            exchange_result
        );
        self.state = new_state;
    }

    fn transmit(&mut self, waker: &Waker, event_id: EventId) -> Poll<Result<T, ChRecvError>> {
        self.set_state(PeerFutureState::Exchanging);
        self.rc.reg_receiver_fut(
            waker,
            event_id,
            (&mut self.data) as *mut Option<T> as *mut (),
        );

        Poll::Pending
    }

    fn close(&mut self, event_id: EventId) -> Poll<Result<T, ChRecvError>> {
        if !self.rc.rt.is_awoken(event_id) {
            return Poll::Pending; // not our event, ignore the poll
        }

        // Let make the exchange. When both sender and receiver futures are registered,
        // the receiver would be first to awake and make the actual memory swap. It can
        // also happen that receiver was awoken because all sender channels are dropped,
        // and it would receive disconnected event.
        return match unsafe { self.rc.exchange_receiver::<T>() } {
            ExchangeResult::Done =>
            // exchange was perfect, return value to app
            {
                self.set_state_closed(ExchangeResult::Done);
                Poll::Ready(Ok(self.data.take().unwrap()))
            }
            ExchangeResult::Disconnected =>
            // all senders are gone, no more values to recv
            {
                self.set_state_closed(ExchangeResult::Disconnected);
                Poll::Ready(Err(ChRecvError))
            }
            ExchangeResult::TryLater =>
            // sender future gone, will wait for a new one
            {
                // keep state same like self.set_state(PeerFutureState::Exchanging);
                modtrace!(
                    "Channel/ChNextFuture: {:?} state {:?} exchange result: {:?}",
                    self.rc.channel_id(),
                    self.state,
                    ExchangeResult::TryLater);

                Poll::Pending
            }
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for ChNextFuture<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        modtrace!("Channel/NextFuture::drop() {:?}", self.rc.channel_id());
        match self.state {
            PeerFutureState::Exchanging => self.rc.cancel_receiver_fut(),
            _ => (),
        }
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for ChNextFuture<'runtime, T, ReactorT> {
    type Output = Result<T, ChRecvError>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let event_id = self.get_event_id();
        modtrace!("Channel/NextFuture::poll() {:?}", self.rc.channel_id());

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            PeerFutureState::Created => this.transmit(&ctx.waker(), event_id), // always Pending
            PeerFutureState::Exchanging => this.close(event_id),
            PeerFutureState::Closed => {
                panic!(
                    "aiur: channel::NextFuture {:?} was polled after completion.",
                    this.rc.channel_id()
                )
            }
        };
    }
}
