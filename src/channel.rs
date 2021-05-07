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


/// Creates a bounded channel and returns ther pair of (sender, receiver).
pub fn channel<'runtime, T, ReactorT: Reactor>(
    rt: &'runtime Runtime<ReactorT>,
) -> (
    ChSender<'runtime, T, ReactorT>,
    ChReceiver<'runtime, T, ReactorT>,
) {
    let channel_id = rt.channels().create();
    (ChSender::new(rt, channel_id), ChReceiver::new(rt, channel_id))
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
}

pub struct ChSender<'runtime, T, ReactorT: Reactor> {
    _rc: RuntimeChannel<'runtime, ReactorT>,
    _temp: PhantomData<T>,
}

impl<'runtime, T, ReactorT: Reactor> ChSender<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>, channel_id: ChannelId) -> Self {
        Self { 
            _rc: RuntimeChannel::new(rt, channel_id),
            _temp: PhantomData,
        }
    }

    pub async fn send(&mut self, value: T) -> Result<(), T> {
        Ok(())
    }
}

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




