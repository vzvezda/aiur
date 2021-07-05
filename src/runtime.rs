//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::Future;

use crate::channel_rt::ChannelRt;
use crate::oneshot_rt::OneshotRt;
use crate::reactor::{EventId, Reactor};
use crate::task::{allocate_void_task, construct_task, Completion, ITask};

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

// This part of the runtime that maintain info about active tasks.
struct TaskMaster {
    spawn_list: RefCell<VecDeque<*mut (dyn ITask + 'static)>>,
    active_tasks: Cell<u32>,
}

impl TaskMaster {
    fn new() -> Self {
        TaskMaster {
            spawn_list: RefCell::new(VecDeque::new()),
            active_tasks: Cell::new(0),
        }
    }

    fn add_task_for_spawn<'lifetime>(&self, task_ptr: *mut (dyn ITask + 'lifetime)) {
        // TODO: justify the unsafe
        let task_ptr = unsafe {
            std::mem::transmute::<*mut (dyn ITask + '_), *mut (dyn ITask + 'static)>(task_ptr)
        };

        self.spawn_list.borrow_mut().push_back(task_ptr);
    }

    fn pop_task(&self) -> Option<*mut (dyn ITask + 'static)> {
        self.spawn_list.borrow_mut().pop_front()
    }

    fn has_tasks(&self) -> bool {
        self.active_tasks.get() > 0 || !self.spawn_list.borrow().is_empty()
    }

    fn has_scheduled_tasks(&self) -> bool {
        self.active_tasks.get() > 0
    }

    fn inc_tasks(&self) {
        let old_tasks = self.active_tasks.get();
        let new_tasks = old_tasks + 1;
        modtrace!("TaskMaster: inc tasks {} -> {}", old_tasks, new_tasks);
        self.active_tasks.set(new_tasks);
    }

    fn dec_tasks(&self) {
        let old_tasks = self.active_tasks.get();
        let new_tasks = old_tasks - 1;
        modtrace!("TaskMaster: dec tasks {} -> {}", old_tasks, new_tasks);
        self.active_tasks.set(self.active_tasks.get() - 1);
    }
}

/// The owner of the reactor (I/O event queue) and executor (task management) data structures.
pub struct Runtime<ReactorT> {
    reactor: ReactorT,
    awoken: Awoken,
    task_master: TaskMaster,
    oneshot_rt: OneshotRt,
    channel_rt: ChannelRt,
}

impl<ReactorT> Runtime<ReactorT>
where
    ReactorT: Reactor,
{
    pub(crate) fn new(reactor: ReactorT) -> Self {
        Self {
            reactor,
            awoken: Awoken::new(),
            task_master: TaskMaster::new(),
            oneshot_rt: OneshotRt::new(),
            channel_rt: ChannelRt::new(),
        }
    }

    // This is the actual execution and poll loop for with_runtime() method.
    pub(crate) fn block_on<'runtime, ResT: 'runtime, FutureT: 'runtime>(
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

        // TODO: Do not use the vector for just self task?
        self.task_master.add_task_for_spawn(task_ptr);

        // while self.executor.borrow().is_active() || self.executor.borrow().has_tasks_to_spawn() {
        while self.task_master.has_tasks() {
            // TODO: repeat spawn/channel until done before proceed to poll
            self.spawn_phase();
            self.jump_phase();
            self.poll_phase();
        }

        unsafe { result.assume_init() }
    }

    pub(crate) fn oneshots(&self) -> &OneshotRt {
        &self.oneshot_rt
    }

    pub(crate) fn channels(&self) -> &ChannelRt {
        &self.channel_rt
    }

    // Adds a new future as a task in a new list that is to be spawn on a spawn_phase
    pub(crate) fn spawn<'runtime, 'scope, F>(&'runtime self, f: F) -> *mut (dyn ITask + 'static)
    where
        F: Future<Output = ()> + 'runtime,
    {
        let task = allocate_void_task(&self.awoken, f);

        self.task_master.add_task_for_spawn(task);

        // ok, we have this unsafe
        let task = unsafe {
            std::mem::transmute::<*mut (dyn ITask + 'runtime), *mut (dyn ITask + 'static)>(task)
        };

        task
    }

    pub(crate) fn jump_phase(&self) {
        self.oneshot_phase();
        self.channel_phase();
    }

    fn channel_phase(&self) {
        // do the channel exchange until there is no more oneshots
        loop {
            // TODO: this is a copy/paste of poll_phase code, we need to unify this

            if let Some(event_id) = self.channels().awake_and_get_event_id() {
                let awoken_task = self.awoken.itask_ptr.get().unwrap();
                self.awoken.event_id.set(event_id);

                if unsafe { (*awoken_task).poll() } == Completion::Done {
                    unsafe {
                        (*awoken_task).on_completed();
                    }
                    self.task_master.dec_tasks();
                    // TODO: deallocate if &addr != &task
                } else {
                }
            } else {
                break;
            }
        }
    }

    fn oneshot_phase(&self) {
        // do the channel exchange until there is no more oneshots
        loop {
            // TODO: this is a copy/paste of poll_phase code, we need to unify this

            if let Some(event_id) = self.oneshots().awake_and_get_event_id() {
                let awoken_task = self.awoken.itask_ptr.get().unwrap();
                self.awoken.event_id.set(event_id);

                if unsafe { (*awoken_task).poll() } == Completion::Done {
                    unsafe {
                        (*awoken_task).on_completed();
                    }
                    self.task_master.dec_tasks();
                    // TODO: deallocate if &addr != &task
                } else {
                }
            } else {
                break;
            }
        }
    }

    //
    //
    pub(crate) fn spawn_phase(&self) {
        loop {
            let itask_ptr = self.task_master.pop_task();
            if itask_ptr.is_none() {
                modtrace!("Rt: spawn phase - no tasks to spawn");
                return;
            }

            let itask_ptr = itask_ptr.unwrap();
            modtrace!("Rt: spawn phase - found a task {:?} to spawn", itask_ptr);

            let completed = unsafe {
                (*itask_ptr).on_pinned();
                (*itask_ptr).poll()
            };

            // Make a poll and inc the counter
            if completed == Completion::Working {
                self.task_master.inc_tasks();
            } else {
                unsafe {
                    (*itask_ptr).on_completed();
                }
            }
        }
    }

    // Waits for a signal from reactor and then
    pub(crate) fn poll_phase(&self) -> Option<*mut dyn ITask> {
        if !self.task_master.has_scheduled_tasks() {
            // it happens: we have tasks in spawn list, but nothing to wait in reactor
            modtrace!("Rt: poll phase no tasks in reactor (will check spawn list or channels)");
            return None;
        }

        // Poll the awaken task
        let awoken_task = self.wait();

        if unsafe { (*awoken_task).poll() } == Completion::Done {
            unsafe {
                (*awoken_task).on_completed();
            }
            self.task_master.dec_tasks();
            // TODO: deallocate if &addr != &task
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

    /// Returns reference to reactor.
    pub fn io(&self) -> &ReactorT {
        &self.reactor
    }
}
