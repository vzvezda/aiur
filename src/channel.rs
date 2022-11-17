//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::channel_rt::{PeerRt, RecverRt, SenderRt, SwapResult};
use crate::event_node::EventNode;
use crate::reactor::{EventId, Reactor};
use crate::runtime::Runtime;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

/// Creates a new asynchronous channel returning the pair of (Sender, Receiver).
///
/// This creates the bounded channel, so whenever a sender sends a data it is suspended
/// in await point until either receiver had the data received or channel got disconnected.
///
/// Sender can be cloned to send data to the same channel, but only one Receiver is supported.
///
/// While there is a channel half that awaits transmission and another half is gone,
/// operation Result would be an error. In a case of the receiver it would be RecvError. When
/// sender detects that receiver is gone the error contains the value sender was supposed
/// to send.
pub fn channel<'runtime, T, ReactorT: Reactor>(
    rt: &'runtime Runtime<ReactorT>,
) -> (Sender<'runtime, T, ReactorT>, Recver<'runtime, T, ReactorT>) {
    let channel_id = rt.channels().create();
    let sender_rt = rt.channels().sender_rt(channel_id);
    let recver_rt = rt.channels().recver_rt(channel_id);
    (Sender::new(rt, sender_rt), Recver::new(rt, recver_rt))
}

/// Error type returned by Receiver: the only possible error is channel closed on sender's side.
#[derive(Debug)] // Debug is required for Result.unwrap()
pub struct RecvError;

// -----------------------------------------------------------------------------------------------
/// The sending half of the channel created by [channel()] function.
///
/// Messages can be sent through this channel with [Sender::send()].
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

    /// Sends a value to a receiver half of communication channel.
    ///
    /// The awaited send() operation does not return until receiver gets the data or
    /// communication channel is gone by having receiver object dropped. In a case of
    /// closed channel sender receives the value back.
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
/// The receiving half of the channel created by [channel()] function.
///
/// Messages from sender can be awaited and received with [Recver::next()].
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

    /// Reads a next value from channel sent by sender half. Error is returned when all
    /// senders are gone, so no values can be received anymore.
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
    event_node: EventNode,
    sender_rt: SenderRt<'runtime>,
    data: Option<T>,
    state: PeerFutureState,
}

impl<'runtime, T, ReactorT: Reactor> SenderFuture<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, sender_rt: SenderRt<'runtime>, value: T) -> Self {
        Self {
            rt,
            event_node: EventNode::new(),
            sender_rt,
            data: Some(value),
            state: PeerFutureState::Created,
        }
    }

    fn set_state(&mut self, new_state: PeerFutureState) {
        modtrace!(
            self.rt.tracer(),
            "channel_sender_future: {:?} state {:?} -> {:?}",
            self.sender_rt.channel_id,
            self.state,
            new_state
        );
        self.state = new_state;
    }

    fn set_state_closed(&mut self, exchange_result: SwapResult) {
        let new_state = PeerFutureState::Closed;
        modtrace!(
            self.rt.tracer(),
            "channel_sender_future: {:?} state {:?} -> {:?}, exchange result: {:?}",
            self.sender_rt.channel_id,
            self.state,
            new_state,
            exchange_result
        );
        self.state = new_state;
    }

    fn transmit(&mut self, event_id: EventId) -> Poll<Result<(), T>> {
        self.set_state(PeerFutureState::Exchanging);

        self.sender_rt
            .pin(event_id, (&mut self.data) as *mut Option<T> as *mut ());

        Poll::Pending
    }

    fn close(&mut self) -> Poll<Result<(), T>> {
        if !self.event_node.is_awoken_for(self.rt) {
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
                    self.rt.tracer(),
                    "channel_next_future: {:?} state {:?} exchange result: {:?}",
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
        modtrace!(
            self.rt.tracer(),
            "channel_sender_future: in the poll() {:?}",
            self.sender_rt.channel_id
        );

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::get_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            PeerFutureState::Created => {
                let event_id = unsafe { this.event_node.on_pin(ctx) };
                this.transmit(event_id) // always Pending
            }
            PeerFutureState::Exchanging => this.close(),
            PeerFutureState::Closed => {
                panic!(
                    "aiur/channel_sender_future: {:?} was polled after completion.",
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
                self.rt.tracer(),
                "channel_sender_future: in the drop() {:?} - cancelling",
                self.sender_rt.channel_id
            );
            self.sender_rt.unpin(self.event_node.get_event_id());
            let _ = self.event_node.on_cancel(); // remove the events from frozen list
        } else {
            // Created: SenderFuture was not polled (so it was not pinned) and it
            // is not logically safe to invoke get_event_id_unchecked(). And we really
            // don't have to, because without poll there were not add_sender_fut() invoked.
            // Closed: there is no registration data in ChannelRt anymore
            modtrace!(
                self.rt.tracer(),
                "channel_sender_future: in the drop() {:?} no op",
                self.sender_rt.channel_id
            );
        }
    }
}

