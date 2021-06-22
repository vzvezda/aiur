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
#[derive(PartialEq, Debug)]
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
struct TxState {
    completion: TxCompletion,
    waker: Waker,
    event_id: EventId,
}

enum TxCompletion {
    Pinned(*mut ()),
    Exchanged,
}

impl TxState {
    fn new(reg_info: RegInfo) -> Self {
        Self {
            completion: TxCompletion::Pinned(reg_info.data),
            waker: reg_info.waker,
            event_id: reg_info.event_id,
        }
    }
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
        match self.completion {
            TxCompletion::Pinned(..) => f.write_str("Pinned"),
            TxCompletion::Exchanged => f.write_str("Exchanged"),
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

    // This is a helper function that runs the state mutated closure with a traced
    // state before and after, like this:
    // 'aiur::ChannelRt: chan:1 reg receiver future (Idle <- [0]:1) -> (Reg <- [0]:1)'
    fn traced<MutateStateFn: FnOnce(&mut ChannelNode)>(
        &mut self,
        mut_state_fn: MutateStateFn,
        op: &str,
    ) {
        if MODTRACE {
            // remember the old state
            let old_self = format!("{:?}", self); // TODO: remove alloc usage here elsewhere

            // mutate the channel node
            mut_state_fn(self);

            // trace state change as old -> new
            modtrace!(
                "ChannelRt: {:?} {} {} -> {:?} ",
                self.id,
                op,
                old_self,
                self
            );
        } else {
            mut_state_fn(self)
        }
    }

    fn add_sender_future(&mut self, reg_info: RegInfo) {
        self.traced(
            |node| {
                node.tx_queue.push(TxState::new(reg_info));
            },
            "add sender future",
        );
    }

    fn reg_recv_future(&mut self, reg_info: RegInfo) {
        self.traced(
            |node| {
                node.rx_state = RxState::Registered(reg_info);
            },
            "reg receiver future",
        );
    }

    fn inc_sender(&mut self) {
        self.traced(
            |node| {
                node.senders_alive += 1;
            },
            "inc senders",
        );
    }

    fn dec_sender(&mut self) {
        self.traced(
            |node| {
                node.senders_alive -= 1;
            },
            "dec senders",
        );
    }

    fn cancel_sender_fut(&mut self, event_id: EventId) {
        self.traced(|node| todo!(), "receiver future canceled");
    }

    fn cancel_receiver_fut(&mut self) {
        self.traced(
            |node| {
                node.rx_state = RxState::Idle;
            },
            "receiver future canceled",
        );
    }

    fn is_channel_alive(&self) -> bool {
        self.senders_alive > 0 || !matches!(self.rx_state, RxState::Gone)
    }

    fn close_receiver(&mut self) {
        self.traced(
            |node| {
                node.rx_state = RxState::Gone;
            },
            "receiver gone",
        );
    }

    // This is the implementation for the runtime if this ChannelNode ready to produce any
    // event. It does not awake waker by itself.
    fn get_wake_event(&self) -> Option<WakeEvent> {
        // Verify if there is a sender future that just got its data transfered to a receiver,
        // that should be awoken. It does not matter in what state the Receiver is.
        if let Some(ref first_tx_state) = self.tx_queue.first() {
            if let TxCompletion::Exchanged = first_tx_state.completion {
                return Some(WakeEvent::new(
                    Peer::Sender,
                    first_tx_state.waker.clone(),
                    first_tx_state.event_id,
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
            // Btw sender future can be only in Registered state, asserted by the code above.
            debug_assert!(matches!(
                first_tx_state.completion,
                TxCompletion::Pinned(..)
            ));

            match first_tx_state.completion {
                TxCompletion::Pinned(..) => {
                    return Some(WakeEvent::new(
                        Peer::Sender,
                        first_tx_state.waker.clone(),
                        first_tx_state.event_id,
                    ));
                }

                _ => (),
            }
        }

        // All senders are gone and receiver is gone: None would be ok, but it probably
        // a bug, such Node should be dropped and we don't want to get event for it.
        debug_assert!(self.senders_alive > 0);

        // There are alive senders that does not have any active futures right now, means
        // this node does not produce any event.
        None
    }

    // Makes the data exchange using std::mem::swap, copy data from one future into another
    // future.
    unsafe fn exchange_impl<T>(tx_data: *mut (), rx_data: *mut ()) {
        let tx_data = std::mem::transmute::<*mut (), *mut Option<T>>(tx_data);
        let rx_data = std::mem::transmute::<*mut (), *mut Option<T>>(rx_data);
        std::mem::swap(&mut *tx_data, &mut *rx_data);
    }

    // This is invoked by Sender future and it should be asserted that there is a
    // sender future is Registered with either exchanged value or not.
    unsafe fn exchange_sender<T>(&mut self) -> ExchangeResult {
        if let Some(first_tx_state) = self.tx_queue.first_mut() {
            match (&self.rx_state, &first_tx_state.completion) {
                (RxState::Gone, TxCompletion::Pinned(..)) => {
                    self.traced(
                        |node| {
                            node.tx_queue.remove(0);
                        },
                        "awoken sender",
                    );
                    // Receiver is gone and sender still holds the value
                    ExchangeResult::Disconnected
                }
                (_, TxCompletion::Exchanged) => {
                    self.traced(
                        |node| {
                            node.tx_queue.remove(0);
                        },
                        "awoken sender",
                    );
                    // This is a typical good exhange scenario that receiver has moved
                    // the value out of sender storage and replanced it with None.
                    ExchangeResult::Done
                }
                (_, _) => {
                    panic!(
                        "ChannelRt: {:?} exchange_sender unexpected state: {:?}",
                        self.id, self
                    );
                }
            }
        } else {
            // do not invoke exchange_sender() when sender does not have a future registered
            panic!(
                "ChannelRt: {:?} exhange_sender with no sender: {:?}",
                self.id, self
            );
        }
    }

    // This is invoked by Receiver future and the precondition that receiver future has
    // registered.
    unsafe fn exchange_receiver<T>(&mut self) -> ExchangeResult {
        if let Some(first_tx_state) = self.tx_queue.first_mut() {
            // Just do the actual data exchange between receiver and first sender in queue.
            // It can happen that between we awake the receiver and it invokes exchange_receiver()
            // there are one more future removed from tx_queue, but it does not matter, the
            // exchange with first sender is ok.
            match (&self.rx_state, &first_tx_state.completion) {
                (RxState::Registered(ref rx_reg_info), TxCompletion::Pinned(tx_ptr)) => {
                    Self::exchange_impl::<T>(rx_reg_info.data, *tx_ptr);
                    self.traced(
                        move |node| {
                            node.rx_state = RxState::Idle;
                            node.tx_queue[0].completion = TxCompletion::Exchanged;
                        },
                        "mem::swapped",
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

// Produce a state like "(Reg <- [Reg+7]:3)"
//                         ^       ^  ^   ^-senders alive
//                         |       |  +-----how many sender futures registered (pinned)
//                         |       +--------state of the first sender (registered, exhanged)
//                         +----------------state of the receiver
impl std::fmt::Debug for ChannelNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("(")?;

        match self.rx_state {
            RxState::Idle => f.write_str("Idle <- "),
            RxState::Registered(..) => f.write_str("Reg <- "),
            RxState::Gone => f.write_str("Gone <- "),
        }?;

        let tx_len = self.tx_queue.len();

        if tx_len > 0 {
            match self.tx_queue[0].completion {
                TxCompletion::Pinned(..) => f.write_str("[Pin"),
                TxCompletion::Exchanged => f.write_str("[Exch"),
            }?;

            if tx_len > 1 {
                f.write_fmt(format_args!(", Pin:{}]:", tx_len - 1))?;
            } else {
                f.write_str("]:")?;
            }
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
        self.get_node_mut(channel_id).cancel_sender_fut(event_id);
    }

    fn cancel_receiver_fut(&mut self, channel_id: ChannelId) {
        self.get_node_mut(channel_id).cancel_receiver_fut();
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
        return (create_fake_waker(event_ptr), EventId(event_ptr));
    }

    fn create_fake_waker(ptr: *const ()) -> std::task::Waker {
        use std::task::{RawWaker, RawWakerVTable};
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

        let raw_waker = RawWaker::new(ptr, &FAKE_WAKER_TABLE);

        unsafe { Waker::from_raw(raw_waker) }
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

    // This is a test sender
    struct RecvEmu<'rt> {
        crt: &'rt ChannelRt,
        channel_id: ChannelId,
        event_id: EventId,
        waker: Waker,
        ptr: *mut (),
    }

    impl<'rt> RecvEmu<'rt> {
        fn new(crt: &'rt ChannelRt, channel_id: ChannelId, storage: &mut Option<u32>) -> Self {
            let ptr = storage as *mut Option<u32> as *mut ();
            Self {
                crt,
                channel_id,
                event_id: EventId(ptr),
                waker: create_fake_waker(ptr),
                ptr,
            }
        }

        fn register(&self) {
            self.crt
                .reg_receiver_fut(self.channel_id, self.waker.clone(), self.event_id, self.ptr);
        }

        fn assert_event(&self, event_id: Option<EventId>) {
            assert_eq!(self.event_id, event_id.expect("Event is expected"));
        }

        fn cancel(&self) {
            self.crt.cancel_receiver_fut(self.channel_id);
        }

        unsafe fn assert_value(&self, rhs: &Option<u32>) {
            assert_eq!(*(self.ptr as *const Option<u32>), *rhs);
        }

        unsafe fn clear_storage(&mut self) {
            // Clear the receiver's value to be able to repeat
            (*(self.ptr as *mut Option<u32>)) = None;
        }

        unsafe fn exchange(&self, expected: ExchangeResult) {
            assert_eq!(
                self.crt.exchange_receiver::<Option<u32>>(self.channel_id),
                expected
            );
        }

        unsafe fn assert_completion(
            &mut self,
            event_id: Option<EventId>,
            exch_result: ExchangeResult,
            value: &Option<u32>,
        ) {
            self.assert_event(event_id);
            self.exchange(exch_result);
            self.assert_value(value);
        }
    }

    impl<'rt> Drop for RecvEmu<'rt> {
        fn drop(&mut self) {
            self.crt.close_receiver(self.channel_id);
        }
    }

    // This is a test sender
    struct SenderEmu<'rt> {
        crt: &'rt ChannelRt,
        channel_id: ChannelId,
        event_id: EventId,
        waker: Waker,
        ptr: *mut (),
    }

    impl<'rt> SenderEmu<'rt> {
        fn new(crt: &'rt ChannelRt, channel_id: ChannelId, storage: &mut Option<u32>) -> Self {
            let ptr = storage as *mut Option<u32> as *mut ();
            crt.inc_sender(channel_id);
            Self {
                crt,
                channel_id,
                event_id: EventId(ptr),
                waker: create_fake_waker(ptr),
                ptr,
            }
        }

        fn register(&self) {
            self.crt
                .add_sender_fut(self.channel_id, self.waker.clone(), self.event_id, self.ptr);
        }

        fn assert_event(&self, event_id: Option<EventId>) {
            assert_eq!(self.event_id, event_id.expect("Event is expected"));
        }

        fn cancel(&self) {
            self.crt.cancel_sender_fut(self.channel_id, self.event_id);
        }

        unsafe fn exchange(&self, expected: ExchangeResult) {
            assert_eq!(
                self.crt.exchange_sender::<Option<u32>>(self.channel_id),
                expected
            );
        }

        unsafe fn assert_value(&self, rhs: &Option<u32>) {
            assert_eq!(*(self.ptr as *mut Option<u32>), *rhs);
        }

        unsafe fn assert_completion(
            &self,
            event_id: Option<EventId>,
            exch_result: ExchangeResult,
            value: &Option<u32>,
        ) {
            self.assert_event(event_id);
            self.exchange(exch_result);
            self.assert_value(value);
        }
    }

    impl<'rt> Drop for SenderEmu<'rt> {
        fn drop(&mut self) {
            self.crt.dec_sender(self.channel_id);
        }
    }

    #[test]
    fn api_test_good_exhange() {
        let mut crt = ChannelRt::new();

        let mut sender: Option<u32> = Some(100);
        let mut recv: Option<u32> = None;

        let channel_id = crt.create();

        // Hide the storage variable above to avoid having multiple mutable references
        // to the same object.
        let sender = SenderEmu::new(&crt, channel_id, &mut sender);
        let mut recv = RecvEmu::new(&crt, channel_id, &mut recv);

        recv.register();
        assert!(crt.awake_and_get_event_id().is_none());
        sender.register();

        unsafe {
            // receiver awoken and have got the right value after exchange
            recv.assert_completion(
                crt.awake_and_get_event_id(),
                ExchangeResult::Done,
                &Some(100),
            );
            recv.clear_storage();

            // verifies that sender is awoken and had value taken out after exchange
            sender.assert_completion(crt.awake_and_get_event_id(), ExchangeResult::Done, &None);
        }
    }

    #[test]
    fn api_test_cancel_receiver() {
        let mut crt = ChannelRt::new();

        let mut sender: Option<u32> = Some(100);
        let mut recv: Option<u32> = None;

        let channel_id = crt.create();

        // Hide the storage variable above to avoid having multiple mutable references
        // to the same object.
        let sender = SenderEmu::new(&crt, channel_id, &mut sender);
        let mut recv = RecvEmu::new(&crt, channel_id, &mut recv);

        sender.register();
        assert!(crt.awake_and_get_event_id().is_none());
        recv.register();


        unsafe {
            // verify if receiver is awoken
            recv.assert_event(crt.awake_and_get_event_id());
            drop(recv); // instead of exchange just close the receiver

            // Sender now awoken with exchange result to be disconnected and value
            // still on senders side.
            sender.assert_completion(crt.awake_and_get_event_id(), 
                ExchangeResult::Disconnected, &Some(100));
        }
    }

    #[test]
    fn api_test_send_two_values() {
        let crt = ChannelRt::new();

        // storage for exchange
        let mut sender1: Option<u32> = Some(100);
        let mut sender2: Option<u32> = Some(50);
        let mut recv: Option<u32> = None;

        let channel_id = crt.create();

        // Hide the storage variable above to avoid having multiple mutable references
        // to the same object.
        let sender1 = SenderEmu::new(&crt, channel_id, &mut sender1);
        let sender2 = SenderEmu::new(&crt, channel_id, &mut sender2);
        let mut recv = RecvEmu::new(&crt, channel_id, &mut recv);

        recv.register();
        assert!(crt.awake_and_get_event_id().is_none());

        sender1.register();
        sender2.register();

        unsafe {
            // receiver awoken and have got the right value after exchange
            recv.assert_completion(
                crt.awake_and_get_event_id(),
                ExchangeResult::Done,
                &Some(100),
            );
            recv.clear_storage();

            // verifies that sender is awoken and had value taken out after exchange
            sender1.assert_completion(crt.awake_and_get_event_id(), ExchangeResult::Done, &None);

            // prepare sender once again
            recv.register();

            // receiver awoken and have got the right value after exchange
            recv.assert_completion(
                crt.awake_and_get_event_id(),
                ExchangeResult::Done,
                &Some(50),
            );

            // verifies that sender is awoken and had value taken out after exchange
            sender2.assert_completion(crt.awake_and_get_event_id(), ExchangeResult::Done, &None);
        }
    }
}
