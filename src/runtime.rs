//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::cell::Cell;

use crate::reactor::{EventId, Reactor};
use crate::task::{ITask, Completion, construct_task};

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

struct TaskMaster {
}

impl TaskMaster {
    fn new() -> Self {
        TaskMaster {}
    }
}

pub struct Runtime<ReactorT> {
    reactor: ReactorT,
    awoken: Awoken,
    _task_master: TaskMaster,
}

impl<ReactorT> Runtime<ReactorT>
where
    ReactorT: Reactor,
{
    pub(crate) fn new(reactor: ReactorT) -> Self {
        Self {
            reactor,
            awoken: Awoken::new(),
            _task_master: TaskMaster::new(),
        }
    }

    pub(crate) fn block_on<'runtime, ResT: 'static, FutureT: 'runtime>(
        &'runtime self,
        future: FutureT,
    ) -> ResT
    where
        FutureT: Future<Output = ResT> + 'runtime,
    {
        let mut result = std::mem::MaybeUninit::<ResT>::uninit();
        let mut task_body = construct_task(&self.awoken, future, result.as_mut_ptr()); 
        // TODO: pin
        let task_ptr = &mut task_body as *mut (dyn ITask + 'runtime);
        task_body.on_pinned();

        loop {
            if unsafe { (*task_ptr).poll() } == Completion::Done {
                break
            }

            self.wait();
        }

        /*

        while something {
            self.spawn_phase();
            let _completed_task = self.poll_phase();
            // TODO: deallocate if &addr != &task
        }
        */

        unsafe { result.assume_init() }

        /*

        // ok, we have this unsafe
        let task = unsafe {
            std::mem::transmute::<*mut (dyn ITask + '_), *mut (dyn ITask + 'static)>(&mut task)
        };
        // TODO: Do not use the vector for just self task?
        self.executor.borrow_mut().task_spawned(task);

        while self.executor.borrow().is_active() || self.executor.borrow().has_tasks_to_spawn() {
            self.spawn_phase();

            // Poll the awaken task
            self.poll_phase();
            // TODO: deallocate if &addr != &task
        }

        unsafe { result.assume_init() }
            */
    }

    // 
    pub(crate) fn spawn_phase(&self) {
    }

    // Waits for the 
    pub(crate) fn poll_phase(&self) -> Option<*mut dyn ITask> {
        // Poll the awaken task
        let awoken_task = self.wait();

        if unsafe { (*awoken_task).poll() } == Completion::Done {
            Some(awoken_task)
        } else {
            None
        }
    }

    fn wait(&self) -> *mut dyn ITask {
        // Waiting for an event from reactor and save the EventId into awoken. The
        // itask pointer in the awoken is saved by Waker.wake().
        self.awoken.set_awoken_event(self.io().wait());

        // return task pointer
        self.awoken.itask_ptr.get().unwrap()
    }

    pub(crate) fn is_awoken(&self, event_id: EventId) -> bool {
        self.awoken.event_id.get() == event_id
    }

    pub fn io(&self) -> &ReactorT {
        &self.reactor
    }
}
