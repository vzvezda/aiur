//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use crate::channel_api::{ChannelId};
use crate::reactor::{Reactor, GetEventId, EventId };
use crate::runtime::Runtime;

// -----------------------------------------------------------------------------------------------
// Sender's end of the channel
pub struct Sender<'runtime, T, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    channel_id: ChannelId,
    marker: PhantomData<T>,
}

impl<'runtime, T, ReactorT: Reactor> Sender<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, channel_id: ChannelId) -> Self {
        Sender {
            rt,
            channel_id,
            marker: PhantomData,
        }
    }

    pub async fn send(&mut self, value: T) -> Result<(), ()> {
        // nullify the channel id, so droping Sender does not closes channel send anymore
        let cid = self.channel_id;
        self.channel_id = ChannelId::null();

        SenderFuture::new(self.rt, cid, value).await
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for Sender<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        println!("sender: drop");
        self.rt.channels().drop_sender(self.channel_id);
    }
}

// -----------------------------------------------------------------------------------------------
enum SenderState {
    Created,
    Transmitting,
    Done,
}

struct SenderFuture<'runtime, T, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    channel_id: ChannelId,
    data: Option<T>,
    result: Option<Result<(), ()>>,
    state: SenderState,
}

// This just makes the get_event_id() method in TimerFuture
impl<'runtime, T, ReactorT: Reactor> GetEventId for SenderFuture<'runtime, T, ReactorT> {}

impl<'runtime, T, ReactorT: Reactor> SenderFuture<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, channel_id: ChannelId, value: T) -> Self {
        SenderFuture {
            rt,
            channel_id,
            data: Some(value),
            result: None,
            state: SenderState::Created,
        }
    }

    fn transmit(&mut self, waker: Waker, event_id: EventId) -> Poll<Result<(), ()>> {
        println!("Sender: transmit");
        self.state = SenderState::Transmitting;
        self.rt.channels().reg_sender(self.channel_id, waker, event_id, 
            (&mut self.data) as *mut Option<T> as *mut ());
        Poll::Pending
    }

    fn close(&mut self, event_id: EventId) -> Poll<Result<(), ()>> {
        println!("Sender: closing");
        if !self.rt.is_awoken(event_id) {
            return Poll::Pending;
        }

        self.state = SenderState::Done;
        if unsafe { self.rt.channels().exchange::<T>(self.channel_id) } {
            self.result = Some(Ok(()));
        } else {
            self.result = Some(Err(()));
        }

        Poll::Ready(self.result.unwrap())
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for SenderFuture<'runtime, T, ReactorT> {
    type Output = Result<(), ()>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        println!("Sender: poll");
        let event_id = self.get_event_id();

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            SenderState::Created => this.transmit(ctx.waker().clone(), event_id),
            SenderState::Transmitting => this.close(event_id),
            SenderState::Done => Poll::Ready(this.result.unwrap()),
        };
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for SenderFuture<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        println!("sender future: drop");
        self.rt.channels().drop_sender(self.channel_id); // can be null()
    }
}


// -----------------------------------------------------------------------------------------------
enum ReceiverState {
    Created,
    Transmitting,
    Done,
}

pub struct Receiver<'runtime, T, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    channel_id: ChannelId,
    state: ReceiverState,
    data: Option<T>,
}

impl<'runtime, T, ReactorT: Reactor> GetEventId for Receiver<'runtime, T, ReactorT> {}

impl<'runtime, T, ReactorT: Reactor> Receiver<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, channel_id: ChannelId) -> Self {
        Receiver {
            rt,
            channel_id,
            state: ReceiverState::Created,
            data: None,
        }
    }

    fn transmit(&mut self, waker: Waker, event_id: EventId) -> Poll<Result<T, bool>> {
        println!("receiver: transmit");
        self.state = ReceiverState::Transmitting;
        self.rt.channels().reg_receiver(self.channel_id, waker, event_id, 
            (&mut self.data) as *mut Option<T> as *mut ());
        Poll::Pending
    }

    fn close(&mut self, event_id: EventId) -> Poll<Result<T, bool>> {
        println!("receiver: close");
        if !self.rt.is_awoken(event_id) {
            return Poll::Pending;
        }

        self.state = ReceiverState::Done;
        return if unsafe { self.rt.channels().exchange::<T>(self.channel_id) } {
            Poll::Ready(Ok(self.data.take().unwrap()))
        } else {
            Poll::Ready(Err(false))
        }
    }
}

impl<'runtime, T, ReactorT: Reactor> Drop for Receiver<'runtime, T, ReactorT> {
    fn drop(&mut self) {
        println!("receiver: drop");
        self.rt.channels().drop_receiver(self.channel_id);
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for Receiver<'runtime, T, ReactorT> {
    type Output = Result<T, bool>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let event_id = self.get_event_id();
        println!("receiver: poll");

        // Unsafe usage: this function does not moves out data from self, as required by
        // Pin::map_unchecked_mut().
        let this = unsafe { self.get_unchecked_mut() };

        return match this.state {
            ReceiverState::Created => this.transmit(ctx.waker().clone(), event_id),
            ReceiverState::Transmitting => this.close(event_id),
            ReceiverState::Done => Poll::Ready(Err(false)),
        };
    }
}

// -----------------------------------------------------------------------------------------------

/// Creates a new channel and returns an pair of (sender, receiver).
pub fn oneshot<'runtime, T, ReactorT: Reactor>(
    rt: &'runtime Runtime<ReactorT>,
) -> (
    Sender<'runtime, T, ReactorT>,
    Receiver<'runtime, T, ReactorT>,
) {
    let channel_id = rt.channels().create();
    (Sender::new(rt, channel_id), Receiver::new(rt, channel_id))
}
