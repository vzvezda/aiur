//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
//
// This module is about oneshot channel support in Runtime. This is a pub(crate)
// visibility (with unsafe's), the exported API is in oneshot.rs module (which is safe).
//
// The oneshot runtime support is quite simple: both sender and receiver registers the
// pointer to the data for exchange and their Waker and EventId:
//
// (Sender<*mut(), EventId, Waker), Receiver<*mut, EventId, Waker>)
//
// As soon as both ends has their data registered, runtime wakes the Receiver, then it wakes
// the Sender.
use std::cell::RefCell;
use std::task::Waker;

use crate::reactor::EventId;

const MODTRACE: bool = true;

// Channel handle used by this low level channel API (which is only has crate visibility)
#[derive(Copy, Clone)]
pub(crate) struct ChannelId(u32);

impl ChannelId {
    pub(crate) fn null() -> Self {
        ChannelId(0)
    }
}

// Registration info provided for both sender and receiver.
#[derive(Debug)]
struct RegInfo {
    data: *mut (),
    waker: Waker,
    event_id: EventId,
}

impl RegInfo {
    fn new(data: *mut (), waker: Waker, event_id: EventId) -> Self {
        RegInfo {
            data,
            waker,
            event_id,
        }
    }
}

// Stage of linking the receiver and sender ends.
enum Linking {
    Created,
    Registered(RegInfo),
    Exchanged,
    Dropped,
}

// Debug 
impl std::fmt::Debug for Linking {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Linking::Created => f.write_str("Created"),
            Linking::Registered(..)=> f.write_str("Registered"),
            Linking::Exchanged => f.write_str("Exchanged"),
            Linking::Dropped => f.write_str("Dropped"),
        }
    }
}

struct OneshotNode {
    sender: Linking,
    receiver: Linking,
    // we need just one more bit for our state machine, see state machine diagram below
    recv_exchanged: bool,
}

impl OneshotNode {
    fn new() -> Self {
        Self {
            sender: Linking::Created,
            receiver: Linking::Created,
            recv_exchanged: false,
        }
    }
}

pub(crate) struct ChannelApi {
    // TODO: support many channels
    node: RefCell<OneshotNode>,
}

impl ChannelApi {
    pub(crate) fn new() -> Self {
        ChannelApi {
            node: RefCell::new(OneshotNode::new()),
        }
    }

    pub(crate) fn create(&self) -> ChannelId {
        // TODO: support many channels
        ChannelId(1)
    }

    pub(crate) fn reg_sender(
        &self,
        _channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        let reg_info = RegInfo::new(data, waker, event_id);
        modtrace!("ChannelApi: sender registration: {:?}", reg_info);
        self.node.borrow_mut().sender = Linking::Registered(reg_info);
    }

    pub(crate) fn reg_receiver(
        &self,
        _channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        let reg_info = RegInfo::new(data, waker, event_id);
        modtrace!("ChannelApi: receiver registration: {:?}", reg_info);
        self.node.borrow_mut().receiver = Linking::Registered(reg_info);
    }

    /*
     *  Ok, here is the oneshot channel state machine:
     *
     *     +--->(D,D)<---+           +---->(D,D)<----+
     *     |      ^      |           |       ^       |
     *     +      +      +           +       +       +
     *   (D,E)<+(D,R}<+(D,C)<--+-->(C,D)+->{R,D)+->(E,D)
     *     ^            ^ ^    |    ^ ^              ^
     *     |            | |    +    | |              |
     *     |            +---+(C,C)+---+              |
     *     |              +   + +   +                |
     *     |            (R,C)<+ +>(C,R)              |
     *     |              +         +                |
     *     |              +->(R,R}<-+                |
     *     |                   |                     |
     *     |                 {R,E)+->{R,D*)+--------->
     *     |                   |                     |
     *     |                 (E,E)                   |
     *     |                   +                     |
     *     |                   |                     |
     *     +-------------------+---------------------+
     *
     *     The state machine is sender/receiver:
     *        * C: Created
     *        * R: Registered
     *        * E: Exchanged
     *        * D: Dropped
     *
     *   Everything starts from (C,C) and in (D,D) channel resource are released. (D,D) has
     *   two instances on the diagram just for clarity.
     *
     *   get_event_id():
     *      * returns None when state described in parentheses, for example (C,C)
     *      * the curly brace means sender or receiver should be awoken (R,R} - awake
     *        the receiver side. In response to the awake, the channel future is expected 
     *        to invoke exchange(), but future drop also possible.
     *      * D* is a special state indicates that transfer was succesful (e.g. receiver was
     *        dropped just after it had value received).
     */

