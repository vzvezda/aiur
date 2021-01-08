//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::cell::{Cell, RefCell};
use std::future::Future;

use crate::reactor::{EventId, Reactor};
use crate::task::{allocate_void_task, construct_task, Completion, ITask};

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
    spawn_list: RefCell<Vec<*mut (dyn ITask + 'static)>>,
    active_tasks: Cell<u32>,
}

impl TaskMaster {
    fn new() -> Self {
        TaskMaster {
            spawn_list: RefCell::new(Vec::new()),
            active_tasks: Cell::new(0),
        }
    }

    fn add_task_for_spawn<'lifetime>(&self, task_ptr: *mut (dyn ITask + 'lifetime)) {
        // TODO: justify the unsafe
        let task_ptr = unsafe {
            std::mem::transmute::<*mut (dyn ITask + '_), *mut (dyn ITask + 'static)>(task_ptr)
        };

        self.spawn_list.borrow_mut().push(task_ptr);
    }

    fn has_tasks(&self) -> bool {
        self.active_tasks.get() > 0 || !self.spawn_list.borrow().is_empty()
    }

    fn has_scheduled_tasks(&self) -> bool {
        self.active_tasks.get() > 0
    }

    fn inc_tasks(&self) {
        self.active_tasks.set(self.active_tasks.get() + 1);
    }

    fn dec_tasks(&self) {
        self.active_tasks.set(self.active_tasks.get() - 1);
    }

    pub fn swap_spawn_queue(&self, vec: &mut Vec<*mut dyn ITask>) {
        vec.clear();
        std::mem::swap(&mut *self.spawn_list.borrow_mut(), vec);
    }
}

//
//
pub struct Runtime<ReactorT> {
    reactor: ReactorT,
    awoken: Awoken,
    task_master: TaskMaster,
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
            self.spawn_phase();
            self.poll_phase();
        }

        unsafe { result.assume_init() }
    }

    // Adds a new future as a task in a new list that is to be spawn on a spawn_phase
    pub(crate) fn spawn<'runtime, F>(&'runtime self, f: F) -> *mut (dyn ITask + 'runtime)
    where
        F: Future<Output = ()> + 'runtime,
    {
        let task = allocate_void_task(&self.awoken, f);

        /*
        // ok, we have this unsafe
        let task = unsafe {
            std::mem::transmute::<*mut (dyn ITask + 'runtime), *mut (dyn ITask + 'static)>(task)
        };
        */

        self.task_master.add_task_for_spawn(task);
        task
    }

    //
    fn get_queue(&self, mut spawn_queue: &mut Vec<*mut dyn ITask>) {
        self.task_master.swap_spawn_queue(&mut spawn_queue);
    }

    //
    pub(crate) fn spawn_phase(&self) {
        let mut spawn_queue = Vec::new();

        self.get_queue(&mut spawn_queue);
        println!("spawn phase queue size {}", spawn_queue.len());

        // this is the spawn block: poll any futures that were "spawn"
        spawn_queue.drain(..).for_each(|itask_ptr| {
            println!("Started spawn future {:?}", itask_ptr);
            let completed = unsafe {
                (*itask_ptr).on_pinned();
                (*itask_ptr).poll()
            };
            // TODO: fill the itask_ptr for waker

            // Make a poll and inc the counter
            if completed == Completion::Working {
                self.task_master.inc_tasks();
            } else {
                unsafe { (*itask_ptr).on_completed(); }
            }
        });
    }

    // Waits for a signal from reactor and then 
    pub(crate) fn poll_phase(&self) -> Option<*mut dyn ITask> {
        if !self.task_master.has_scheduled_tasks() {
            println!("Poll phase without tasks");
            // it happens: we have tasks in spawn list, but nothing to wait in reactor
            return None;
        }

        // Poll the awaken task
        let awoken_task = self.wait();

        if unsafe { (*awoken_task).poll() } == Completion::Done {
            println!("Completed task");
            unsafe { (*awoken_task).on_completed(); }
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

    pub fn io(&self) -> &ReactorT {
        &self.reactor
    }
}
