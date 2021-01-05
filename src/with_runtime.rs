//    ^
//  }/_\{   (c) 2020-present Vladimir Zvezda
//  |\ /|
//    v
use std::future::Future;

use crate::Reactor;
use crate::Runtime;

// [1] - improvement to this machinery to support reference in input and return type
//       of async function supplied to with_runtime() function. So with this improvement
//       it is now possible to use with with_runtime like this:
//
//       async fn my_main<'b>(rt: &Runtime, init: &'b u32) -> &'b u32 { todo!() }

// A helper trait to solve the lifetime problem for with_runtime().
// See https://stackoverflow.com/questions/63517250/specify-rust-closures-lifetime
pub trait LifetimeLinkerFn<'runtime, ReactorT, InitT, ResT> {
    type OutputFuture: Future<Output = ResT>; // +'runtime; commented because [1]

    fn call(self, arg: &'runtime Runtime<ReactorT>, init: InitT) -> Self::OutputFuture;
}

// for all F:FnOnce(&'runtime Runtime)->impl Future<Output=ResT> + a define the
// LifetimeLinkerFn.
impl<'runtime, ReactorT, InitT, ResT, FutureT, FuncT>
    LifetimeLinkerFn<'runtime, ReactorT, InitT, ResT> for FuncT
where
    FuncT: FnOnce(&'runtime Runtime<ReactorT>, InitT) -> FutureT,
    FutureT: Future<Output = ResT>,
    //    FutureT: 'runtime, // +'runtime, commented because [1]
    ReactorT: Reactor + 'runtime, // +'runtime is required (won't compile)
{
    type OutputFuture = FutureT;

    fn call(self, rt: &'runtime Runtime<ReactorT>, init: InitT) -> FutureT {
        self(rt, init)
    }
}

// This is how you start a async function with aiur. The idea that reactor crate wrap this method
// into another one.
pub fn with_runtime_base<ReactorT, FuncT, InitT, ResT>(
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
