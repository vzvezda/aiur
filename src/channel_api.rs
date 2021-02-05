//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::cell::RefCell;
use std::task::Waker;

use crate::reactor::EventId;

#[derive(Copy, Clone)]
pub(crate) struct ChannelId(u32);

impl ChannelId {
    pub(crate) fn null() -> Self {
        ChannelId(0)
    }
}

#[derive(Debug)]
struct ChannelEnd {
    data: *mut (),
    waker: Waker,
    event_id: EventId,
}

impl ChannelEnd {
    fn new(data: *mut (), waker: Waker, event_id: EventId) -> Self {
        ChannelEnd {
            data,
            waker,
            event_id,
        }
    }
}

struct ChannelNode {
    sender_alive: bool,
    recv_alive: bool,
    complete: Option<bool>,
    recv: Option<ChannelEnd>,
    send: Option<ChannelEnd>,
}

impl ChannelNode {
    fn new() -> Self {
        ChannelNode {
            sender_alive: true,
            recv_alive: true,
            complete: None,
            recv: None,
            send: None,
        }
    }
}

pub(crate) struct ChannelApi {
    node: RefCell<ChannelNode>,
}

impl ChannelApi {
    pub(crate) fn new() -> Self {
        ChannelApi {
            node: RefCell::new(ChannelNode::new()),
        }
    }

    pub(crate) fn create(&self) -> ChannelId {
        // todo
        ChannelId(1)
    }

    pub(crate) fn reg_sender(
        &self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        let ce = ChannelEnd::new(data, waker, event_id);
        println!("ChannelApi: sender registerd: {:?}", ce);
        self.node.borrow_mut().send = Some(ce);
    }

    pub(crate) fn reg_receiver(
        &self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        let ce = ChannelEnd::new(data, waker, event_id);
        println!("ChannelApi: receiver registerd: {:?}", ce);
        self.node.borrow_mut().recv = Some(ce);
    }

    pub(crate) fn get_event_id(&self) -> Option<EventId> {
        let mut node = self.node.borrow_mut();
        if node.send.is_none() || node.recv.is_none() {
            println!("ChannelApi: get_event_id -> None (not connected)");
            return None;
        }

        if node.complete.is_none() {
            // first time call
            node.recv.as_ref().unwrap().waker.wake_by_ref();
            let event_id = node.recv.as_ref().unwrap().event_id;
            println!("ChannelApi: get_event_id -> connected, poll receiver {:?}", event_id);
            return Some(event_id);
        }

        let event_id = node.send.as_ref().unwrap().event_id;
        node.send.as_ref().unwrap().waker.wake_by_ref();
        println!("ChannelApi: get_event_id -> connected, poll sender {:?}", event_id);

        node.send = None;
        node.recv = None;

        return Some(event_id);
    }

    pub(crate) unsafe fn exchange<T>(&self, channel_id: ChannelId) -> bool {
        if self.node.borrow().complete.is_some() {
            return self.node.borrow().complete.unwrap();
            println!("ChannelApi: exchange<T> completed");
        }

        let transfer_ok = self.node.borrow().send.is_some() && self.node.borrow().recv.is_some();

        self.node.borrow_mut().complete = Some(transfer_ok);
        // Exchange was unsucessful
        if !transfer_ok {
            println!("ChannelApi: exchange<T> unsuccesfull");
            return false;
        }

        let mut node = self.node.borrow_mut();
        let sender =
            std::mem::transmute::<*mut (), *mut Option<T>>(node.send.as_mut().unwrap().data);
        let receiver =
            std::mem::transmute::<*mut (), *mut Option<T>>(node.recv.as_mut().unwrap().data);
        std::mem::swap(&mut *sender, &mut *receiver);
        println!("ChannelApi: exchange<T> just happened");

        true
    }

    pub(crate) fn drop_sender(&self, _channel_id: ChannelId) {
        println!("ChannelApi: drop sender");
        // todo
    }

    pub(crate) fn drop_receiver(&self, _channel_id: ChannelId) {
        println!("ChannelApi: drop receiver");
        // todo
    }
}
