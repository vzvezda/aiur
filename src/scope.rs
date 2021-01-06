//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use crate::runtime::Runtime;
use crate::reactor::Reactor;
use crate::task::ITask;

pub struct Scope<'runtime, ReactorT> where ReactorT: Reactor {
    rt: &'runtime Runtime<ReactorT>,
    tasks: Vec<*mut (dyn ITask + 'runtime)>,

    // there is also an idea that we can avoid dynamic memory and use a list of tasks
    // interconnected to each other.
    // task_list: Option<*mut (dyn ITask + 'runtime)>,
}

impl<'runtime, ReactorT> Scope<'runtime, ReactorT> where ReactorT: Reactor {
    pub fn new<'name>(rt: &'runtime Runtime<ReactorT>) -> Self {
        Scope {
            rt,
            tasks: Vec::new(),
        }
    }

    pub fn spawn<'scope, FutureT>(&'scope mut self, future: FutureT)
    where
        FutureT: std::future::Future<Output = ()> + 'runtime,
    {
        self.tasks.push(self.rt.spawn(future));
        println!("task has been spawn");
    }

}

impl<'runtime, ReactorT> Drop for Scope<'runtime, ReactorT> where ReactorT: Reactor {
    fn drop(&mut self) {
        while self.tasks.iter().any(|itask_ptr| {
            unsafe {
                !(**itask_ptr).is_completed()
            }
        }) {
            self.rt.spawn_phase();
            let task = self.rt.poll_phase();
            if task.is_some() {
                 unsafe { (*task.unwrap()).on_completed(); }
            }
        }

        println!("Finish dropping scope");
    }
}
