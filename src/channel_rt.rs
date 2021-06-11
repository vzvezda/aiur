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
#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) struct ChannelId(u32);

impl ChannelId {
    pub(crate) fn null() -> Self {
        ChannelId(0)
    }
}

impl std::fmt::Debug for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("chan:{}", self.0))
    }
}

// The result of exchange<T> for send/receive future
#[derive(PartialEq)]
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
    Idle,
    Registered(RegInfo),
    Gone,
}

// This is a state for senders
enum TxState {
    Registered(RegInfo),
    Exchanged(RegInfo),
}

// Only really used for tracing to specifiy if this is a sender or receiver that should be
// awaken.
#[derive(Copy, Clone)]
enum Peer {
    Sender,
    Receiver,
}

struct WakeEvent {
    peer: Peer,
    waker: Waker,
    event_id: EventId,
}

impl WakeEvent {
    fn new(peer: Peer, waker: Waker, event_id: EventId) -> Self {
        Self {
            peer,
            waker,
            event_id,
        }
    }

    fn wake(&self) -> EventId {
        self.waker.wake_by_ref();
        self.event_id
    }
}

impl std::fmt::Debug for RxState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RxState::Idle => f.write_str("Idle"),
            RxState::Registered(..) => f.write_str("Registered"),
            RxState::Gone => f.write_str("Gone"),
        }
    }
}