// -----------------------------------------------------------------------------------------------
// Leaf Future returned by async fn next() in Recver
//
// Receiver's NextFuture has a lot of copy paste with SenderFuture, but unification
// produced more code and less clarity.
//
pub struct NextFuture<'runtime, T, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    event_node: EventNode,
    recver_rt: RecverRt<'runtime>,
    state: PeerFutureState,
    data: Option<T>,
}

impl<'runtime, T, ReactorT: Reactor> NextFuture<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, recver_rt: RecverRt<'runtime>) -> Self {
        Self {
            rt,
            event_node: EventNode::new(),
            recver_rt,
            state: PeerFutureState::Created,
            data: None,
        }
    }

    fn set_state(&mut self, new_state: PeerFutureState) {
        modtrace!(
            self.rt.tracer(),
            "channel_next_future: {:?} state {:?} -> {:?}",
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
            self.rt.tracer(),
            "channel_next_future: {:?} state {:?} -> {:?}, exchange result: {:?}",
            self.recver_rt.channel_id,
            self.state,
            new_state,
            exchange_result
        );
        self.state = new_state;
    }

    fn transmit(&mut self, event_id: EventId) -> Poll<Result<T, RecvError>> {
        self.set_state(PeerFutureState::Exchanging);
        self.recver_rt
            .pin(event_id, (&mut self.data) as *mut Option<T> as *mut ());

        Poll::Pending
    }

    fn close(&mut self) -> Poll<Result<T, RecvError>> {
        if !self.event_node.is_awoken_for(self.rt) {
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
                    self.rt.tracer(),
                    "channel_next_future: {:?} state {:?} exchange result: {:?}",
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
        if matches!(self.state, PeerFutureState::Exchanging) {
            modtrace!(
                self.rt.tracer(),
                "channel_next_future: in the drop() {:?} cancelling",
                self.recver_rt.channel_id
            );

            self.recver_rt.unpin(self.event_node.get_event_id());
            let _ = self.event_node.on_cancel(); // remove the events from frozen list
        } else {
            modtrace!(
                self.rt.tracer(),
                "channel_next_future: in the drop() {:?} no op",
                self.recver_rt.channel_id
            );
        }
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for NextFuture<'runtime, T, ReactorT> {
    type Output = Result<T, RecvError>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        modtrace!(
            self.rt.tracer(),
            "channel_next_future: in the poll() {:?}",
            self.recver_rt.channel_id
        );

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            PeerFutureState::Created => {
                let event_id = unsafe { this.event_node.on_pin(ctx) };
                this.transmit(event_id) // always Pending
            }
            PeerFutureState::Exchanging => this.close(),
            PeerFutureState::Closed => {
                panic!(
                    "aiur/channel_next_future: {:?} was polled after completion.",
                    this.recver_rt.channel_id
                )
            }
        };
    }
}
