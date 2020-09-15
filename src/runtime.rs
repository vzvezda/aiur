//  ( /   @ @    ()  Aiur - the home planet of some famous executors
//   \  __| |__  /   (c) 2020 - present, Vladimir Zvezda
//    -/   "   \-
use std::future::Future;

// A helper trait to solve the lifetime problem for with_runtime().
// See https://stackoverflow.com/questions/63517250/specify-rust-closures-lifetime
pub trait LifetimeLinkerFn<'a, R> {
    type OutputFuture: Future<Output = R> + 'a;
    fn call(self, arg: &'a Runtime) -> Self::OutputFuture;
}

// for all F:FnOnce(&'a Runtime)->impl Future<Output=R> + a define the
// LifetimeLinkerFn.
impl<'a, R, Fut: 'a, Func> LifetimeLinkerFn<'a, R> for Func
where
    Func: FnOnce(&'a Runtime) -> Fut,
    Fut: Future<Output = R> + 'a,
{
    type OutputFuture = Fut;
    fn call(self, rt: &'a Runtime) -> Fut {
        self(rt)
    }
}

/// Runs the async function and polls the returned future under aiur runtime. The function is
/// something like `async fn foo(rt: &Runtime) -> Result`. Note, that
/// it currently does not work with the closures because of lifetime elision problem.
pub fn with_runtime<C, R>(async_function: C) -> R
where
    C: for<'a> LifetimeLinkerFn<'a, R>, // async fn foo(rt: &Runtime) -> R
{
    let runtime = Runtime::new();
    let future = async_function.call(&runtime);

    runtime.block_on(future)
}

pub struct Runtime {
}

/// What is supposed to be a runtime:
/// * API for leaf Futures (like I/O)
/// * Use with_runtime(|rt| { }) to execute a code with runtime
/// * Spawn API
impl Runtime {
    fn new() -> Self {
        Runtime {
        }
    }

    // The block_on is private
    fn block_on<'runtime, F, R>(&'runtime self, _future: F) -> R
    where
        F: Future<Output = R> + 'runtime,
    {
        // no poll ipmlementation yet
        todo!()
    }
}
