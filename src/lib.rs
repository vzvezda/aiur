//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
#[macro_use]
mod modtrace_macro;

mod oneshot;
mod reactor;
mod runtime;
mod scope;
mod task;
mod timer;
mod with_runtime;
mod oneshot_rt;

pub mod toy_rt;

pub use oneshot::oneshot;
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
        pub use $crate::sleep;
        pub use $crate::GetEventId;


        pub type Receiver<'runtime, T> = 
            $crate::oneshot::Receiver<'runtime, T, $reactor>;

        pub type Sender<'runtime, T> = 
            $crate::oneshot::Sender<'runtime, T, $reactor>;

        pub fn oneshot<'runtime, T>(
            rt: &'runtime Runtime
        ) -> (
            $crate::oneshot::Sender<'runtime, T, $reactor>,
            $crate::oneshot::Receiver<'runtime, T, $reactor>,
        ) {
            $crate::oneshot::oneshot::<T, $reactor>(rt)
        }

        pub fn with_runtime<ReactorFn, FuncT, InitT, ResT>(
            reactor_constructor: ReactorFn,
            async_function: FuncT,
            init: InitT,
        ) -> ResT
        where
            // async fn foo(rt: &Runtime, param: ParamT) -> ResT
            FuncT: for<'runtime> $crate::LifetimeLinkerFn<'runtime, $reactor, InitT, ResT>,
            ReactorFn: FnOnce() -> $reactor,
        {
            $crate::with_runtime_base(reactor_constructor(), async_function, init)
        }
    };
}
