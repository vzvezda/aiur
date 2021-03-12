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
// As soon as both channel sides has their data registered, runtime wakes the
// Receiver to get the data, then it wakes the Sender.
use std::cell::RefCell;
use std::task::Waker;

use crate::reactor::EventId;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

// Channel handle used by this low level channel API (which is only has crate visibility)
#[derive(Debug, Copy, Clone)]
pub(crate) struct ChannelId(u32);

impl ChannelId {
    pub(crate) fn null() -> Self {
        ChannelId(0)
    }
}

// Registration info provided for both sender and receiver.
#[derive(Debug, Clone)]
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
#[derive(Clone)] // Cloning the Waker in aiur is cheap
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
            Linking::Registered(..) => f.write_str("Registered"),
            Linking::Exchanged => f.write_str("Exchanged"),
            Linking::Dropped => f.write_str("Dropped"),
        }
    }
}

#[derive(Clone)]
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

// See the state machine chart in code below
impl std::fmt::Debug for OneshotNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.sender {
            Linking::Created => f.write_str("(C,"),
            Linking::Registered(..) => {
                if matches!(self.receiver, Linking::Registered(..)) {
                    f.write_str("(R,")
                } else {
                    f.write_str("{R,")
                }
            }
            Linking::Exchanged => f.write_str("(E,"),
            Linking::Dropped => f.write_str("(D,"),
        }?;

        match self.receiver {
            Linking::Created => f.write_str("C)"),
            Linking::Registered(..) => f.write_str("R}"),
            Linking::Exchanged => f.write_str("E)"),
            Linking::Dropped => {
                if self.recv_exchanged && matches!(self.sender, Linking::Registered(..)) {
                    f.write_str("D*)")
                } else {
                    f.write_str("D)")
                }
            }
        }
    }
}

// Runtime API for Oneshot futures
pub(crate) struct OneshotRt {
    // Actual implementation forwarded to inner struct with mutability. Perhaps the
    // UnsafeCell should be ok here since the API is private for the crate.
    inner: RefCell<InnerOneshotRt>,
}

impl OneshotRt {
    pub(crate) fn new() -> Self {
        OneshotRt {
            inner: RefCell::new(InnerOneshotRt::new()),
        }
    }

    pub(crate) fn create(&self) -> ChannelId {
        self.inner.borrow_mut().create()
    }

