//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::cell::RefCell;
use std::task::Waker;

use crate::reactor::EventId;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

// Channel handle used by this low level channel API, which is only has crate visibility.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct ChannelId(u32);

impl ChannelId {
    pub(crate) fn null() -> Self {
        ChannelId(0)
    }
}

// The result of exchange<T> for send/receive future
pub(crate) enum ExchangeResult {
    Done,
    Disconnected,
    TryLater, // a new state in compare to oneshot
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

    pub(crate) unsafe fn exchange_sender<T>(&self, channel_id: ChannelId) -> ExchangeResult {
        self.inner.borrow_mut().exchange_sender::<T>(channel_id)
    }

    pub(crate) unsafe fn exchange_receiver<T>(&self, channel_id: ChannelId) -> ExchangeResult {
        self.inner.borrow_mut().exchange_receiver::<T>(channel_id)
    }

    pub(crate) fn inc_sender(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().inc_sender(channel_id);
    }

    pub(crate) fn dec_sender(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().dec_sender(channel_id);
    }

    pub(crate) fn close_receiver(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().close_receiver(channel_id);
    }

    pub(crate) fn cancel_sender_fut(&self, channel_id: ChannelId, event_id: EventId) {
        self.inner
            .borrow_mut()
            .cancel_sender_fut(channel_id, event_id);
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

// This is the state of the receiver
enum RxState {
    Created,
    Registered(RegInfo),
    Gone,
}

// This is a state for senders
enum TxState {
    Registered(RegInfo),
    Transmitted(RegInfo),
}

impl std::fmt::Debug for RxState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RxState::Created => f.write_str("Created"),
            RxState::Registered(..) => f.write_str("Registered"),
            RxState::Gone => f.write_str("Gone"),
        }
    }
}

impl std::fmt::Debug for TxState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxState::Registered(..) => f.write_str("Registered"),
            TxState::Transmitted(..) => f.write_str("Transmitted"),
        }
    }
}

// This is a channel object
struct ChannelNode {
    id: ChannelId,
    rx_state: RxState,
    tx_queue: Vec<TxState>,
    senders_alive: u32,
}

impl ChannelNode {
    fn new(channel_id: ChannelId) -> Self {
        Self {
            id: channel_id,
            rx_state: RxState::Created,
            tx_queue: Vec::new(),
            senders_alive: 0, // intially incremented by ChSender::new()
        }
    }

    fn add_sender_future(&mut self, reg_info: RegInfo) {
        self.tx_queue.push(TxState::Registered(reg_info));
    }

    fn reg_recv_future(&mut self, reg_info: RegInfo) {
        self.rx_state = RxState::Registered(reg_info);
    }

    fn inc_sender(&mut self) {
        self.senders_alive += 1;
        modtrace!(
            "ChannelRt: {:?} inc senders to {}",
            self.id,
            self.senders_alive
        );
    }

    fn dec_sender(&mut self) {
        self.senders_alive -= 1;
        modtrace!(
            "ChannelRt: {:?} dec senders to {}",
            self.id,
            self.senders_alive
        );
    }

    fn close_receiver(&mut self) {
        self.rx_state = RxState::Gone;
        modtrace!("ChannelRt: {:?} receiver channel been destroyed", self.id);
    }

    fn is_channel_alive(&self) -> bool {
        self.senders_alive > 0 || !matches!(self.rx_state, RxState::Gone)
    }
}

// Produce a state like "(C,R}". See the state machine chart in code below for meaning.
impl std::fmt::Debug for ChannelNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
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

        modtrace!("ChannelRt: exchange<T> mem::swap() just happened");
    }

    unsafe fn exchange_sender<T>(&mut self, channel_id: ChannelId) -> ExchangeResult {
        let node_mut = self.get_node_mut(channel_id);

        todo!()
    }

    unsafe fn exchange_receiver<T>(&mut self, channel_id: ChannelId) -> ExchangeResult {
        let node_mut = self.get_node_mut(channel_id);

        todo!()
    }

    fn get_event_id_for_node(node: &ChannelNode) -> Option<EventId> {

        if matches!(node.rx_state, RxState::Created) {
            return None;
        }

        if node.tx_queue.is_empty() && node.senders_alive > 0 {
            return None; // no sender futures
        }

        // first awake the receiver. sender cannot be in Created state, other state like
        // Registered or Dropped are ok.
        match &node.rx_state {
            RxState::Registered(ref rx_reg_info) => {
                rx_reg_info.waker.wake_by_ref();
                return Some(rx_reg_info.event_id);
            }
            _ => (),
        }

        // Awake the sender, the receiver cannnot be in Created state, but other states like
        // Exhanged or Dropped are ok.
        match &node.tx_queue[0] {
            TxState::Registered(ref tx_reg_info) => {
                tx_reg_info.waker.wake_by_ref();
                return Some(tx_reg_info.event_id);
            }
            _ => (),
        }

        None
    }

    fn awake_and_get_event_id(&mut self) -> Option<EventId> {
        if self.node.is_none() {
            return None;
        }

        Self::get_event_id_for_node(self.get_node_mut(ChannelId(1)))
    }

    fn inc_sender(&mut self, channel_id: ChannelId) {
        self.get_node_mut(channel_id).inc_sender();
    }

    fn dec_sender(&mut self, channel_id: ChannelId) {
        self.get_node_mut(channel_id).dec_sender();
        self.drop_channel_if_needed(channel_id);
    }

    fn close_receiver(&mut self, channel_id: ChannelId) {
        self.get_node_mut(channel_id).close_receiver();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_test_drop() {
        let mut crt = InnerChannelRt::new();

        let channel_id = crt.create();
        crt.inc_sender(channel_id);
        assert!(crt.awake_and_get_event_id().is_none());
        crt.dec_sender(channel_id);
        assert!(crt.awake_and_get_event_id().is_none());
        crt.close_receiver(channel_id);
        assert!(crt.awake_and_get_event_id().is_none());
    }
}

