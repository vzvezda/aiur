//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::cell::Cell;
use std::cell::RefCell;
use std::future::Future;

use crate::channel_rt::ChannelRt;
use crate::event_node::EventNode;
use crate::oneshot_rt::OneshotRt;
use crate::pin_local;
use crate::reactor::{EventId, Reactor};
use crate::task::{ITask, Task};
use crate::tracer::Tracer;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

/// The owner of the reactor (I/O event queue) and executor (task management) data structures.
pub struct Runtime<ReactorT> {
    reactor: ReactorT,
    awoken_event_id: Cell<EventId>,
    oneshot_rt: OneshotRt,
    channel_rt: ChannelRt,
    frozen_list: RefCell<EventNode>, // can we have cell here?
    tracer: Tracer,
}

impl<ReactorT> Runtime<ReactorT>
where
    ReactorT: Reactor,
{
    pub(crate) fn new(reactor: ReactorT, tracer: Tracer) -> Self {
        Self {
            reactor,
            awoken_event_id: Cell::new(EventId::null()),
            oneshot_rt: OneshotRt::new(&tracer),
            channel_rt: ChannelRt::new(&tracer),
            frozen_list: RefCell::new(EventNode::new()),
            tracer,
        }
    }

    pub(crate) fn oneshots(&self) -> &OneshotRt {
        &self.oneshot_rt
    }

    pub(crate) fn channels(&self) -> &ChannelRt {
        &self.channel_rt
    }

    pub(crate) fn jump_phase(&self) {
        self.oneshot_phase();
        self.channel_phase();
    }

    fn channel_phase(&self) {
        // do the channel exchange until there is no more channels
        loop {
            if let Some(event_id) = self.channels().get_awake_event_id() {
                let awoken_task = event_id.as_event_node().get_itask_ptr();
                self.awoken_event_id.set(event_id);
                unsafe { (*awoken_task).poll() };
            } else {
                break;
            }
        }
    }

    fn oneshot_phase(&self) {
        // do the channel exchange until there is no more oneshots
        loop {
            if let Some(event_id) = self.oneshots().get_awake_event_id() {
                let awoken_task = event_id.as_event_node().get_itask_ptr();
                self.awoken_event_id.set(event_id);
                unsafe { (*awoken_task).poll() };
            } else {
                break;
            }
        }
    }

    pub(crate) fn tracer(&self) -> &Tracer {
        &self.tracer
    }

    fn wait(&self) -> *const dyn ITask {
        // loop because that event from reactor may come for a frozen task
        loop {
            // Waiting for an event from reactor. The itask pointer of the task in the awoken is
            // saved by Waker.wake().
            let event_id = self.io().wait();
            let itask_ptr = event_id.as_event_node().get_itask_ptr();

            unsafe {
                if (*itask_ptr).is_frozen() {
                    // we cannot poll the task because it is frozen. Save the event somewhere.
                    self.save_event_for_frozen_task(event_id);
                    continue; // have to wait for another task
                } else {
                    // Save the event_id to awoken.
                    self.awoken_event_id.set(event_id);

                    // return task pointer to root task or first unfrozen ancestor
                    break (*itask_ptr).unfrozen_ancestor();
                }
            }
        }
    }

    // Saves event from reactor for later
    fn save_event_for_frozen_task(&self, event_id: EventId) {
        unsafe { self.frozen_list.borrow_mut().push_back(event_id) };
    }

    // Polls if there is something unfrozen in the list of frozen events
    fn poll_unfrozen(&self) {
        // loop until there is something we can find in the list of frozen events
        while let Some(unfrozen) = self.find_unfrozen_event() {
            self.awoken_event_id.set(unfrozen);
            unsafe {
                let itask_ptr = unfrozen.as_event_node().get_itask_ptr();
                (*(*itask_ptr).unfrozen_ancestor()).poll();
            }
        }
    }

    // Scans the frozen events array if there is an event for task that unfrozen right now.
    // Removes such event from frozen_events array.
    fn find_unfrozen_event(&self) -> Option<EventId> {
        let mut list = self.frozen_list.borrow_mut();
        unsafe { list.find_unfrozen().map(|event_id| event_id) }
    }

    //
    pub fn nested_loop<FutureT, ResultT>(&self, future: FutureT) -> ResultT
    where
        FutureT: Future<Output = ResultT>,
    {
        // Put a future in a task and pin the task
        let task = Task::new(future);
        pin_local!(task);
        task.on_pinned(); // assign self-references after pinning

        // Forget &mut Task here and user only &task
        let task = task.as_ref();

        modtrace!(self.tracer(), "runtime: nested loop for task");

        // Polls the future once to give it chance to schedule its i/o in reactor. It
        // is possible that this poll() call would make some other nested_loop().
        task.poll();

        while !task.is_completed() {
            self.poll_unfrozen();

            if task.is_completed() {
                break;
            }

            self.jump_phase();

            if task.is_completed() {
                break;
            }

            // Await the reactor i/o
            let awoken_task = self.wait();
            unsafe { (*awoken_task).poll() };
        }

        // todo: remove this task from frozen events if any

        modtrace!(self.tracer(), "runtime: exit nested loop for the task");
        task.take_result()
    }

    /// Used by a leaf feature in poll() method to verify if it was the reason it was awoken.
    pub fn is_awoken_for(&self, event_id: EventId) -> bool {
        self.awoken_event_id.get() == event_id
    }

    /// Returns reference to reactor.
    pub fn io(&self) -> &ReactorT {
        &self.reactor
    }
}
