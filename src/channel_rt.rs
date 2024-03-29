//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::cell::RefCell;

use crate::reactor::EventId;
use crate::tracer::Tracer;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

// Channel handle used by this low level channel API, which is only has crate visibility.
#[derive(Copy, Clone, Eq, PartialEq)]
pub(crate) struct ChannelId(u32);

impl std::fmt::Debug for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("chan:{}", self.0))
    }
}

// The result of swap<T> for send/receive future
#[derive(PartialEq, Debug)]
pub(crate) enum SwapResult {
    Done,
    Disconnected,
    TryLater, // a new state in compare to oneshot
}

// Both sender and receiver has almost the same API which is in this trait.
pub(crate) trait PeerRt {
    fn pin(&self, event_id: EventId, pointer: *mut ());
    fn unpin(&self, event_id: EventId);
    fn close(&self);

    unsafe fn swap<T>(&self) -> SwapResult;
}

// Sender API
#[derive(Copy, Clone)]
pub(crate) struct SenderRt<'rt> {
    channel_rt: &'rt ChannelRt,
    pub(crate) channel_id: ChannelId, // visible as it used for tracing
}

impl<'rt> SenderRt<'rt> {
    pub(crate) fn inc_ref(&self) {
        self.channel_rt.inc_sender(self.channel_id)
    }
}

impl<'rt> PeerRt for SenderRt<'rt> {
    fn pin(&self, event_id: EventId, pointer: *mut ()) {
        self.channel_rt
            .add_sender_fut(self.channel_id, event_id, pointer)
    }
    fn unpin(&self, event_id: EventId) {
        self.channel_rt.cancel_sender_fut(self.channel_id, event_id)
    }
    unsafe fn swap<T>(&self) -> SwapResult {
        self.channel_rt.swap_sender::<T>(self.channel_id)
    }
    fn close(&self) {
        self.channel_rt.dec_sender(self.channel_id)
    }
}

// Receiver API
#[derive(Copy, Clone)]
pub(crate) struct RecverRt<'rt> {
    channel_rt: &'rt ChannelRt,
    pub(crate) channel_id: ChannelId, // visible as it used for tracing
}

impl<'rt> PeerRt for RecverRt<'rt> {
    fn pin(&self, event_id: EventId, pointer: *mut ()) {
        self.channel_rt
            .reg_receiver_fut(self.channel_id, event_id, pointer)
    }
    fn unpin(&self, _event_id: EventId) {
        self.channel_rt.cancel_receiver_fut(self.channel_id)
    }
    unsafe fn swap<T>(&self) -> SwapResult {
        self.channel_rt.swap_receiver::<T>(self.channel_id)
    }
    fn close(&self) {
        self.channel_rt.close_receiver(self.channel_id)
    }
}

// Runtime API for Channel futures
pub(crate) struct ChannelRt {
    // Actual implementation forwarded to inner struct with mutability. Perhaps the
    // UnsafeCell should be ok here since the API is private for the crate.
    inner: RefCell<InnerChannelRt>,
}

impl ChannelRt {
    pub(crate) fn new(tracer: &Tracer) -> Self {
        ChannelRt {
            inner: RefCell::new(InnerChannelRt::new(tracer)),
        }
    }

    pub(crate) fn create(&self) -> ChannelId {
        self.inner.borrow_mut().create()
    }

