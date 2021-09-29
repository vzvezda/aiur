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

use crate::tracer::Tracer;
use crate::reactor::EventId;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

// Channel handle used by this low level channel API (which is only has crate visibility)
#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) struct OneshotId(u32);

impl std::fmt::Debug for OneshotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("oneshot:{}", self.0))
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

// Where is a sender or  receiver in the communication phase
#[derive(Clone)] // Cloning the Waker in aiur does not involve allocation
enum PeerState {
    Created,
    Registered(RegInfo),
    Exchanged,
    Dropped,
}

// Debug
impl std::fmt::Debug for PeerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerState::Created => f.write_str("Created"),
            PeerState::Registered(..) => f.write_str("Registered"),
            PeerState::Exchanged => f.write_str("Exchanged"),
            PeerState::Dropped => f.write_str("Dropped"),
        }
    }
}

#[derive(Clone)]
struct OneshotNode {
    id: OneshotId,
    sender: PeerState,
    receiver: PeerState,
    // we need just one more bit for our state machine, see state machine diagram below
    recv_exchanged: bool,
}

impl OneshotNode {
    fn new(id: u32) -> Self {
        Self {
            id: OneshotId(id),
            sender: PeerState::Created,
            receiver: PeerState::Created,
            recv_exchanged: false,
        }
    }

    // If both ends of the channel are dropped and channel can be removed from runtime.
    fn can_be_dropped(&self) -> bool {
        match (&self.receiver, &self.sender) {
            (PeerState::Dropped, PeerState::Dropped) => true,
            _ => false,
        }
    }
}

