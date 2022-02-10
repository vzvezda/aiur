//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::Future;

use crate::channel_rt::ChannelRt;
use crate::oneshot_rt::OneshotRt;
use crate::pin_local;
use crate::reactor::{EventId, Reactor};
use crate::task::{ITask, Task};
use crate::tracer::Tracer;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

// Info about what the reactor was awoken from: the task and the event.
pub(crate) struct Awoken {
    itask_ptr: Cell<Option<*mut dyn ITask>>,
    event_id: Cell<EventId>,
}

impl Awoken {
    fn new() -> Self {
        Awoken {
            itask_ptr: Cell::new(None),
            event_id: Cell::new(EventId::null()),
        }
    }

    pub(crate) fn set_awoken_task(&self, itask: *mut dyn ITask) {
        self.itask_ptr.set(Some(itask));
    }

    pub(crate) fn set_awoken_event(&self, event_id: EventId) {
        self.event_id.set(event_id);
    }
}

/// The owner of the reactor (I/O event queue) and executor (task management) data structures.
pub struct Runtime<ReactorT> {
    reactor: ReactorT,
    awoken: Awoken,
    oneshot_rt: OneshotRt,
    channel_rt: ChannelRt,
    tracer: Tracer,
}

impl<ReactorT> Runtime<ReactorT>
where
    ReactorT: Reactor,
{
    pub(crate) fn new(reactor: ReactorT, tracer: Tracer) -> Self {
        Self {
            reactor,
            awoken: Awoken::new(),
            oneshot_rt: OneshotRt::new(&tracer),
            channel_rt: ChannelRt::new(&tracer),
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
            if let Some(event_id) = self.channels().awake_and_get_event_id() {
                let awoken_task = self.awoken.itask_ptr.get().unwrap();
                self.awoken.event_id.set(event_id);
                unsafe { (*awoken_task).poll() };
            } else {
                break;
            }
        }
    }

    fn oneshot_phase(&self) {
        // do the channel exchange until there is no more oneshots
        loop {
            if let Some(event_id) = self.oneshots().awake_and_get_event_id() {
                let awoken_task = self.awoken.itask_ptr.get().unwrap();
                self.awoken.event_id.set(event_id);
                unsafe { (*awoken_task).poll() };
            } else {
                break;
            }
        }
    }

    pub(crate) fn tracer(&self) -> &Tracer {
        &self.tracer
    }

    fn wait(&self) -> *mut dyn ITask {
        // Waiting for an event from reactor and save the EventId into awoken. The
        // itask pointer in the awoken is saved by Waker.wake().
        self.awoken.set_awoken_event(self.io().wait());

        // return task pointer
        self.awoken.itask_ptr.get().unwrap()
    }

    fn poll_unfrozen(&self) {
        // todo: find if there is an unfrozen task and poll it
    }

    // Experemental
    pub fn nested_loop<FutureT, ResultT>(&self, future: FutureT) -> ResultT
    where
        FutureT: Future<Output = ResultT>,
    {
        // Put a future in a task and pin the task
        let task = Task::new(future, &self.awoken);
        pin_local!(task);

        modtrace!(self.tracer(), "runtime: nested loop for task");

        // Prepare the task once it pinned and polls the future once to give it chance
        // to schedule its i/o in reactor. It is possible that start() would make
        // some other nested_loop().
        task.start();

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
    pub fn is_awoken(&self, event_id: EventId) -> bool {
        self.awoken.event_id.get() == event_id
    }

    /// Returns reference to reactor.
    pub fn io(&self) -> &ReactorT {
        &self.reactor
    }
}