impl std::fmt::Debug for TxState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxState::Registered(..) => f.write_str("Registered"),
            TxState::Exchanged(..) => f.write_str("Exchanged"),
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
        let node = Self {
            id: channel_id,
            rx_state: RxState::Idle,
            tx_queue: Vec::new(),
            senders_alive: 0, // intially incremented by ChSender::new()
        };

        modtrace!("ChannelRt: new {:?} {:?}", channel_id, node);
        node
    }

    fn add_sender_future(&mut self, reg_info: RegInfo) {
        let old_self = format!("{:?}", self); // fix alloc here and elsewhere
        self.tx_queue.push(TxState::Registered(reg_info));
        modtrace!(
            "ChannelRt: {:?} add sender future {} -> {:?} ",
            self.id,
            old_self,
            self
        );
    }

    fn reg_recv_future(&mut self, reg_info: RegInfo) {
        let old_self = format!("{:?}", self);
        self.rx_state = RxState::Registered(reg_info);
        modtrace!(
            "ChannelRt: {:?} reg receiver future {} -> {:?} ",
            self.id,
            old_self,
            self
        );
    }

    fn inc_sender(&mut self) {
        let old_self = format!("{:?}", self);
        self.senders_alive += 1;
        modtrace!(
            "ChannelRt: {:?} inc senders {} -> {:?} ",
            self.id,
            old_self,
            self
        );
    }

    fn dec_sender(&mut self) {
        let old_self = format!("{:?}", self);
        self.senders_alive -= 1;
        modtrace!(
            "ChannelRt: {:?} dec senders {} -> {:?} ",
            self.id,
            old_self,
            self
        );
    }

    fn close_receiver(&mut self) {
        let old_self = format!("{:?}", self);
        self.rx_state = RxState::Gone;
        modtrace!(
            "ChannelRt: {:?} receiver gone {} -> {:?}",
            self.id,
            old_self,
            self
        );
    }

    fn is_channel_alive(&self) -> bool {
        self.senders_alive > 0 || !matches!(self.rx_state, RxState::Gone)
    }

    // This is the implementation for the runtime if this ChannelNode ready to produce any
    // event. It does not awake waker by itself.
    fn get_wake_event(&self) -> Option<WakeEvent> {
        // Verify if there is a sender future that just got its data transfered to a receiver,
        // that should be awoken. It does not matter in what state the Receiver is.
        if let Some(ref first_tx_state) = self.tx_queue.first() {
            if let TxState::Exchanged(tx_reg_info) = first_tx_state {
                return Some(WakeEvent::new(
                    Peer::Sender,
                    tx_reg_info.waker.clone(),
                    tx_reg_info.event_id,
                ));
            }
        }

        if matches!(self.rx_state, RxState::Idle) {
            return None; // Receiver alive but Idle -> nobody to awake
        }

        if self.tx_queue.is_empty() && self.senders_alive > 0 {
            return None; // no sender futures right now, but there are alive senders
        }

        // Awake the receiver if it is Registered
        match &self.rx_state {
            RxState::Registered(ref rx_reg_info) => {
                // the state of sender is that it either have TxState::Registered in queue,
                // or sender_alive = 0.  This is verified by code above.
                debug_assert!(!self.tx_queue.is_empty() || self.senders_alive == 0);

                // We should awake receiver to either swap (if TxState::Registered) or to
                // receive ChannelClosed err (no alive senders)
                return Some(WakeEvent::new(
                    Peer::Receiver,
                    rx_reg_info.waker.clone(),
                    rx_reg_info.event_id,
                ));
            }
            _ => (),
        }

        // When we are here, the receiver is not in Idle and not in Registered, which means
        // it is in Gone.
        debug_assert!(matches!(self.rx_state, RxState::Gone));

        // We now can awake any sender futures one by one if there are any.
        if let Some(first_tx_state) = self.tx_queue.first() {
            // Btw sender future can be only in Registered state, verified by code above.
            debug_assert!(matches!(first_tx_state, TxState::Registered(..)));

            match first_tx_state {
                TxState::Registered(first_tx_reg_info) => {
                    return Some(WakeEvent::new(
                        Peer::Sender,
                        first_tx_reg_info.waker.clone(),
                        first_tx_reg_info.event_id,
                    ));
                }

                _ => (),
            }
        }

        // We do not really have to have such nodes when both receiver is gone and all senders
        // are gone as well. The runtime should remove such nodes and should not invoke this
        // method anyway.
        debug_assert!(self.senders_alive > 0);

        // There are alive senders that does not have any active futures right now, means
        // this node does not produce any event.
        None
    }

    // Makes the data exchange using std::mem::swap
    unsafe fn exchange_impl<T>(tx_data: *mut (), rx_data: *mut ()) {
        let tx_data = std::mem::transmute::<*mut (), *mut Option<T>>(tx_data);
        let rx_data = std::mem::transmute::<*mut (), *mut Option<T>>(rx_data);
        std::mem::swap(&mut *tx_data, &mut *rx_data);
    }

    // This is invoked by Sender future and it should be asserted that there is a
    // sender future is Registered with either exchanged value or not.
    unsafe fn exchange_sender<T>(&mut self) -> ExchangeResult {
        let old_node = format!("{:?}", self);

        if let Some(first_tx_state) = self.tx_queue.first_mut() {
            match (&self.rx_state, first_tx_state) {
                (RxState::Gone, TxState::Registered(..)) => {
                    self.tx_queue.remove(0);
                    modtrace!(
                        "ChannelRt: {:?} awoken sender {} -> {:?}",
                        self.id,
                        old_node,
                        self
                    );
                    // Receiver is gone and sender still holds the value
                    ExchangeResult::Disconnected
                }
                (_, TxState::Exchanged(..)) => {
                    self.tx_queue.remove(0);
                    modtrace!(
                        "ChannelRt: {:?} awoken sender {} -> {:?}",
                        self.id,
                        old_node,
                        self
                    );
                    // This is a typical good exhange scenario that receiver has moved
                    // the value out of sender storage and replanced it with None.
                    ExchangeResult::Done
                }
                (_, _) => {
                    panic!(
                        "ChannelRt: {:?} exchange_sender unexpected state: {}",
                        self.id, old_node
                    );
                }
            }
        } else {
            // do not invoke exchange_sender() when sender does not have a future registered
            panic!(
                "ChannelRt: {:?} exhange_sender with no sender: {}",
                self.id, old_node
            );
        }
    }

    // This is invoked by Receiver future and the precondition that receiver future has
    // registered.
    unsafe fn exchange_receiver<T>(&mut self) -> ExchangeResult {
        let old_node = format!("{:?}", self);

        if let Some(first_tx_state) = self.tx_queue.first_mut() {
            // Just do the actual data exchange between receiver and first sender in queue.
            // It can happen that between we awake the receiver and it invokes exchange_receiver()
            // there are one more future removed from tx_queue, but it does not matter, the
            // exchange with first sender is ok.
            match (&self.rx_state, &first_tx_state) {
                (RxState::Registered(ref rx_reg_info), TxState::Registered(ref tx_reg_info)) => {
                    Self::exchange_impl::<T>(rx_reg_info.data, tx_reg_info.data);
                    self.rx_state = RxState::Idle;
                    *first_tx_state = TxState::Exchanged(tx_reg_info.clone());
                    modtrace!(
                        "ChannelRt: {:?} mem::swapped  {} -> {:?}",
                        self.id,
                        old_node,
                        self
                    );
                    ExchangeResult::Done
                }
                // other state are not legal and should be asserted by Channel Futures:
                //    * Receiver: it must not call exhange_receiver() if not in Registered state
                //    * Sender: there should be no way exchange_receiver() is invoked while
                //              sender is in Exhanged state.
                _ => panic!(
                    "ChannelRt: exchange_receiver unexpected {:?} {:?}",
                    self.rx_state, first_tx_state
                ),
            }
        } else {
            // There is no sender future
            if self.senders_alive == 0 {
                // The receiver might awoken because there is no senders anymore, so
                // the sender's end of the channel is Disconnected.
                ExchangeResult::Disconnected
            } else {
                // It looks like the sender future was dropped after Receiver future is awoken.
                // Receiver can be awoken later.
                ExchangeResult::TryLater
            }
        }
    }

}

