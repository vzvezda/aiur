//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
mod reactor;
mod runtime;
mod scope;
mod task;
mod timer;
mod with_runtime;

pub mod toy_rt;

pub use reactor::{EventId, GetEventId, Reactor, TemporalReactor};
pub use runtime::Runtime;
pub use scope::Scope;
pub use timer::sleep;
pub use toy_rt::ToyReactor;
pub use with_runtime::{with_runtime_base, LifetimeLinkerFn};

// This is a help macro to create API for your own runtime based on re-exporting aiur runtime
// and specialize it with your reactor.
#[macro_export]
macro_rules! export_runtime {
    ($reactor:ident) => {
        pub type Runtime = $crate::Runtime<$reactor>;
        pub type Scope<'runtime> = $crate::Scope<'runtime, $reactor>;
        pub type EventId = $crate::EventId;
        pub use $crate::GetEventId;
        pub use $crate::sleep;

        pub fn with_runtime<ReactorFn, FuncT, InitT, ResT: 'static>(
            reactor_constructor: ReactorFn,
            async_function: FuncT,
            init: InitT,
        ) -> ResT
        where
            // async fn foo(rt: &Runtime, param: ParamT) -> ResT
            FuncT: for<'runtime> $crate::LifetimeLinkerFn<'runtime, $reactor, InitT, ResT>,
            ReactorFn: FnOnce () -> $reactor
        {
            $crate::with_runtime_base(reactor_constructor(), async_function, init)
        }


    };
}
