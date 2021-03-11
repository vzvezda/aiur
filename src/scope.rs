//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use crate::runtime::Runtime;
use crate::reactor::Reactor;
use crate::task::ITask;

// enable/disable output of modtrace! macro
const MODTRACE: bool = true;

pub struct Scope<'runtime, ReactorT> where ReactorT: Reactor {
    rt: &'runtime Runtime<ReactorT>,
    tasks: std::cell::RefCell<Vec<*mut (dyn ITask + 'runtime)>>,
    //tasks: Vec<*mut (dyn ITask + 'runtime)>,

    // need a better storage
    name: String,

    // there is also an idea that we can avoid dynamic memory and use a list of tasks
    // interconnected to each other.
    // task_list: Option<*mut (dyn ITask + 'runtime)>,
}
/*
pub struct JoinHandle<'scope, 'runtime, ReactorT> where ReactorT: Reactor {
    scope: &'scope Scope<'runtime, ReactorT>,
    task: *mut (dyn ITask + 'scope)
}
*/

impl<'runtime, ReactorT> Scope<'runtime, ReactorT> where ReactorT: Reactor {
    pub fn new(rt: &'runtime Runtime<ReactorT>) -> Self {
        Self::new_named(rt, "")
    }

    pub fn new_named<'name>(rt: &'runtime Runtime<ReactorT>, name: &'name str) -> Self {
        Scope {
            rt,
            //tasks: Vec::new(),
            tasks: std::cell::RefCell::new(Vec::new()),
            name: name.to_string(),
        }
    }

    // Does it make sense to use RefCell and not mut ref? The cope is used for channels,
    // but perhaps we can use runtime for channels.\
    // But haven't we had a plan to have a Pin<&mut self>?
    pub fn spawn<'scope, FutureT>(&'scope /*mut */self, future: FutureT) 
    where
        FutureT: std::future::Future<Output = ()> + 'runtime,
    {
        let task: *mut (dyn ITask + 'runtime) = self.rt.spawn(future);
        self.tasks.borrow_mut().push(task);
        //self.tasks.push(task);
        modtrace!("Scope: scope '{}' to spawn task {:?}", self.name, task);
        //JoinHandle { scope: &self, task }
    }

}

impl<'runtime, ReactorT> Drop for Scope<'runtime, ReactorT> where ReactorT: Reactor {
    fn drop(&mut self) {
        modtrace!("Scope: <<<< Entering the poll loop in Scope('{}')::drop()", self.name);
        while self.tasks.borrow_mut().iter().any(|itask_ptr| {
        //while self.tasks.iter().any(|itask_ptr| {
            unsafe {
                !(**itask_ptr).is_completed()
            }
        }) {
            modtrace!("Scope: poll loop in Scope('{}')", self.name);
            self.rt.spawn_phase();
            self.rt.channel_phase();
            self.rt.poll_phase();
        }

        modtrace!("Scope: >>>> Left the poll loop in Scope('{}')::drop(), scope dropped", 
            self.name);
    }
}