// Produce a state like "(C,R}". See the state machine chart in code below for meaning.
impl std::fmt::Debug for ChannelNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // (R <- [R+7]:3)
        f.write_str("(")?;

        match self.rx_state {
            RxState::Idle => f.write_str("Idle <- "),
            RxState::Registered(..) => f.write_str("Reg <- "),
            RxState::Gone => f.write_str("Gone <- "),
        }?;

        let tx_len = self.tx_queue.len();

        if tx_len > 0 {
            match self.tx_queue[0] {
                TxState::Registered(..) => f.write_str("[Reg"),
                TxState::Exchanged(..) => f.write_str("[Exch"),
            }?;

            if tx_len > 1 {
                f.write_fmt(format_args!(",..+{}]:", tx_len - 1))?;
            }
            f.write_str("]:")?;
        } else {
            f.write_str("[0]:")?;
        }

        f.write_fmt(format_args!("{}", self.senders_alive))?;

        f.write_str(")")
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

    // This is invoked by Sender future and it should be asserted that there is a
    // sender future is Registered with either exchanged value or not.
    //
    // Panics if channel_id is not found and if channel id is inconsistent state.
    unsafe fn exchange_sender<T>(&mut self, channel_id: ChannelId) -> ExchangeResult {
        self.get_node_mut(channel_id).exchange_sender::<T>()
    }

    // This is invoked by Receiver future and the precondition that receiver future has
    // registered. 
    //
    // Panics if channel_id is not found and if channel id is inconsistent state.
    unsafe fn exchange_receiver<T>(&mut self, channel_id: ChannelId) -> ExchangeResult {
        self.get_node_mut(channel_id).exchange_receiver::<T>()
    }

    // Awakes the waker and returns its EventId
    fn get_event_id_for_node(node: &ChannelNode) -> Option<EventId> {
        node.get_wake_event().map(|ev| ev.wake())
    }

    fn awake_and_get_event_id(&mut self) -> Option<EventId> {
        // TODO: iteration for nodes
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
            modtrace!("ChannelRt: {:?} has been dropped", channel_id);
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
    // Suddenly I understood that I can test the InnerChannelRt in isolation, so I have created
    // API tests here. These tests below helped me to develop the InnerChannelRt.
    use super::*;

    // This creates (Waker, EventId) that can be used for testing Channel Runtime API. The waker
    // is kind of fake it will not awake anything, but we need one for channel peers registration.
    fn create_fake_event(event_ptr: *const ()) -> (Waker, EventId) {
        use std::task::{RawWaker, RawWakerVTable};

        fn to_waker() -> std::task::Waker {
            // Cloning the waker returns just the copy of the pointer.
            unsafe fn clone_impl(raw_waker_ptr: *const ()) -> RawWaker {
                // Just copy the pointer, which is works as a clone
                RawWaker::new(raw_waker_ptr, &FAKE_WAKER_TABLE)
            }

            // Wake the future
            unsafe fn wake_impl(raw_waker_ptr: *const ()) {
                wake_by_ref_impl(raw_waker_ptr);
            }

            // Wake the future by ref
            unsafe fn wake_by_ref_impl(_raw_waker_ptr: *const ()) {
                // nothing here, this waker does not awake anything
            }

            // Drop the waker
            unsafe fn drop_impl(_raw_waker_ptr: *const ()) {}

            const FAKE_WAKER_TABLE: RawWakerVTable =
                RawWakerVTable::new(clone_impl, wake_impl, wake_by_ref_impl, drop_impl);

            let raw_waker = RawWaker::new(
                std::ptr::null(), // Can i use it? I don't dereference raw waker ptr anywhere
                &FAKE_WAKER_TABLE,
            );

            unsafe { Waker::from_raw(raw_waker) }
        }

        return (to_waker(), EventId(event_ptr)); // TODO
    }

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

    #[test]
    fn api_test_good_exhange() {
        let mut crt = InnerChannelRt::new();

        // TODO: we have UB here: mut reference sender data and *mut ptr in exhange.
        // I think we should forget sender_data.

        let mut sender_data: Option<u32> = Some(100);
        let mut receiver_data: Option<u32> = None;

        let mut sender_ptr = &mut sender_data as *mut Option<u32> as *mut ();
        let mut receiver_ptr = &mut receiver_data as *mut Option<u32> as *mut ();

        let (sender_waker, sender_event) = create_fake_event(sender_ptr);
        let (receiver_waker, receiver_event) = create_fake_event(receiver_ptr);

        let channel_id = crt.create();

        crt.inc_sender(channel_id);
        crt.reg_receiver_fut(channel_id, receiver_waker, receiver_event, receiver_ptr);
        assert!(crt.awake_and_get_event_id().is_none(), "No event expected");
        crt.add_sender_fut(channel_id, sender_waker, sender_event, sender_ptr);

        // exchange for the receiver
        let event = crt.awake_and_get_event_id();
        assert!(event.is_some(), "It is time for event");
        let event = event.unwrap();
        assert!(event == receiver_event, "Receiver event expected");
        assert!(
            unsafe { crt.exchange_receiver::<Option<u32>>(channel_id) } == ExchangeResult::Done
        );

        // exchange the sender
        let event = crt.awake_and_get_event_id();
        assert!(event.is_some(), "It is time for event");
        let event = event.unwrap();
        assert!(event == sender_event, "Sender event expected");
        assert!(unsafe { crt.exchange_sender::<Option<u32>>(channel_id) } == ExchangeResult::Done);

        assert!(receiver_data.expect("Date expected in receiver after exchange") == 100);

        crt.dec_sender(channel_id);
        crt.close_receiver(channel_id);
    }
}
