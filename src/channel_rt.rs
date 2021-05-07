//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::cell::RefCell;
use std::task::Waker;

use crate::reactor::EventId;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

// Channel handle used by this low level channel API (which is only has crate visibility)
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct ChannelId(u32);

impl ChannelId {
    pub(crate) fn null() -> Self {
        ChannelId(0)
    }
}

// Runtime API for Channel futures
pub(crate) struct ChannelRt {
    // Actual implementation forwarded to inner struct with mutability. Perhaps the
    // UnsafeCell should be ok here since the API is private for the crate.
    inner: RefCell<InnerChannelRt>,
}

impl ChannelRt {
    pub(crate) fn new() -> Self {
        ChannelRt {
            inner: RefCell::new(InnerChannelRt::new()),
        }
    }

    pub(crate) fn create(&self) -> ChannelId {
        self.inner.borrow_mut().create()
    }

    pub(crate) fn awake_and_get_event_id(&self) -> Option<EventId> {
        self.inner.borrow_mut().awake_and_get_event_id()
    }

    pub(crate) fn add_sender(
        &self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        self.inner
            .borrow_mut()
            .add_sender(channel_id, waker, event_id, data);
    }

    pub(crate) fn reg_receiver(
        &self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        self.inner
            .borrow_mut()
            .reg_receiver(channel_id, waker, event_id, data);
    }

    pub(crate) unsafe fn exchange<T>(&self, channel_id: ChannelId) -> bool {
        self.inner.borrow_mut().exchange::<T>(channel_id)
    }

    pub(crate) fn cancel_sender(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().cancel_sender(channel_id);
    }

    pub(crate) fn cancel_receiver(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().cancel_receiver(channel_id);
    }
}

// Registration info provided for both sender and receiver.
#[derive(Debug, Clone)]
struct RegInfo {
    data: *mut (),
    waker: Waker,
    event_id: EventId,
}

enum PeerState {
    Created,
    Registered(RegInfo),
    Exchanged,
    Dropped,
}

struct ChannelNode {
    id: ChannelId,
    receiver: RegInfo,
    senders: Vec<RegInfo>,
}

struct InnerChannelRt {
    nodes: Vec<ChannelNode>,
}

impl InnerChannelRt {
    fn new() -> Self {
        InnerChannelRt { nodes: Vec::new() }
    }

    fn create(&mut self) -> ChannelId {
        ChannelId(1)
    }

    fn awake_and_get_event_id(&mut self) -> Option<EventId> {
        None
    }

    fn add_sender(
        &mut self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        todo!()
    }

    fn reg_receiver(
        &mut self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        todo!()
    }

    unsafe fn exchange<T>(&mut self, channel_id: ChannelId) -> bool {
        todo!()
    }

    fn cancel_sender(&mut self, channel_id: ChannelId) {
        todo!()
    }

    fn cancel_receiver(&mut self, channel_id: ChannelId) {
        todo!()
    }
}
