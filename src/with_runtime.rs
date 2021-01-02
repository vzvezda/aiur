//    ^
//  }/_\{   (c) 2020-present Vladimir Zvezda
//  |\ /|      
//    v
use std::future::Future;

use crate::Runtime;
use crate::Reactor;

// A helper trait to solve the lifetime problem for with_runtime().
// See https://stackoverflow.com/questions/63517250/specify-rust-closures-lifetime
pub trait LifetimeLinkerFn<'runtime, ReactorT, InitT, ResT: 'static> {
    type OutputFuture: Future<Output = ResT> + 'runtime;

    fn call(self, arg: &'runtime Runtime<ReactorT>, init: InitT) -> Self::OutputFuture;
}

// for all F:FnOnce(&'runtime Runtime)->impl Future<Output=ResT> + a define the
// LifetimeLinkerFn.
impl<'runtime, ReactorT, InitT, ResT: 'static, FutureT: 'runtime, FuncT> 
LifetimeLinkerFn<'runtime, ReactorT, InitT, ResT> for FuncT
where
    FuncT: FnOnce(&'runtime Runtime<ReactorT>, InitT) -> FutureT,
    FutureT: Future<Output = ResT> + 'runtime,
    ReactorT: Reactor + 'runtime,
{
    type OutputFuture = FutureT;

    fn call(self, rt: &'runtime Runtime<ReactorT>, init: InitT) -> FutureT {
        self(rt, init)
    }
}

// This is how you start a async function with aiur. The idea that reactor crate wrap this method
// into another one.
pub fn with_runtime_base<ReactorT, FuncT, InitT, ResT: 'static>(
    reactor: ReactorT,
    async_function: FuncT,
    init: InitT,
) -> ResT
where
    // async fn foo(rt: &Runtime, param: ParamT) -> ResT
    FuncT: for<'runtime> LifetimeLinkerFn<'runtime, ReactorT, InitT, ResT>,
    ReactorT: Reactor,
{
    let runtime = Runtime::<ReactorT>::new(reactor);
    let future = async_function.call(&runtime, init);

    // return the result of the execution of the future
    runtime.block_on(future)
}
