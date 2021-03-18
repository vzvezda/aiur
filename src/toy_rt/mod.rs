//  \ O /     
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \     
//
// Toy Runtime is a runtime based on aiur with reactor that only support sleeping. Sleeping
// works as emulation of any long IO, so this runtime is used for testing.
mod toy_reactor;

// has to export for the macro
pub use toy_reactor::ToyReactor;
pub use toy_reactor::SleepMode;

// Make a toy runtime
crate::export_runtime!(ToyReactor);

pub fn with_runtime_in_mode<FuncT, InitT, ResT>(
    sleep_mode: SleepMode,
    async_function: FuncT,
    init: InitT,
) -> ResT
where
    // async fn foo(rt: &Runtime, param: ParamT) -> ResT
    FuncT: for<'runtime> crate::LifetimeLinkerFn<'runtime, ToyReactor, InitT, ResT>,
{
    with_runtime(move || ToyReactor::new_with_mode(sleep_mode), 
        async_function, init)
}

