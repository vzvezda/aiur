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

    // need a better storage
    name: String,

    // there is also an idea that we can avoid dynamic memory and use a list of tasks
    // interconnected to each other.
    // task_list: Option<*mut (dyn ITask + 'runtime)>,
}

impl<'runtime, ReactorT> Scope<'runtime, ReactorT> where ReactorT: Reactor {
    pub fn new(rt: &'runtime Runtime<ReactorT>) -> Self {
        Scope {
            rt,
            tasks: Vec::new(),
            name: String::new(),
        }
    }

    pub fn new_named<'name>(rt: &'runtime Runtime<ReactorT>, name: &'name str) -> Self {
        Scope {
            rt,
            tasks: Vec::new(),
            name: name.to_string(),
        }
    }

    pub fn spawn<'scope, FutureT>(&'scope mut self, future: FutureT)
    where
        FutureT: std::future::Future<Output = ()> + 'runtime,
    {
        self.tasks.push(self.rt.spawn(future));
        println!("task {} has been spawn", self.name);
    }

}

impl<'runtime, ReactorT> Drop for Scope<'runtime, ReactorT> where ReactorT: Reactor {
    fn drop(&mut self) {
        println!("<<<< Entering scope {}", self.name);
        while self.tasks.iter().any(|itask_ptr| {
            unsafe {
                !(**itask_ptr).is_completed()
            }
        }) {
            println!("Scope loop {}", self.name);
            self.rt.spawn_phase();
            self.rt.poll_phase();
        }

        println!(">>>> Leaving scope {}", self.name);
    }
}
