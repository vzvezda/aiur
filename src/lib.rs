//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \

//! Single thread executor library for building async runtimes.
//!
//! Note: as for now it is more a research project rather then production ready library.
//!
//! The async/await machinery in Rust compiler in a nutshell provides a transformation 
//! of a synchronous looking code into the struct that impls [std::future::Future] and 
//! then user need a library to run such future (or "poll the future to completion"). 
//! Popular async runtime libraries are tokio, async-std, smol.   
//!
//! The library that runs the future (usually with something called block_on() function) can 
//! have two distinct parts in its API:
//!
//!    * executor - something that organizes the futures from user into a set of tasks 
//!                 for execution (like spawn(), JoinHandle)
//!
//!    * reactor - schedule I/O using OS API (epoll, io_uring, completion port) and be able 
//!                awake a task when certain I/O event occurs.
//!
//! This library implements the executor part of the async runtime. Suppose I would like to create
//! async runtime named Mega. I can do it using the aiur crate: 
//!
//!    1. Create a crate mega_runtime that depends aiur
//!    2. Implement aiur::[Reactor] in MegaReactor and API to schedule I/O
//!    3. Now I can have a `type MegaRuntime = aiur::Runtime<MegaReactor>`, which is now a complete
//!       runtime with both executor and reactor.
//!    4. Apps are using MegaRuntime not knowing anything that there is some aiur used
//!       under the hood.
//!
//! As aiur does not have anything OS-specific, it can be used on any OS with std. For testing 
//! purposes it has toy reactor that only supports async sleeping.
//!
//! Major distinction from popular libraries that aiur only provides a single thread executor. It
//! seems to be a lager topic and can be discussed later.
//!
//! There other ideas where ongoing research happens:
//!   * structured concurrency
//!   * async destruction
//!   * nostd
//!   * scoped access to API
//!

#[macro_use]
mod modtrace_macro;

mod channel;
mod channel_rt;
mod oneshot;
mod oneshot_rt;
mod reactor;
mod runtime;
mod scope;
mod task;
mod timer;
mod with_runtime;

pub mod toy_rt;

pub use oneshot::{oneshot, RecverOnce, SenderOnce};
pub use channel::{channel, Recver, Sender};
pub use reactor::{EventId, GetEventId, Reactor, TemporalReactor};
pub use runtime::Runtime;
pub use scope::Scope;
pub use timer::sleep;
pub use toy_rt::ToyReactor;
pub use with_runtime::{with_runtime_base, LifetimeLinkerFn};

/// This is a help macro to create API for your own runtime based on re-exporting aiur runtime
/// and specialize it with your reactor.
#[macro_export]
macro_rules! export_runtime {
    ($reactor:ident) => {
        pub type Runtime = $crate::Runtime<$reactor>;
        pub type Scope<'runtime> = $crate::Scope<'runtime, $reactor>;
        pub type EventId = $crate::EventId;
        pub use $crate::sleep;
        pub use $crate::GetEventId;

        pub type RecverOnce<'runtime, T> = $crate::RecverOnce<'runtime, T, $reactor>;
        pub type SenderOnce<'runtime, T> = $crate::SenderOnce<'runtime, T, $reactor>;
        pub type Recver<'runtime, T> = $crate::Recver<'runtime, T, $reactor>;
        pub type Sender<'runtime, T> = $crate::Sender<'runtime, T, $reactor>;

        pub fn oneshot<'runtime, T>(
            rt: &'runtime Runtime,
        ) -> (
            $crate::SenderOnce<'runtime, T, $reactor>,
            $crate::RecverOnce<'runtime, T, $reactor>,
        ) {
            $crate::oneshot::<T, $reactor>(rt)
        }

        pub fn channel<'runtime, T>(
            rt: &'runtime Runtime,
        ) -> (
            $crate::Sender<'runtime, T, $reactor>,
            $crate::Recver<'runtime, T, $reactor>,
        ) {
            $crate::channel::<T, $reactor>(rt)
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