    pub(crate) fn sender_rt<'rt>(&'rt self, channel_id: ChannelId) -> SenderRt<'rt> {
        SenderRt {
            channel_rt: self,
            channel_id,
        }
    }

    pub(crate) fn recver_rt<'rt>(&'rt self, channel_id: ChannelId) -> RecverRt<'rt> {
        RecverRt {
            channel_rt: self,
            channel_id,
        }
    }

    #[cfg(test)]
    fn is_exist(&self, channel_id: ChannelId) -> bool {
        self.inner.borrow().is_exist(channel_id)
    }

    
    pub(crate) fn get_awake_event_id(&self) -> Option<EventId> {
        self.inner.borrow_mut().get_awake_event_id()
    }

    fn add_sender_fut(
        &self,
        channel_id: ChannelId,
        event_id: EventId,
        data: *mut (),
    ) {
        self.inner
            .borrow_mut()
            .add_sender_fut(channel_id, event_id, data);
    }

    fn reg_receiver_fut(
        &self,
        channel_id: ChannelId,
        event_id: EventId,
        data: *mut (),
    ) {
        self.inner
            .borrow_mut()
            .reg_receiver_fut(channel_id, event_id, data);
    }

    unsafe fn swap_sender<T>(&self, channel_id: ChannelId) -> SwapResult {
        self.inner.borrow_mut().swap_sender::<T>(channel_id)
    }

    unsafe fn swap_receiver<T>(&self, channel_id: ChannelId) -> SwapResult {
        self.inner.borrow_mut().swap_receiver::<T>(channel_id)
    }

    fn inc_sender(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().inc_sender(channel_id);
    }

    fn dec_sender(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().dec_sender(channel_id);
    }

    fn close_receiver(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().close_receiver(channel_id);
    }

    fn cancel_sender_fut(&self, channel_id: ChannelId, event_id: EventId) {
        self.inner
            .borrow_mut()
            .cancel_sender_fut(channel_id, event_id);
    }

    fn cancel_receiver_fut(&self, channel_id: ChannelId) {
        self.inner.borrow_mut().cancel_receiver_fut(channel_id);
    }
}

// Registration info provided for both sender and receiver.
#[derive(Debug, Clone)]
struct RegInfo {
    data: *mut (),
    event_id: EventId,
}

impl RegInfo {
    fn new(data: *mut (), event_id: EventId) -> Self {
        RegInfo {
            data,
            event_id,
        }
    }
}

// This is the state of the receiver
enum RxState {
    Idle,
    Pinned(RegInfo),
    Gone,
}

// This is a state for senders
struct TxState {
    completion: TxCompletion,
    event_id: EventId,
}

enum TxCompletion {
    Pinned(*mut ()),
    Emptied,
}

