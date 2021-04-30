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


// Runtime API for Oneshot futures
pub(crate) struct ChannelRt {
    // Actual implementation forwarded to inner struct with mutability. Perhaps the
    // UnsafeCell should be ok here since the API is private for the crate.
}

impl ChannelRt {
    pub(crate) fn new() -> Self {
        ChannelRt {
        }
    }

    pub(crate) fn create(&self) -> ChannelId {
        ChannelId(1)
    }

    pub(crate) fn awake_and_get_event_id(&self) -> Option<EventId> {
        None
    }
}


