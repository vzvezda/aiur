//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use crate::channel_rt::ChannelId;
use crate::reactor::{EventId, GetEventId, Reactor};
use crate::runtime::Runtime;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

/// Creates a bounded channel and returns ther pair of (sender, receiver).
pub fn channel<'runtime, T, ReactorT: Reactor>(
    rt: &'runtime Runtime<ReactorT>,
) -> (
    ChSender<'runtime, T, ReactorT>,
    ChReceiver<'runtime, T, ReactorT>,
) {
    let channel_id = rt.channels().create();
    (
        ChSender::new(rt, channel_id),
        ChReceiver::new(rt, channel_id),
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

    fn add_sender(&self, waker: &Waker, sender_event_id: EventId, pointer: *mut ()) {
        self.rt
            .channels()
            .add_sender(self.channel_id, waker.clone(), sender_event_id, pointer);
    }

    fn reg_receiver(&self, waker: &Waker, receiver_event_id: EventId, pointer: *mut ()) {
        self.rt
            .channels()
            .reg_receiver(self.channel_id, waker.clone(), receiver_event_id, pointer);
    }

    unsafe fn exchange<T>(&self) -> bool {
        self.rt.channels().exchange::<T>(self.channel_id)
    }

    fn cancel_sender(&self) {
        self.rt.channels().cancel_sender(self.channel_id);
    }

    fn cancel_receiver(&self) {
        self.rt.channels().cancel_receiver(self.channel_id);
    }
}

// -----------------------------------------------------------------------------------------------
// Sender's end of the channel
pub struct ChSender<'runtime, T, ReactorT: Reactor> {
    rc: RuntimeChannel<'runtime, ReactorT>,
    temp: PhantomData<T>,
}

impl<'runtime, T, ReactorT: Reactor> ChSender<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, channel_id: ChannelId) -> Self {
        Self {
            rc: RuntimeChannel::new(rt, channel_id),
            temp: PhantomData,
        }
    }

    pub async fn send(&mut self, value: T) -> Result<(), T> {
        ChSenderFuture::new(&self.rc, value).await
    }
}

// -----------------------------------------------------------------------------------------------
// Receiver's end of the channel
pub struct ChReceiver<'runtime, T, ReactorT: Reactor> {
    _rc: RuntimeChannel<'runtime, ReactorT>,
    _temp: PhantomData<T>,
}

impl<'runtime, T, ReactorT: Reactor> ChReceiver<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, channel_id: ChannelId) -> Self {
        Self {
            _rc: RuntimeChannel::new(rt, channel_id),
            _temp: PhantomData,
        }
    }

    pub async fn next(&mut self) -> Result<T, ChRecvError> {
        Err(ChRecvError)
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
// Leaf Future returned by async fn send() in ChSender
struct ChSenderFuture<'runtime, T, ReactorT: Reactor> {
    rc: RuntimeChannel<'runtime, ReactorT>,
    data: Option<T>,
    state: PeerFutureState,
}

// This just adds the get_event_id() method to SenderFuture
impl<'runtime, T, ReactorT: Reactor> GetEventId for ChSenderFuture<'runtime, T, ReactorT> {}

impl<'runtime, T, ReactorT: Reactor> ChSenderFuture<'runtime, T, ReactorT> {
    fn new(rc: &RuntimeChannel<'runtime, ReactorT>, value: T) -> Self {
        Self {
            rc: RuntimeChannel::new(rc.rt, rc.channel_id),
            data: Some(value),
            state: PeerFutureState::Created,
        }
    }

    fn set_state(&mut self, new_state: PeerFutureState) {
        modtrace!("Channel/SenderFuture: state {:?} -> {:?}", self.state, new_state);
        self.state = new_state;
    }

    fn transmit(&mut self, waker: &Waker, event_id: EventId) -> Poll<Result<(), T>> {
        self.set_state(PeerFutureState::Exchanging);

        self.rc.add_sender(
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

        self.set_state(PeerFutureState::Closed);

        return if unsafe { self.rc.exchange::<T>() } {
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Err(self.data.take().unwrap()))
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for ChSenderFuture<'runtime, T, ReactorT> {
    type Output = Result<(), T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let event_id = self.get_event_id();
        modtrace!("Channel/ChSenderFuture::poll()");

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            PeerFutureState::Created => this.transmit(&ctx.waker(), event_id), // always Pending
            PeerFutureState::Exchanging => this.close(event_id),
            PeerFutureState::Closed => {
                panic!("aiur: channel::ChSenderFuture was polled after completion.")
            }
        };
    }
}