impl TxState {
    fn new(reg_info: RegInfo) -> Self {
        Self {
            completion: TxCompletion::Pinned(reg_info.data),
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
    event_id: EventId,
}

impl WakeEvent {
    fn new(peer: Peer, event_id: EventId) -> Self {
        Self {
            peer,
            event_id,
        }
    }

    fn get_event_id(&self) -> EventId {
        self.event_id
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
    fn new(channel_id: ChannelId, tracer: &Tracer) -> Self {
        let node = Self {
            id: channel_id,
            rx_state: RxState::Idle,
            tx_queue: Vec::new(),
            senders_alive: 0, // intially incremented by ChSender::new()
        };

        modtrace!(tracer, "channel_rt: new {:?} {:?}", channel_id, node);
        node
    }

    // This is a helper function that runs the state mutated closure with a traced
    // state before and after, like this:
    // 'aiur::ChannelRt: chan:1 reg receiver future (Idle <- [0]:1) -> (Reg <- [0]:1)'
    fn traced<MutateStateFn: FnOnce(&mut ChannelNode)>(
        &mut self,
        tracer: &Tracer,
        op: &str,
        mut_state_fn: MutateStateFn,
    ) {
        if MODTRACE {
            // remember the old state
            let old_self = format!("{:?}", self); // TODO: remove alloc usage here

            // mutate the channel node
            mut_state_fn(self);

            // trace state change as old -> new
            modtrace!(
                tracer,
                "channel_rt: {:?} {} {} -> {:?} ",
                self.id,
                op,
                old_self,
                self
            );
        } else {
            mut_state_fn(self)
        }
    }

    fn add_sender_future(&mut self, reg_info: RegInfo, tracer: &Tracer) {
        self.traced(tracer, "add sender future", |node| {
            node.tx_queue.push(TxState::new(reg_info));
        });
    }

    fn reg_recv_future(&mut self, reg_info: RegInfo, tracer: &Tracer) {
        self.traced(tracer, "reg receiver future", |node| {
            node.rx_state = RxState::Pinned(reg_info);
        });
    }

    fn inc_sender(&mut self, tracer: &Tracer) {
        self.traced(tracer, "inc senders", |node| {
            node.senders_alive += 1;
        });
    }

    fn dec_sender(&mut self, tracer: &Tracer) {
        self.traced(tracer, "dec senders", |node| {
            node.senders_alive -= 1;
        });
    }

    fn cancel_sender_fut(&mut self, event_id: EventId, tracer: &Tracer) {
        self.traced(tracer, "sender future canceled", |node| {
            node.tx_queue.remove(
                node.tx_queue
                    .iter()
                    .position(|x| x.event_id == event_id)
                    .unwrap(),
            );
        });
    }

    fn cancel_receiver_fut(&mut self, tracer: &Tracer) {
        self.traced(tracer, "receiver future canceled", |node| {
            node.rx_state = RxState::Idle;
        });
    }

    fn is_channel_alive(&self) -> bool {
        self.senders_alive > 0 || !matches!(self.rx_state, RxState::Gone)
    }

    fn close_receiver(&mut self, tracer: &Tracer) {
        self.traced(tracer, "receiver gone", |node| {
            node.rx_state = RxState::Gone;
        });
    }

    // This is the implementation for the runtime if this ChannelNode ready to produce any
    // event. 
    fn get_wake_event(&self) -> Option<WakeEvent> {
        // Verify if there is a sender future that just got its data transferred to a receiver,
        // that should be awoken. It does not matter in what state the receiver is.
        if let Some(ref first_tx_state) = self.tx_queue.first() {
            if let TxCompletion::Emptied = first_tx_state.completion {
                return Some(WakeEvent::new(
                    Peer::Sender,
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

        // Awake the receiver if it is Pinned
        match &self.rx_state {
            RxState::Pinned(ref rx_reg_info) => {
                // the state of sender is that it either have TxState::Pinned in queue,
                // or sender_alive = 0.  This is verified by code above.
                debug_assert!(!self.tx_queue.is_empty() || self.senders_alive == 0);

                // We should awake receiver to either swap (if TxState::Pinned) or to
                // receive ChannelClosed err (no alive senders)
                return Some(WakeEvent::new(
                    Peer::Receiver,
                    rx_reg_info.event_id,
                ));
            }
            _ => (),
        }

        // When we are here, the receiver is not in Idle and not in Pinned, which means
        // it is in Gone.
        debug_assert!(matches!(self.rx_state, RxState::Gone));

        // We now can awake any sender futures one by one if there are any.
        if let Some(first_tx_state) = self.tx_queue.first() {
            // Btw sender future can be only in Pinned state, asserted by the code above.
            debug_assert!(matches!(
                first_tx_state.completion,
                TxCompletion::Pinned(..)
            ));

            match first_tx_state.completion {
                TxCompletion::Pinned(..) => {
                    return Some(WakeEvent::new(
                        Peer::Sender,
                        first_tx_state.event_id,
                    ));
                }

                _ => (),
            }
        }

        // All senders are gone and receiver is gone: None would be ok, but it probably
        // a bug, such Node should be dropped and we don't want to get event for it.
        //
        // later: commented this out because this function sometimes in impl Debug for
        // ChannelNode and this assert actually happens.
        //
        //debug_assert!(self.senders_alive > 0);

        // There are alive senders that does not have any active futures right now, means
        // this node does not produce any event.
        None
    }

    // Makes the data exchange using std::mem::swap, copy data from one future into another
    // future.
    // Unsafe: Caller should guaranty the validity of the pointers and the data type. This
    // is achieved in public crate API by using Unpin futures.
    unsafe fn exchange_impl<T>(tx_data: *mut (), rx_data: *mut ()) {
        let tx_data = std::mem::transmute::<*mut (), *mut Option<T>>(tx_data);
        let rx_data = std::mem::transmute::<*mut (), *mut Option<T>>(rx_data);
        std::mem::swap(&mut *tx_data, &mut *rx_data);
    }

    // This is invoked by Sender future and it should be asserted that there is a
    // sender future is Pinned with either Emptied value or not.
    unsafe fn swap_sender<T>(&mut self, tracer: &Tracer) -> SwapResult {
        if let Some(first_tx_state) = self.tx_queue.first_mut() {
            match (&self.rx_state, &first_tx_state.completion) {
                (RxState::Gone, TxCompletion::Pinned(..)) => {
                    self.traced(tracer, "awoken sender", |node| {
                        node.tx_queue.remove(0);
                    });
                    // Receiver is gone and sender still holds the value
                    SwapResult::Disconnected
                }
                (_, TxCompletion::Emptied) => {
                    self.traced(tracer, "awoken sender", |node| {
                        node.tx_queue.remove(0);
                    });
                    // This is a typical good exhange scenario that receiver has moved
                    // the value out of sender storage and replanced it with None.
                    SwapResult::Done
                }
                (_, _) => {
                    panic!(
                        "ChannelRt: {:?} swap_sender unexpected state: {:?}",
                        self.id, self
                    );
                }
            }
        } else {
            // do not invoke swap_sender() when sender does not have a future pinned
            panic!(
                "ChannelRt: {:?} exhange_sender with no sender: {:?}",
                self.id, self
            );
        }
    }

    // This is invoked by Receiver future and the precondition that receiver future has
    // pinned.
    unsafe fn swap_receiver<T>(&mut self, tracer: &Tracer) -> SwapResult {
        if let Some(first_tx_state) = self.tx_queue.first_mut() {
            // Just do the actual data exchange between receiver and first sender in queue.
            // It can happen that between we awake the receiver and it invokes swap_receiver()
            // there are one more future removed from tx_queue, but it does not matter, the
            // exchange with first sender is ok.
            match (&self.rx_state, &first_tx_state.completion) {
                (RxState::Pinned(ref rx_reg_info), TxCompletion::Pinned(tx_ptr)) => {
                    Self::exchange_impl::<T>(rx_reg_info.data, *tx_ptr);
                    self.traced(tracer, "mem::swapped", move |node| {
                        node.rx_state = RxState::Idle;
                        node.tx_queue[0].completion = TxCompletion::Emptied;
                    });
                    SwapResult::Done
                }
                // other state are not legal and should be asserted by Channel Futures:
                //    * Receiver: it must not call exhange_receiver() if not in Pinned state
                //    * Sender: there should be no way swap_receiver() is invoked while
                //              sender is in Exhanged state.
                _ => panic!("ChannelRt: swap_receiver unexpected {:?}", self),
            }
        } else {
            // There is no sender future
            if self.senders_alive == 0 {
                // The receiver might awoken because there is no senders anymore, so
                // the sender's end of the channel is Disconnected.
                SwapResult::Disconnected
            } else {
                // It looks like the sender future was dropped after Receiver future is awoken.
                // Receiver can be awoken later.
                SwapResult::TryLater
            }
        }
    }
}

// Textual form of the ChannelNode that is helpful for testing and development.
//
// Produce a state like "(@Pin <- [Pin+7]:3)"
//                        ^ ^       ^  ^  ^--# of senders alive
//                        | |       |  +-----how many sender futures has been pinned total
//                        | |       +--------state of the first sender (pinned, exhanged)
//                        | +----------------state of the receiver
//                        +------------------'@' indicates a future to be awoken in this state
//
// Receivers states are:
//     * 'Idle' - when receiver side is alive but did not provide pointer for swap
//     * 'Pin' - means that receiver provided pointer for swap
//     * 'Gone' - means that receiver's side of the channel is dropped
//
// Senders states are:
//     * 'Pin' - means sender has provided pointer for swap
//     * 'Empt' - means that receiver has taken data from sender
//
// Senders are organized as a queue, the only top sender can be 'Empt', other senders in
// queue are always 'Pin'. If 'Pin' sender in the middle of queue is closed, it just removed
// from queue without receiver knowing about that.
//
// When the number of senders is 0, this is just like Gone for receiver that there are no
// more sender and channel looks disconnected on Receiver side.
//
// When recv is Gone and # of senders is 0 - channel closed.
impl std::fmt::Debug for ChannelNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // "@"
        let (rx_event_tag, tx_event_tag) =
            self.get_wake_event()
                .map_or(("", ""), |wake_event| match wake_event.peer {
                    Peer::Sender => ("", "@"),
                    Peer::Receiver => ("@", ""),
                });

        f.write_str("(")?;
        f.write_str(rx_event_tag)?;

        match self.rx_state {
            RxState::Idle => f.write_str("Idle <- "),
            RxState::Pinned(..) => f.write_str("Pin <- "),
            RxState::Gone => f.write_str("Gone <- "),
        }?;

        let tx_len = self.tx_queue.len();

        if tx_len > 0 {
            f.write_str("[")?;
            f.write_str(tx_event_tag)?;

            match self.tx_queue[0].completion {
                TxCompletion::Pinned(..) => f.write_str("Pin"),
                TxCompletion::Emptied => f.write_str("Empt"),
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

// Actual implementation of internal Channel API with functions that do have &mut self.
struct InnerChannelRt {
    // Improvement ideas:
    //
    // There are various ideas how we can improve containers for channels in order to
    // avoid dynamic memory usages:
    //     * fixed size table
    //     * as the future is pinned, we can have some kind of 'intrusive list'
    //     * it can be the caller that provides store the channel node like
    //       let channel: Pin<&mut ChannelNode> = ...
    //
    // Another improvement idea that currently executor has to scan all channel objects
    // to verify if there is an event. Whenever there is a channel node that goes into
    // the state that produces awake event, it can be stored somewhere in a queue.
    // The queue can be also made by an intrusive list.
    //
    // These ideas seems to require preparing the benching.
    nodes: Vec<ChannelNode>,
    last_id: u32,
    tracer: Tracer,
}

impl InnerChannelRt {
    fn new(tracer: &Tracer) -> Self {
        InnerChannelRt {
            nodes: Vec::new(),
            last_id: 0,
            tracer: tracer.clone(),
        }
    }

    fn create(&mut self) -> ChannelId {
        self.last_id += 1;
        let channel_id = ChannelId(self.last_id);
        self.nodes.push(ChannelNode::new(channel_id, &self.tracer));
        channel_id
    }

    #[cfg(test)]
    fn is_exist(&self, channel_id: ChannelId) -> bool {
        self.nodes
            .iter()
            .find(|node| node.id == channel_id)
            .is_some()
    }

    // Returns mutable reference to node
    fn get_node_mut(&mut self, channel_id: ChannelId) -> &mut ChannelNode {
        self.nodes
            .iter_mut()
            .find(|node| node.id == channel_id)
            .unwrap() // panics if channel_id is not found
    }

    fn get_node(&mut self, channel_id: ChannelId) -> &ChannelNode {
        self.nodes
            .iter()
            .find(|node| node.id == channel_id)
            .unwrap() // panics if channel_id is not found
    }

    fn add_sender_fut(
        &mut self,
        channel_id: ChannelId,
        event_id: EventId,
        data: *mut (),
    ) {
        let reg_info = RegInfo::new(data, event_id);
        let tracer = self.tracer.clone();
        self.get_node_mut(channel_id)
            .add_sender_future(reg_info, &tracer);
    }

    fn reg_receiver_fut(
        &mut self,
        channel_id: ChannelId,
        event_id: EventId,
        data: *mut (),
    ) {
        let tracer = self.tracer.clone();
        let reg_info = RegInfo::new(data, event_id);
        self.get_node_mut(channel_id)
            .reg_recv_future(reg_info, &tracer);
    }

    // This is invoked by Sender future and it should be asserted that there is a
    // sender future has pointer pinned with either Emptied value or not.
    //
    // Panics if channel_id is not found and if channel id is inconsistent state.
    unsafe fn swap_sender<T>(&mut self, channel_id: ChannelId) -> SwapResult {
        let tracer = self.tracer.clone();
        self.get_node_mut(channel_id).swap_sender::<T>(&tracer)
    }

    // This is invoked by Receiver future and the precondition that receiver future has
    // registered.
    //
    // Panics if channel_id is not found and if channel id is inconsistent state.
    unsafe fn swap_receiver<T>(&mut self, channel_id: ChannelId) -> SwapResult {
        let tracer = self.tracer.clone();
        self.get_node_mut(channel_id).swap_receiver::<T>(&tracer)
    }

    // Awakes the waker and returns its EventId
    fn get_event_id_for_node(node: &ChannelNode) -> Option<EventId> {
        node.get_wake_event().map(|ev| ev.get_event_id())
    }

    fn get_awake_event_id(&mut self) -> Option<EventId> {
        self.nodes
            .iter()
            .find_map(|node| Self::get_event_id_for_node(&node))
    }

    fn inc_sender(&mut self, channel_id: ChannelId) {
        let tracer = self.tracer.clone();
        self.get_node_mut(channel_id).inc_sender(&tracer);
    }

    fn dec_sender(&mut self, channel_id: ChannelId) {
        let tracer = self.tracer.clone();
        self.get_node_mut(channel_id).dec_sender(&tracer);
        self.drop_channel_if_needed(channel_id);
    }

    fn close_receiver(&mut self, channel_id: ChannelId) {
        let tracer = self.tracer.clone();
        self.get_node_mut(channel_id).close_receiver(&tracer);
        self.drop_channel_if_needed(channel_id);
    }

    fn drop_channel_if_needed(&mut self, channel_id: ChannelId) {
        if !self.get_node(channel_id).is_channel_alive() {
            self.nodes.remove(
                self.nodes
                    .iter()
                    .position(|node| node.id == channel_id)
                    .unwrap(),
            );
            modtrace!(&self.tracer, "channel_rt: {:?} has been dropped", channel_id);
        }
    }

    fn cancel_sender_fut(&mut self, channel_id: ChannelId, event_id: EventId) {
        let tracer = self.tracer.clone();
        self.get_node_mut(channel_id)
            .cancel_sender_fut(event_id, &tracer);
    }

    fn cancel_receiver_fut(&mut self, channel_id: ChannelId) {
        let tracer = self.tracer.clone();
        self.get_node_mut(channel_id).cancel_receiver_fut(&tracer);
    }
}

#[cfg(test)]
mod tests {
    // Suddenly I understood that I can test the InnerChannelRt in isolation, so I have created
    // API tests here. These tests below helped me to develop the InnerChannelRt.
    use super::*;

    // Unified code for SenderEmu and RecverEmu
    struct PeerEmu<PeerT: PeerRt> {
        peer_rt: PeerT,
        event_id: EventId,
        ptr: *mut (),
    }

    // Sender and Reciever helper structs for API tests.
    type SenderEmu<'rt> = PeerEmu<SenderRt<'rt>>;
    type RecverEmu<'rt> = PeerEmu<RecverRt<'rt>>;

    // Most of the code for sender and receiver are the same when using PeerRt trait
    impl<PeerT: PeerRt> PeerEmu<PeerT> {
        fn register(&self) {
            self.peer_rt.pin(self.event_id, self.ptr);
        }

        fn cancel(&self) {
            self.peer_rt.unpin(self.event_id);
        }

        fn assert_event(&self, event_id: Option<EventId>) {
            assert_eq!(self.event_id, event_id.expect("Event is expected"));
        }

        unsafe fn assert_value(&self, rhs: &Option<u32>) {
            assert_eq!(*(self.ptr as *const Option<u32>), *rhs);
        }

        unsafe fn exchange(&self, expected: SwapResult) {
            assert_eq!(self.peer_rt.swap::<Option<u32>>(), expected);
        }

        unsafe fn assert_completion(
            &self,
            event_id: Option<EventId>,
            exch_result: SwapResult,
            value: &Option<u32>,
        ) {
            self.assert_event(event_id);
            self.exchange(exch_result);
            self.assert_value(value);
        }
    }

    // SenderEmu specific code: also has the inc_sender() for reference counting
    impl<'rt> PeerEmu<SenderRt<'rt>> {
        fn new(crt: &'rt ChannelRt, channel_id: ChannelId, storage: &mut Option<u32>) -> Self {
            let ptr = storage as *mut Option<u32> as *mut ();
            crt.inc_sender(channel_id);
            Self {
                peer_rt: crt.sender_rt(channel_id),
                event_id: EventId(ptr),
                ptr,
            }
        }
    }

    // RecverEmu specific code: also includes the clear storage for testing
    impl<'rt> PeerEmu<RecverRt<'rt>> {
        fn new(crt: &'rt ChannelRt, channel_id: ChannelId, storage: &mut Option<u32>) -> Self {
            let ptr = storage as *mut Option<u32> as *mut ();
            Self {
                peer_rt: crt.recver_rt(channel_id),
                event_id: EventId(ptr),
                ptr,
            }
        }

        // Clear the receiver's value to be able to repeat
        unsafe fn clear_storage(&mut self) {
            (*(self.ptr as *mut Option<u32>)) = None;
        }
    }

    impl<PeerT: PeerRt> Drop for PeerEmu<PeerT> {
        fn drop(&mut self) {
            self.peer_rt.close();
        }
    }

    /// Verifies that channel is destroyed after both sender and recver no longer attached.
    #[test]
    fn api_test_dec_references_destroys_channel() {
        let mut crt = InnerChannelRt::new(&Tracer::new_testing());

        let channel_id = crt.create();
        assert!(crt.is_exist(channel_id));
        crt.inc_sender(channel_id);
        assert!(crt.is_exist(channel_id));
        assert!(crt.get_awake_event_id().is_none());
        crt.dec_sender(channel_id);
        assert!(crt.get_awake_event_id().is_none());
        assert!(crt.is_exist(channel_id));
        crt.close_receiver(channel_id);
        assert!(crt.get_awake_event_id().is_none());
        assert!(!crt.is_exist(channel_id));
    }

    /// Verifies that sender and receiver has value changed after being pinned and
    /// invoking exchange().
    #[test]
    fn api_test_peers_pinned_gives_value_exchanged() {
        let crt = ChannelRt::new(&Tracer::new_testing());

        let mut sender: Option<u32> = Some(100);
        let mut recver: Option<u32> = None;

        let channel_id = crt.create();
        assert!(crt.is_exist(channel_id));

        // Hide the storage variable above to avoid having multiple mutable references
        // to the same object.
        let sender = SenderEmu::new(&crt, channel_id, &mut sender);
        let mut recver = RecverEmu::new(&crt, channel_id, &mut recver);

        recver.register();
        assert!(crt.get_awake_event_id().is_none());
        sender.register();

        unsafe {
            // receiver awoken and have got the right value after exchange
            recver.assert_completion(crt.get_awake_event_id(), SwapResult::Done, &Some(100));
            recver.clear_storage();

            // verifies that sender is awoken and had value taken out after exchange
            sender.assert_completion(crt.get_awake_event_id(), SwapResult::Done, &None);
        }

        drop(sender);
        drop(recver);

        assert!(!crt.is_exist(channel_id));
    }

    /// When invoking cancel() on pinned receiver future instead of exchange(), it gives
    /// the sender an error (Disconnecting) and the value back.
    #[test]
    fn api_test_cancel_receiver_gives_disconnected_err_on_sender() {
        let crt = ChannelRt::new(&Tracer::new_testing());

        let mut sender: Option<u32> = Some(100);
        let mut recver: Option<u32> = None;

        let channel_id = crt.create();

        // Hide the storage variable above to avoid having multiple mutable references
        // to the same object.
        let sender = SenderEmu::new(&crt, channel_id, &mut sender);
        let recver = RecverEmu::new(&crt, channel_id, &mut recver);

        sender.register();
        assert!(crt.get_awake_event_id().is_none());
        recver.register();

        unsafe {
            assert!(crt.is_exist(channel_id));

            // verify if receiver is awoken
            recver.assert_event(crt.get_awake_event_id());
            drop(recver); // instead of exchange just close the receiver

            // Sender now awoken with exchange result to be disconnected and value
            // still on senders side.
            sender.assert_completion(
                crt.get_awake_event_id(),
                SwapResult::Disconnected,
                &Some(100),
            );
        }

        drop(sender);

        assert!(!crt.is_exist(channel_id));
    }

    /// When there are two senders pinned, both values are received on recver side.
    #[test]
    fn api_test_two_sender_send_value_gives_both_received() {
        let crt = ChannelRt::new(&Tracer::new_testing());

        // storage for exchange
        let mut sender1: Option<u32> = Some(100);
        let mut sender2: Option<u32> = Some(50);
        let mut recver: Option<u32> = None;

        let channel_id = crt.create();

        // Hide the storage variable above to avoid having multiple mutable references
        // to the same object.
        let sender1 = SenderEmu::new(&crt, channel_id, &mut sender1);
        let sender2 = SenderEmu::new(&crt, channel_id, &mut sender2);
        let mut recver = RecverEmu::new(&crt, channel_id, &mut recver);

        recver.register();
        assert!(crt.get_awake_event_id().is_none());

        sender1.register();
        sender2.register();

        unsafe {
            // receiver awoken and have got the right value after exchange
            recver.assert_completion(crt.get_awake_event_id(), SwapResult::Done, &Some(100));
            recver.clear_storage();

            // verifies that sender is awoken and had value taken out after exchange
            sender1.assert_completion(crt.get_awake_event_id(), SwapResult::Done, &None);

            // prepare sender once again
            recver.register();

            // receiver awoken and have got the right value after exchange
            recver.assert_completion(crt.get_awake_event_id(), SwapResult::Done, &Some(50));

            // verifies that sender is awoken and had value taken out after exchange
            sender2.assert_completion(crt.get_awake_event_id(), SwapResult::Done, &None);
        }
    }

    /// If we cancel pinned/Emptied sender the receiver is ok and can receive further
    /// values from senders.
    #[test]
    fn api_test_cancel_exchanged_sender_does_not_affect_receiver() {
        let crt = ChannelRt::new(&Tracer::new_testing());

        // storage for exchange
        let mut sender1: Option<u32> = Some(100);
        let mut sender2: Option<u32> = Some(50);
        let mut recver: Option<u32> = None;

        let channel_id = crt.create();

        // Hide the storage variable above to avoid having multiple mutable references
        // to the same object.
        let sender1 = SenderEmu::new(&crt, channel_id, &mut sender1);
        let sender2 = SenderEmu::new(&crt, channel_id, &mut sender2);
        let mut recver = RecverEmu::new(&crt, channel_id, &mut recver);

        recver.register();
        assert!(crt.get_awake_event_id().is_none());

        sender1.register();
        sender2.register();

        unsafe {
            // receiver awoken and have got the right value after exchange
            recver.assert_completion(crt.get_awake_event_id(), SwapResult::Done, &Some(100));
            recver.clear_storage();

            sender1.cancel();
            drop(sender1);

            // prepare sender once again
            recver.register();

            // receiver awoken and have got the right value after exchange
            recver.assert_completion(crt.get_awake_event_id(), SwapResult::Done, &Some(50));

            // verifies that sender is awoken and had value taken out after exchange
            sender2.assert_completion(crt.get_awake_event_id(), SwapResult::Done, &None);
        }
    }

    /// With canceling the scheduled sender receiver still receives values from another senders.
    #[test]
    fn api_test_cancel_scheduled_sender_recv_still_receive_values_from_another_senders() {
        let crt = ChannelRt::new(&Tracer::new_testing());

        // storage for exchange
        let mut sender1: Option<u32> = Some(100);
        let mut sender2: Option<u32> = Some(50);
        let mut recver: Option<u32> = None;

        let channel_id = crt.create();

        // Hide the storage variable above to avoid having multiple mutable references
        // to the same object.
        let sender1 = SenderEmu::new(&crt, channel_id, &mut sender1);
        let sender2 = SenderEmu::new(&crt, channel_id, &mut sender2);
        let mut recver = RecverEmu::new(&crt, channel_id, &mut recver);

        recver.register();
        assert!(crt.get_awake_event_id().is_none());

        sender1.register();
        sender2.register();

        unsafe {
            // receiver awoken and have got the right value after exchange
            recver.assert_completion(crt.get_awake_event_id(), SwapResult::Done, &Some(100));
            recver.clear_storage();

            // cancel and drop the queued sender [Exch, Pin <- this one]
            sender2.cancel();
            drop(sender2);

            // verifies that sender is awoken and had value taken out after exchange
            sender1.assert_completion(crt.get_awake_event_id(), SwapResult::Done, &None);
        }
    }
}
