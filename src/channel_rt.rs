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

    pub(crate) fn add_sender_fut(
        &self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        self.inner
            .borrow_mut()
            .add_sender_fut(channel_id, waker, event_id, data);
    }

    pub(crate) fn reg_receiver_fut(
        &self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        self.inner
            .borrow_mut()
            .reg_receiver_fut(channel_id, waker, event_id, data);
    }

    pub(crate) unsafe fn exchange<T>(&self, channel_id: ChannelId) -> bool {
        self.inner.borrow_mut().exchange::<T>(channel_id)
    }

    pub(crate) fn inc_sender(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().inc_sender(channel_id);
    }

    pub(crate) fn dec_sender(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().dec_sender(channel_id);
    }

    pub(crate) fn dec_receiver(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().dec_receiver(channel_id);
    }

    pub(crate) fn cancel_sender_fut(&self, channel_id: ChannelId, event_id: EventId) {
        self.inner.borrow_mut().cancel_sender_fut(channel_id, event_id);
    }

    pub(crate) fn cancel_receiver_fut(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().cancel_receiver_fut(channel_id);
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

// This is a channel
struct ChannelNode {
    id: ChannelId,
    senders_alive: u32,
    recv_is_alive: bool,
    recv_future: PeerState,
    send_future: PeerState,
    recv_exchanged: bool,
    send_queue: Vec<RegInfo>,
}

impl ChannelNode {
    fn new(channel_id: ChannelId) -> Self {
        Self {
            id: channel_id,
            senders_alive: 0, // incremented by ChSender::new()
            recv_is_alive: true,
            recv_future: PeerState::Created,
            send_future: PeerState::Created,
            recv_exchanged: false,
            send_queue: Vec::new(), 
        }
    }

    fn add_sender_future(&mut self, reg_info: RegInfo) {
        if matches!(self.send_future, PeerState::Created) {
            self.send_future = PeerState::Registered(reg_info);
        }
        else {
            self.send_queue.push(reg_info);
        }
    }

    fn reg_recv_future(&mut self, reg_info: RegInfo) {
        self.recv_future = PeerState::Registered(reg_info);
    }

    fn inc_sender(&mut self) {
        self.senders_alive += 1;
        modtrace!("ChannelRt: {:?} inc senders to {}", self.id, self.senders_alive);
    }

    fn dec_sender(&mut self) {
        self.senders_alive -= 1;
        modtrace!("ChannelRt: {:?} dec senders to {}", self.id, self.senders_alive);
    }

    fn dec_receiver(&mut self) {
        self.recv_is_alive = false;
        modtrace!("ChannelRt: {:?} receiver has been dropped", self.id);
    }

    fn is_channel_alive(&self) -> bool {
        self.senders_alive > 0 || self.recv_is_alive
    }

}

// Produce a state like "(C,R}". See the state machine chart in code below for meaning.
impl std::fmt::Debug for ChannelNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.send_future {
            PeerState::Created => f.write_str("(C,"),
            PeerState::Registered(..) => {
                if matches!(self.recv_future, PeerState::Registered(..)) {
                    f.write_str("(R,")
                } else if matches!(self.recv_future, PeerState::Created) {
                    f.write_str("(R,")
                } else {
                    f.write_str("{R,")
                }
            }
            PeerState::Exchanged => f.write_str("(E,"),
            PeerState::Dropped => f.write_str("(D,"),
        }?;

        match self.recv_future {
            PeerState::Created => f.write_str("C)"),
            PeerState::Registered(..) => {
                if matches!(self.send_future, PeerState::Created) {
                    f.write_str("R)")
                } else {
                    f.write_str("R}")
                }
            }
            PeerState::Exchanged => f.write_str("E)"),
            PeerState::Dropped => {
                if self.recv_exchanged && matches!(self.send_future, PeerState::Registered(..)) {
                    f.write_str("D*)")
                } else {
                    f.write_str("D)")
                }
            }
        }
    }
}


struct InnerChannelRt {
    node: Option<ChannelNode>, // TODO: many channels
}

impl InnerChannelRt {
    fn new() -> Self {
        InnerChannelRt { node: None }
    }

    fn create(&mut self) -> ChannelId {
        assert!(self.node.is_none());
        let id = ChannelId(1);
        self.node = Some(ChannelNode::new(id));
        modtrace!("ChannelRt: new channel has been created: {:?}", id);
        id
    }

    fn get_node_mut(&mut self, channel_id: ChannelId) -> &mut ChannelNode {
        self.node.as_mut().unwrap()
    }
    fn get_node(&mut self, channel_id: ChannelId) -> &ChannelNode {
        self.node.as_ref().unwrap()
    }

    fn add_sender_fut(
        &mut self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        let reg_info = RegInfo::new(data, waker, event_id);
        self.get_node_mut(channel_id).add_sender_future(reg_info);
    }

    fn reg_receiver_fut(
        &mut self,
        channel_id: ChannelId,
        waker: Waker,
        event_id: EventId,
        data: *mut (),
    ) {
        let reg_info = RegInfo::new(data, waker, event_id);
        self.get_node_mut(channel_id).reg_recv_future(reg_info);
    }

    unsafe fn exhange_impl<T>(tx_data: *mut (), rx_data: *mut ()) {
        let tx_data = std::mem::transmute::<*mut (), *mut Option<T>>(tx_data);
        let rx_data = std::mem::transmute::<*mut (), *mut Option<T>>(rx_data);
        std::mem::swap(&mut *tx_data, &mut *rx_data);

        modtrace!("OneshotRt: exchange<T> mem::swap() just happened");
    }

    unsafe fn exchange<T>(&mut self, channel_id: ChannelId) -> bool {
        let node_mut = self.get_node_mut(channel_id);


        todo!()
    }

    fn awake_and_get_event_id(&mut self) -> Option<EventId> {
        if self.node.is_none() {
            return None;
        }


        None
    }



    fn inc_sender(&mut self, channel_id: ChannelId) {
        self.get_node_mut(channel_id).inc_sender();
    }

    fn dec_sender(&mut self, channel_id: ChannelId) {
        self.get_node_mut(channel_id).dec_sender();
        self.drop_channel_if_needed(channel_id);
    }

    fn dec_receiver(&mut self, channel_id: ChannelId) {
        self.get_node_mut(channel_id).dec_receiver();
        self.drop_channel_if_needed(channel_id);
    }

    fn drop_channel_if_needed(&mut self, channel_id: ChannelId) {
        if !self.get_node(channel_id).is_channel_alive() {
            self.node = None;
            modtrace!("ChannelRt: channel {:?} has been closed", channel_id);
        }
    }

    fn cancel_sender_fut(&mut self, channel_id: ChannelId, event_id: EventId) {
        todo!()
    }

    fn cancel_receiver_fut(&mut self, channel_id: ChannelId) {
        todo!()
    }
}
