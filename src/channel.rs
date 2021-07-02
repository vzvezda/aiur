//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::marker::{PhantomData, PhantomPinned};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use crate::channel_rt::{SwapResult, RecverRt, SenderRt, PeerRt};
use crate::reactor::{EventId, GetEventId, Reactor};
use crate::runtime::Runtime;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

/// Creates a bounded channel and returns ther pair of (sender, receiver).
pub fn channel<'runtime, T, ReactorT: Reactor>(
    rt: &'runtime Runtime<ReactorT>,
) -> (Sender<'runtime, T, ReactorT>, Recver<'runtime, T, ReactorT>) {
    let channel_id = rt.channels().create();
    let sender_rt = rt.channels().sender_rt(channel_id);
    let recver_rt = rt.channels().recver_rt(channel_id);
    (Sender::new(rt, sender_rt), Recver::new(rt, recver_rt))
}

/// Error type returned by Receiver: the only possible error is channel closed on sender's side.
#[derive(Debug)] // Debug required for Result.unwrap()
pub struct RecvError;

// -----------------------------------------------------------------------------------------------
// Sender's side of the channel
pub struct Sender<'runtime, T, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    sender_rt: SenderRt<'runtime>,
    temp: PhantomData<T>,
}

impl<'runtime, T, ReactorT: Reactor> Sender<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, sender_rt: SenderRt<'runtime>) -> Self {
        sender_rt.inc_ref();
        Self {
            rt,
            sender_rt,
            temp: PhantomData,
        }
    }

    pub async fn send(&mut self, value: T) -> Result<(), T> {
        SenderFuture::new(self.rt, self.sender_rt, value).await
    }
}

// Sender is clonable: having many senders are ok
impl<'runtime, T, ReactorT: Reactor> Clone for Sender<'runtime, T, ReactorT> {
    fn clone(&self) -> Self {
        // new() also increments sender's counters
        Self::new(self.rt, self.sender_rt)
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
        self.sender_rt.close();
    }
}

// -----------------------------------------------------------------------------------------------
// Receiver's end of the channel
pub struct Recver<'runtime, T, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    recver_rt: RecverRt<'runtime>,
    temp: PhantomData<T>,
}

impl<'runtime, T, ReactorT: Reactor> Recver<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, recver_rt: RecverRt<'runtime>) -> Self {
        Self {
            rt,
            recver_rt,
            temp: PhantomData,
        }
    }

    pub async fn next(&mut self) -> Result<T, RecvError> {
        NextFuture::new(self.rt, self.recver_rt).await
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for Recver<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        self.recver_rt.close();
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
struct SenderFuture<'runtime, T, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    sender_rt: SenderRt<'runtime>,
    data: Option<T>,
    state: PeerFutureState,
    _pin: PhantomPinned, // we need the &data to be stable while pinned
}

// This just adds the get_event_id() method to SenderFuture
impl<'runtime, T, ReactorT: Reactor> GetEventId for SenderFuture<'runtime, T, ReactorT> {}

impl<'runtime, T, ReactorT: Reactor> SenderFuture<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, sender_rt: SenderRt<'runtime>, value: T) -> Self {
        Self {
            rt,
            sender_rt, 
            data: Some(value),
            state: PeerFutureState::Created,
            _pin: PhantomPinned,
        }
    }

    fn set_state(&mut self, new_state: PeerFutureState) {
        modtrace!(
            "Channel/SenderFuture: {:?} state {:?} -> {:?}",
            self.sender_rt.channel_id,
            self.state,
            new_state
        );
        self.state = new_state;
    }

    fn set_state_closed(&mut self, exchange_result: SwapResult) {
        let new_state = PeerFutureState::Closed;
        modtrace!(
            "Channel/SenderFuture: {:?} state {:?} -> {:?}, exchange result: {:?}",
            self.sender_rt.channel_id,
            self.state,
            new_state,
            exchange_result
        );
        self.state = new_state;
    }

    fn transmit(&mut self, waker: &Waker, event_id: EventId) -> Poll<Result<(), T>> {
        self.set_state(PeerFutureState::Exchanging);

        self.sender_rt.pin(
            waker,
            event_id,
            (&mut self.data) as *mut Option<T> as *mut (),
        );

        Poll::Pending
    }

    fn close(&mut self, event_id: EventId) -> Poll<Result<(), T>> {
        if !self.rt.is_awoken(event_id) {
            return Poll::Pending; // not our event, ignore the poll
        }

        // Let make the exchange. When both sender and receiver futures are registered,
        // the receiver would be first to awake and make the actual memory swap. Sender awakes
        // after receiver and it receives a value if value were delivered to receiver.
        // It can also happen that sender was awoken because receiver is dropped,
        // and it would receive disconnected event.
        return match unsafe { self.sender_rt.swap::<T>() } {
            SwapResult::Done =>
            // exchange was perfect, return value to app
            {
                self.set_state_closed(SwapResult::Done);
                Poll::Ready(Ok(()))
            }
            SwapResult::Disconnected =>
            // receiver is gone, nothing can be sent to this channel anymore
            {
                self.set_state_closed(SwapResult::Disconnected);
                Poll::Ready(Err(self.data.take().unwrap()))
            }
            SwapResult::TryLater =>
            // receiver future gone but receiver channel object is still alive,
            // will wait for a new attempt.
            {
                // keep state same like self.set_state(PeerFutureState::Exchanging);
                modtrace!(
                    "Channel/NextFuture: {:?} state {:?} exchange result: {:?}",
                    self.sender_rt.channel_id,
                    self.state,
                    SwapResult::TryLater
                );

                Poll::Pending
            }
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for SenderFuture<'runtime, T, ReactorT> {
    type Output = Result<(), T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let event_id = self.get_event_id();
        modtrace!("Channel/SenderFuture::poll() {:?}", self.sender_rt.channel_id);

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            PeerFutureState::Created => this.transmit(&ctx.waker(), event_id), // always Pending
            PeerFutureState::Exchanging => this.close(event_id),
            PeerFutureState::Closed => {
                panic!(
                    "aiur: channel::SenderFuture {:?} was polled after completion.",
                    this.sender_rt.channel_id
                );
            }
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for SenderFuture<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        if matches!(self.state, PeerFutureState::Exchanging) {
            modtrace!(
                "Channel/SenderFuture::drop() {:?} - cancel",
                self.sender_rt.channel_id
            );
            // unsafe: this object was pinned, so it is ok to invoke get_event_id_unchecked
            let event_id = unsafe { self.get_event_id_unchecked() };
            self.sender_rt.unpin(event_id);
        } else {
            // Created: SenderFuture was not polled (so it was not pinned) and it
            // is not logically safe to invoke get_event_id_unchecked(). And we really
            // don't have to, because without poll there were not add_sender_fut() invoked.
            // Closed: there is no registration date in ChannelRt anymore
            modtrace!("Channel/SenderFuture::drop() {:?}", self.sender_rt.channel_id);
        }
    }
}

