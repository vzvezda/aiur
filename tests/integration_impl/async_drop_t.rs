//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::time::Duration;

// Use the toy runtime
use aiur::toy_rt::{self};

// With emulated sleep test run instantly, actual sleep actually wait for specified
// amount of time.
//const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

// drop() impl for this truct is blocked until drop_async().await is completed.
struct WaitOnDrop<'rt> {
    rt: &'rt toy_rt::Runtime,
    drop_length: Duration,
}

impl<'rt> WaitOnDrop<'rt> {
    fn new_onesec(rt: &'rt toy_rt::Runtime) -> Self {
        Self::new(rt, Duration::from_millis(1000))
    }

    fn new(rt: &'rt toy_rt::Runtime, drop_length: Duration) -> Self {
        Self { rt, drop_length }
    }

    // a helper function to invoke from Drop::drop()
    async fn drop_async(&self) {
        toy_rt::sleep(self.rt, self.drop_length).await;
    }
}

impl<'rt> Drop for WaitOnDrop<'rt> {
    fn drop(&mut self) {
        self.rt.nested_loop(self.drop_async());
    }
}

// Just makes sure that async destruction is feasible. When there is only one task
// there is no much to test, check out other tests with several tasks and frozen events.
#[test]
fn async_drop_proof() {
    async fn measured(rt: &toy_rt::Runtime) {
        let _long_drop = WaitOnDrop::new_onesec(rt);
        // Here invokes the drop(_long_drop) that 
    }

    async fn async_starter(rt: &toy_rt::Runtime, _: ()) {
        let start = rt.io().now32();
        measured(rt).await;
        let elapsed = rt.io().now32() - start;
        assert!(elapsed >= 1 * 1000);
        assert!(elapsed < 2 * 1000);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_starter, ());
}