// Produce a state like "(C->R}". See the state machine chart in code below for meaning.
impl std::fmt::Debug for OneshotNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.sender {
            PeerState::Created => f.write_str("(C->"),
            PeerState::Registered(..) => {
                if matches!(self.receiver, PeerState::Registered(..)) {
                    f.write_str("(R->")
                } else if matches!(self.receiver, PeerState::Created) {
                    f.write_str("(R->")
                } else {
                    f.write_str("{R->")
                }
            }
            PeerState::Exchanged => f.write_str("(E->"),
            PeerState::Dropped => f.write_str("(D->"),
        }?;

        match self.receiver {
            PeerState::Created => f.write_str("C)"),
            PeerState::Registered(..) => {
                if matches!(self.sender, PeerState::Created) {
                    f.write_str("R)")
                } else {
                    f.write_str("R}")
                }
            }
            PeerState::Exchanged => f.write_str("E)"),
            PeerState::Dropped => {
                if self.recv_exchanged && matches!(self.sender, PeerState::Registered(..)) {
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
    pub(crate) fn new(tracer: &Tracer) -> Self {
        OneshotRt {
            inner: RefCell::new(InnerOneshotRt::new(tracer)),
        }
    }

    pub(crate) fn create(&self) -> OneshotId {
        self.inner.borrow_mut().create()
    }

    pub(crate) fn reg_sender(
        &self,
        oneshot_id: OneshotId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        self.inner
            .borrow_mut()
            .reg_sender(oneshot_id, waker, event_id, data);
    }

    pub(crate) fn reg_receiver(
        &self,
        oneshot_id: OneshotId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        self.inner
            .borrow_mut()
            .reg_receiver(oneshot_id, waker, event_id, data);
    }

    pub(crate) fn awake_and_get_event_id(&self) -> Option<EventId> {
        self.inner.borrow().awake_and_get_event_id()
    }

    pub(crate) unsafe fn exchange<T>(&self, oneshot_id: OneshotId) -> bool {
        self.inner.borrow_mut().exchange::<T>(oneshot_id)
    }

    pub(crate) fn cancel_sender(&self, oneshot_id: OneshotId) {
        self.inner.borrow_mut().cancel_sender(oneshot_id);
    }

    pub(crate) fn cancel_receiver(&self, oneshot_id: OneshotId) {
        self.inner.borrow_mut().cancel_receiver(oneshot_id);
    }
}

struct InnerOneshotRt {
    nodes: Vec<OneshotNode>,
    last_id: u32,
    tracer: Tracer,
}

impl InnerOneshotRt {
    fn new(tracer: &Tracer) -> Self {
        InnerOneshotRt {
            nodes: Vec::new(),
            last_id: 0,
            tracer: tracer.clone(),
        }
    }

    // find the index in self.nodes() for given oneshot_id. Panics if not found.
    fn find_index(&self, oneshot_id: OneshotId) -> usize {
        self.nodes
            .iter()
            .position(|node| node.id == oneshot_id)
            .unwrap()
    }

    // Changes the state of the sender. Panics if channel_id is not found.
    fn set_sender(&mut self, oneshot_id: OneshotId, sender: PeerState, log_context: &str) {
        // TODO: too many index access
        let idx = self.find_index(oneshot_id);
        let old = self.nodes[idx].clone();

        self.nodes[idx] = OneshotNode {
            id: old.id,
            sender: sender,
            receiver: old.receiver.clone(),
            recv_exchanged: old.recv_exchanged,
        };

        modtrace!(
            self.tracer,
            "oneshot_rt: {:?} state {:?} -> {:?} ({})",
            oneshot_id,
            old,
            self.nodes[idx],
            log_context
        );

        if self.nodes[idx].can_be_dropped() {
            modtrace!(
                self.tracer,
                "oneshot_rt: remove {:?} from idx {}",
                oneshot_id,
                idx
            );
            self.nodes.remove(idx);
        }
    }

    fn set_receiver(&mut self, oneshot_id: OneshotId, receiver: PeerState, log_context: &str) {
        let idx = self.find_index(oneshot_id);
        let old = self.nodes[idx].clone();
        self.nodes[idx] = OneshotNode {
            id: old.id,
            sender: old.sender.clone(),
            receiver: receiver,
            recv_exchanged: old.recv_exchanged,
        };

        modtrace!(
            self.tracer,
            "oneshot_rt: {:?} state {:?} -> {:?} ({})",
            oneshot_id,
            old,
            self.nodes[idx],
            log_context
        );

        if self.nodes[idx].can_be_dropped() {
            self.nodes.remove(idx);
            modtrace!(
                self.tracer,
                "oneshot_rt: remove {:?} from idx {}",
                oneshot_id,
                idx
            );
        }
    }

    fn set_receiver_ext(
        &mut self,
        oneshot_id: OneshotId,
        receiver: PeerState,
        recv_exchanged: bool,
        log_context: &str,
    ) {
        let idx = self.find_index(oneshot_id);
        let old = self.nodes[idx].clone();
        self.nodes[idx] = OneshotNode {
            id: old.id,
            sender: old.sender.clone(),
            receiver: receiver,
            recv_exchanged: recv_exchanged,
        };
        modtrace!(
            self.tracer,
            "oneshot_rt: {:?} state {:?} -> {:?} ({})",
            oneshot_id,
            old,
            self.nodes[idx],
            log_context
        );
    }

    fn create(&mut self) -> OneshotId {
        self.last_id = self.last_id.wrapping_add(1);
        self.nodes.push(OneshotNode::new(self.last_id));
        OneshotId(self.last_id)
    }

    fn reg_sender(
        &mut self,
        oneshot_id: OneshotId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        let reg_info = RegInfo::new(data, waker, event_id);
        self.set_sender(
            oneshot_id,
            PeerState::Registered(reg_info),
            "by reg_sender()",
        );
    }

    fn reg_receiver(
        &mut self,
        oneshot_id: OneshotId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        let reg_info = RegInfo::new(data, waker, event_id);
        self.set_receiver(
            oneshot_id,
            PeerState::Registered(reg_info),
            "by reg_receiver()",
        );
    }

    // Scans all nodes and if there is a oneshot that ready to await, it makes waker.wake_by_ref()
    // and returns the event_id.
    fn awake_and_get_event_id(&self) -> Option<EventId> {
        self.nodes
            .iter()
            .find_map(|node| Self::get_event_id_for_node(&node))
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
     *     |              |   + +   |                |
     *     |              +   | |   +                |
     *     |            (R,C)<+ +>(C,R)              |
     *     |              +         +                |
     *     |              +->(R,R}<-+                |
     *     |                   |                     |
     *     ^-----------------{R,E)+->{R,D*)+---------^
     *     |                   |       |             |
     *     |                 (E,E)     v             |
     *     |                   +     (D,D)           |
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
     *   awake_and_get_event_id() returns event for the Runtime:
     *      * returns None when state is described in parentheses, for example (C,C)
     *      * the curly brace means sender or receiver should be awoken (R,R} - awake
     *        the receiver side. In response to the awake, the channel future is expected
     *        to invoke exchange(), but drop also possible and expected.
     *      * "D*" is a special state indicates that transfer was succesful: receiver was
     *        dropped just after it had value received, so we should signal success to
     *        sender.
     */

    fn get_event_id_for_node(node: &OneshotNode) -> Option<EventId> {
        // nobody to awake when there is a channel side in "Created" state
        if matches!(node.sender, PeerState::Created) {
            return None;
        }
        if matches!(node.receiver, PeerState::Created) {
            return None;
        }

        // first awake the receiver. sender cannot be in Created state, other state like
        // Registered or Dropped are ok.
        match &node.receiver {
            PeerState::Registered(ref rx_reg_info) => {
                rx_reg_info.waker.wake_by_ref();
                return Some(rx_reg_info.event_id);
            }
            _ => (),
        }

        // Awake the sender, the receiver cannnot be in Created state, but other states like
        // Exhanged or Dropped are ok.
        match &node.sender {
            PeerState::Registered(ref tx_reg_info) => {
                tx_reg_info.waker.wake_by_ref();
                return Some(tx_reg_info.event_id);
            }
            _ => (),
        }

        // Some of the pairs of states are impossible, for example it is not possible
        // to have a channel side exchanged while another end is not yet registered.
        debug_assert!(
            match (&node.sender, &node.receiver) {
                (PeerState::Created, PeerState::Exchanged)
                | (PeerState::Exchanged, PeerState::Created)
                | (PeerState::Exchanged, PeerState::Registered(..)) => false,
                _ => true,
            },
            concat!(
                "aiur: oneshot::awake_and_get_event_id() invoked in unexpected state. ",
                "Sender: {:?}, receiver: {:?}"
            ),
            node.sender,
            node.receiver,
        );

        // Othere states like (Exchanged, Exchanged) or (Dropped, Exchanged) are possible
        // and not produce any events, so None is returned.
        return None;
    }

    unsafe fn exhange_impl<T>(tx_data: *mut (), rx_data: *mut (), tracer: &Tracer) {
        let tx_data = std::mem::transmute::<*mut (), *mut Option<T>>(tx_data);
        let rx_data = std::mem::transmute::<*mut (), *mut Option<T>>(rx_data);
        std::mem::swap(&mut *tx_data, &mut *rx_data);
        modtrace!(tracer, "oneshot_rt: exchange<T> mem::swap() just happened");
    }

    pub(crate) unsafe fn exchange<T>(&mut self, oneshot_id: OneshotId) -> bool {
        let node = self.nodes[self.find_index(oneshot_id)].clone();
        match (&node.sender, &node.receiver) {
            (PeerState::Registered(..), PeerState::Exchanged) => {
                self.set_sender(oneshot_id, PeerState::Exchanged, "by exchange()");
                return true;
            }
            (PeerState::Registered(..), PeerState::Dropped) => {
                self.set_sender(oneshot_id, PeerState::Exchanged, "by exchange()");
                // Receiver can be dropped after exchange happened
                return node.recv_exchanged;
            }
            (PeerState::Dropped, PeerState::Registered(..)) => {
                self.set_receiver(oneshot_id, PeerState::Exchanged, "by exchange()");
                return false;
            }
            (PeerState::Registered(ref tx), PeerState::Registered(ref rx)) => {
                Self::exhange_impl::<T>(tx.data, rx.data, &self.tracer);
                self.set_receiver_ext(oneshot_id, PeerState::Exchanged, true, "by exchange()");
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

    pub(crate) fn cancel_sender(&mut self, oneshot_id: OneshotId) {
        self.set_sender(oneshot_id, PeerState::Dropped, "by cancel_sender()");
    }

    pub(crate) fn cancel_receiver(&mut self, oneshot_id: OneshotId) {
        self.set_receiver(oneshot_id, PeerState::Dropped, "by cancel_receiver()");
    }
}