    pub(crate) fn reg_sender(
        &self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        self.inner
            .borrow_mut()
            .reg_sender(channel_id, waker, event_id, data);
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

    pub(crate) fn get_event_id(&self) -> Option<EventId> {
        self.inner.borrow().get_event_id()
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

struct InnerOneshotRt {
    node: OneshotNode, // TODO: support many channels
}

impl InnerOneshotRt {
    fn new() -> Self {
        InnerOneshotRt {
            node: OneshotNode::new(),
        }
    }

    fn set_sender(&mut self, channel_id: ChannelId, sender: Linking, log_context: &str) {
        let old = self.node.clone();

        self.node = OneshotNode {
            sender: sender,
            receiver: old.receiver.clone(),
            recv_exchanged: old.recv_exchanged,
        };

        modtrace!(
            "OneshotRt: {:?} state {:?} -> {:?} ({})",
            channel_id,
            old,
            self.node,
            log_context
        );
    }

    fn set_receiver(&mut self, channel_id: ChannelId, receiver: Linking, log_context: &str) {
        let old = self.node.clone();
        self.node = OneshotNode {
            sender: old.sender.clone(),
            receiver: receiver,
            recv_exchanged: old.recv_exchanged,
        };

        modtrace!(
            "OneshotRt: {:?} state {:?} -> {:?} ({})",
            channel_id,
            old,
            self.node,
            log_context
        );
    }

    fn set_receiver_ext(
        &mut self,
        channel_id: ChannelId,
        receiver: Linking,
        recv_exchanged: bool,
        log_context: &str,
    ) {
        let old = self.node.clone();
        self.node = OneshotNode {
            sender: old.sender.clone(),
            receiver: receiver,
            recv_exchanged: recv_exchanged,
        };
        modtrace!(
            "OneshotRt: {:?} state {:?} -> {:?} ({})",
            channel_id,
            old,
            self.node,
            log_context
        );
    }

    fn create(&mut self) -> ChannelId {
        ChannelId(1) // TODO: support many channels
    }

    fn reg_sender(
        &mut self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        let reg_info = RegInfo::new(data, waker, event_id);
        self.set_sender(channel_id, Linking::Registered(reg_info), "by reg_sender()");
    }

    fn reg_receiver(
        &mut self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        let reg_info = RegInfo::new(data, waker, event_id);
        self.set_receiver(channel_id, Linking::Registered(reg_info), "by reg_receiver()");
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
     *     |                 {R,E)+->{R,D*)+---------^
     *     |                   |                     |
     *     |                 (E,E)                   |
     *     |                   +                     |
     *     |                   |                     |
     *     +-------------------+---------------------+
     *
     *     The state machine is (sender,receiver):
     *        * C: Created
     *        * R: Registered
     *        * E: Exchanged
     *        * D: Dropped
     *
     *   Everything starts from (C,C) and in (D,D) all channel resources are released. (D,D) has
     *   two instances on the diagram above for clarity, but this is the same state.
     *
     *   get_event_id() returns event for the Runtime:
     *      * returns None when state is described in parentheses, for example (C,C)
     *      * the curly brace means sender or receiver should be awoken (R,R} - awake
     *        the receiver side. In response to the awake, the channel future is expected
     *        to invoke exchange(), but drop also possible and expected.
     *      * "D*" is a special state indicates that transfer was succesful: receiver was
     *        dropped just after it had value received, so we should signal success to
     *        sender.
     */

    fn get_event_id(&self) -> Option<EventId> {
        // nobody to awake when there is a channel side in "Created" state
        if matches!(self.node.sender, Linking::Created) {
            return None;
        }
        if matches!(self.node.receiver, Linking::Created) {
            return None;
        }

        // first awake the receiver. sender cannot be in Created state, other state like
        // Registered or Dropped are ok.
        match &self.node.receiver {
            Linking::Registered(ref rx_reg_info) => {
                rx_reg_info.waker.wake_by_ref();
                return Some(rx_reg_info.event_id);
            }
            _ => (),
        }

        // Awake the sender, the receiver cannnot be in Created state, but other states like
        // Exhanged or Dropped are ok.
        match &self.node.sender {
            Linking::Registered(ref tx_reg_info) => {
                tx_reg_info.waker.wake_by_ref();
                return Some(tx_reg_info.event_id);
            }
            _ => (),
        }

        // Some of the pairs of states are impossible, for example it is not possible
        // to have a channel side exchanged while another end is not yet registered.
        debug_assert!(
            match (&self.node.sender, &self.node.receiver) {
                (Linking::Created, Linking::Exchanged)
                | (Linking::Exchanged, Linking::Created)
                | (Linking::Exchanged, Linking::Registered(..)) => false,
                _ => true,
            },
            concat!(
                "aiur: oneshot::get_event_id() invoked in unexpected state. ",
                "Sender: {:?}, receiver: {:?}"
            ),
            self.node.sender,
            self.node.receiver,
        );

        // Othere states like (Exchanged, Exchanged) or (Dropped, Exchanged) are possible
        // and not produce any events, so None is returned.
        return None;
    }

    unsafe fn exhange_impl<T>(tx_data: *mut (), rx_data: *mut ()) {
        let mut tx_data = std::mem::transmute::<*mut (), *mut Option<T>>(tx_data);
        let mut rx_data = std::mem::transmute::<*mut (), *mut Option<T>>(rx_data);
        std::mem::swap(&mut *tx_data, &mut *rx_data);

        modtrace!("OneshotRt: exchange<T> mem::swap() just happened");
    }

    pub(crate) unsafe fn exchange<T>(&mut self, channel_id: ChannelId) -> bool {
        match (&self.node.sender, &self.node.receiver) {
            (Linking::Registered(..), Linking::Exchanged) => {
                self.set_sender(channel_id, Linking::Exchanged, "exchange()");
                return true;
            }
            (Linking::Registered(..), Linking::Dropped) => {
                self.set_sender(channel_id, Linking::Exchanged, "exchange()");
                // Receiver can be dropped after exchange happened
                return self.node.recv_exchanged;
            }
            (Linking::Dropped, Linking::Registered(..)) => {
                self.set_receiver(channel_id, Linking::Exchanged, "exchange()");
                return false;
            }
            (Linking::Registered(ref tx), Linking::Registered(ref rx)) => {
                Self::exhange_impl::<T>(tx.data, rx.data);
                self.set_receiver_ext(channel_id, Linking::Exchanged, true, "exchange()");
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
                    self.node.sender, self.node.receiver
                )
            }
        }
    }

    pub(crate) fn cancel_sender(&mut self, channel_id: ChannelId) {
        modtrace!("OneshotRt: drop sender");
        self.set_sender(channel_id, Linking::Dropped, "sender cancelled");
    }

    pub(crate) fn cancel_receiver(&mut self, channel_id: ChannelId) {
        self.set_receiver(channel_id, Linking::Dropped, "receiver cancelled");
    }
}