// -----------------------------------------------------------------------------------------------
// Leaf Future returned by async fn next() in Recver
//
// Receiver's NextFuture has a lot of copy paste with SenderFuture, but unification
// produced more code and less clarity.
pub struct NextFuture<'runtime, T, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    recver_rt: RecverRt<'runtime>,
    state: PeerFutureState,
    data: Option<T>,
    _pin: PhantomPinned, // we need the &data to be stable while pinned
}

impl<'runtime, T, ReactorT: Reactor> GetEventId for NextFuture<'runtime, T, ReactorT> {}

impl<'runtime, T, ReactorT: Reactor> NextFuture<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, recver_rt: RecverRt<'runtime>) -> Self {
        Self {
            rt, 
            recver_rt,
            state: PeerFutureState::Created,
            data: None,
            _pin: PhantomPinned,
        }
    }

    fn set_state(&mut self, new_state: PeerFutureState) {
        modtrace!(
            "Channel/NextFuture: {:?} state {:?} -> {:?}",
            self.recver_rt.channel_id,
            self.state,
            new_state
        );
        self.state = new_state;
    }

    // Same as set_state but different logging
    fn set_state_closed(&mut self, exchange_result: SwapResult) {
        let new_state = PeerFutureState::Closed;
        modtrace!(
            "Channel/NextFuture: {:?} state {:?} -> {:?}, exchange result: {:?}",
            self.recver_rt.channel_id,
            self.state,
            new_state,
            exchange_result
        );
        self.state = new_state;
    }

    fn transmit(&mut self, waker: &Waker, event_id: EventId) -> Poll<Result<T, RecvError>> {
        self.set_state(PeerFutureState::Exchanging);
        self.recver_rt.pin(
            waker,
            event_id,
            (&mut self.data) as *mut Option<T> as *mut (),
        );

        Poll::Pending
    }

    fn close(&mut self, event_id: EventId) -> Poll<Result<T, RecvError>> {
        if !self.rt.is_awoken(event_id) {
            return Poll::Pending; // not our event, ignore the poll
        }

        // Let make the exchange. When both sender and receiver futures are registered,
        // the receiver would be first to awake and make the actual memory swap. It can
        // also happen that receiver was awoken because all sender channels are dropped,
        // and it would receive disconnected event.
        return match unsafe { self.recver_rt.swap::<T>() } {
            SwapResult::Done =>
            // exchange was perfect, return value to app
            {
                self.set_state_closed(SwapResult::Done);
                Poll::Ready(Ok(self.data.take().unwrap()))
            }
            SwapResult::Disconnected =>
            // all senders are gone, no more values to recv
            {
                self.set_state_closed(SwapResult::Disconnected);
                Poll::Ready(Err(RecvError))
            }
            SwapResult::TryLater =>
            // sender future gone, will wait for a new one
            {
                // keep state same like self.set_state(PeerFutureState::Exchanging);
                modtrace!(
                    "Channel/NextFuture: {:?} state {:?} exchange result: {:?}",
                    self.recver_rt.channel_id,
                    self.state,
                    SwapResult::TryLater
                );

                Poll::Pending
            }
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for NextFuture<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        modtrace!("Channel/NextFuture::drop() {:?}", self.recver_rt.channel_id);
        if matches!(self.state, PeerFutureState::Exchanging) {
            // unsafe: this object was pinned, so it is ok to invoke get_event_id_unchecked
            let event_id = unsafe { self.get_event_id_unchecked() };
            self.recver_rt.unpin(event_id);
        }
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for NextFuture<'runtime, T, ReactorT> {
    type Output = Result<T, RecvError>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let event_id = self.get_event_id();
        modtrace!("Channel/NextFuture::poll() {:?}", self.recver_rt.channel_id);

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            PeerFutureState::Created => this.transmit(&ctx.waker(), event_id), // always Pending
            PeerFutureState::Exchanging => this.close(event_id),
            PeerFutureState::Closed => {
                panic!(
                    "aiur: channel::NextFuture {:?} was polled after completion.",
                    this.recver_rt.channel_id
                )
            }
        };
    }
}