    pub(crate) fn get_event_id(&self) -> Option<EventId> {
        let node = self.node.borrow();

        // nobody to awake when there is a channel side in "Created" state
        if matches!(node.sender, Linking::Created) || matches!(node.receiver, Linking::Created) {
            return None;
        }

        // first awake the receiver. sender cannot be in Created state, other state like
        // Registered or Dropped are ok.
        match &node.receiver {
            Linking::Registered(ref rx_reg_info) => {
                rx_reg_info.waker.wake_by_ref();
                return Some(rx_reg_info.event_id);
            }
            _ => (),
        }

        // Awake the sender, the receiver cannnot be in Created state, but other states like
        // Exhanged or Dropped are ok.
        match &node.sender {
            Linking::Registered(ref tx_reg_info) => {
                tx_reg_info.waker.wake_by_ref();
                return Some(tx_reg_info.event_id);
            }
            _ => (),
        }

        // Some of the pairs of states are impossible, for example it is not possible
        // to have a channel side exchanged while another end is not yet registered.
        debug_assert!(
            match (&node.sender, &node.receiver) {
                (Linking::Created, Linking::Exchanged)
                | (Linking::Exchanged, Linking::Created)
                | (Linking::Exchanged, Linking::Registered(..)) => false,
                _ => true,
            },
            concat!(
                "aiur: oneshot::get_event_id() invoked in unexpected state. ",
                "Sender: {:?}, receiver: {:?}"
            ),
            node.sender,
            node.receiver,
        );

        // Othere states like (Exchanged, Exchanged) or (Dropped, Exchanged) are possible
        // and not produce any events, so None is returned.
        return None;
    }

    unsafe fn exhange_impl<T>(tx_data: *mut (), rx_data: *mut ()) {
        let mut tx_data = std::mem::transmute::<*mut (), *mut Option<T>>(tx_data);
        let mut rx_data = std::mem::transmute::<*mut (), *mut Option<T>>(rx_data);
        std::mem::swap(&mut *tx_data, &mut *rx_data);

        modtrace!("ChannelApi: exchange<T> just happened");
    }

    pub(crate) unsafe fn exchange<T>(&self, channel_id: ChannelId) -> bool {
        let mut node = self.node.borrow_mut();

        modtrace!("Exchange {:?},{:?}", node.sender, node.receiver);

        match (&node.sender, &node.receiver) {
            (Linking::Registered(..), Linking::Exchanged) => {
                node.sender = Linking::Exchanged;
                return true;
            }
            (Linking::Registered(..), Linking::Dropped) => {
                node.sender = Linking::Exchanged;
                // Receiver can be dropped after exchange happened
                return node.recv_exchanged;
            }
            (Linking::Dropped, Linking::Registered(..)) => {
                node.receiver = Linking::Exchanged;
                return false;
            }
            (Linking::Registered(ref tx), Linking::Registered(ref rx)) => {
                Self::exhange_impl::<T>(tx.data, rx.data);
                node.receiver = Linking::Exchanged;
                node.recv_exchanged = true;
                return true;
            }
            _ =>
            // some kind of bug in the code, the Oneshot futures should not make
            // this happen.
            {
                panic!(
                    concat!(
                        "aiur: oneshot::exhange() invoked in unexpected state. ",
                        "Sender: {:?}, receiver: {:?}"
                    ),
                    node.sender, node.receiver
                )
            }
        }
    }

    pub(crate) fn cancel_sender(&self, _channel_id: ChannelId) {
        modtrace!("ChannelApi: drop sender");
        let mut node = self.node.borrow_mut();
        node.sender = Linking::Dropped;
    }

    pub(crate) fn cancel_receiver(&self, _channel_id: ChannelId) {
        modtrace!("ChannelApi: drop receiver");
        let mut node = self.node.borrow_mut();
        node.receiver = Linking::Dropped;
    }
}
