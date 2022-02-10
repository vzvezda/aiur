//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::time::Duration;

// Use the toy runtime
use aiur::toy_rt::{self};
use super::future_utils::{self};

// With emulated sleep test run instantly, actual sleep actually wait for specified
// amount of time.

//const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

// Just makes sure that with_runtime() somehow works
#[test]
fn toy_runtime_fn_works() {
    async fn async_42(_rt: &toy_rt::Runtime, _: ()) -> u32 {
        42
    }

    assert_eq!(toy_rt::with_runtime_in_mode(SLEEP_MODE, async_42, ()), 42);
}

// Verifies that with_runtime() works when InitT is a reference
#[test]
fn toy_runtime_reference_compiles() {
    let mut param: u32 = 0;

    async fn async_42ref(_rt: &toy_rt::Runtime, pref: &mut u32) {
        *pref = 42;
    }

    // First version of with_runtime() machinery did not work with parameter references,
    // so this test was introduced. No with with_runtime() works more less as expected.
    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_42ref, &mut param);
    assert_eq!(param, 42);
}

// Verifies that with_runtime() works when InitT is a reference and RetT is a reference as well
#[test]
fn toy_runtime_reference_in_return_type_compiles() {
    let param = (40u32, 2u8);

    // async function takes a param a reference and return a reference as well
    async fn async_ref_returned<'i>(_rt: &toy_rt::Runtime, pref: &'i (u32, u8)) -> &'i u8 {
        &(*pref).1
    }
    let ret = toy_rt::with_runtime_in_mode(SLEEP_MODE, async_ref_returned, &param);
    assert_eq!(*ret, 2);
}

// Verifies that with_runtime() works with mutable references
#[test]
fn toy_runtime_mutable_references_in_param_return() {
    let mut param = (0u32, 0u8);

    // async function takes a param a reference and return a reference as well
    async fn async_ref_returned<'i>(_rt: &toy_rt::Runtime, pref: &'i mut (u32, u8)) -> &'i mut u8 {
        (*pref).0 = 42;
        (*pref).1 = 42;

        &mut (*pref).1
    }
    let ret = toy_rt::with_runtime_in_mode(SLEEP_MODE, async_ref_returned, &mut param);
    assert_eq!(*ret, 42);
    assert_eq!(param.0, 42);
    assert_eq!(param.1, 42);
    //assert_eq!(*ret, 42); // - does not compile, immutable borrow above
}

async fn async_sleep_once(rt: &toy_rt::Runtime, seconds: u32) {
    let now = rt.io().now32();
    toy_rt::sleep(rt, Duration::from_secs(seconds as u64)).await;
    assert!(
        rt.io().now32() >= now + seconds * 1000,
        "Delay did not happen"
    );
}

// Make sure that sleep is somehow works
#[test]
fn simple_sleep_works() {
    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_sleep_once, 1);
}

// Make sure that consequential sleeps work
#[test]
fn consequential_sleeps_works() {
    // verify that we can schedule timer one by one
    async fn multi_sleep(rt: &toy_rt::Runtime, seconds: u32) {
        async_sleep_once(rt, seconds).await;
        async_sleep_once(rt, seconds).await;
        async_sleep_once(rt, seconds).await;
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, multi_sleep, 1);
}

mod measure {
    use std::cell::Cell;

    pub(super) struct MutCounter {
        counter: Cell<u32>,
    }

    impl MutCounter {
        pub(super) fn new(init: u32) -> Self {
            MutCounter {
                counter: Cell::new(init),
            }
        }

        pub(super) fn inc(&self) {
            self.counter.set(self.get() + 1);
        }

        /* currently unused
        pub(super) fn dec(&self) {
            self.counter.set(self.get() - 1);
        }
        */

        pub(super) fn get(&self) -> u32 {
            self.counter.get()
        }
    }
}

// Verify that cancellation works
#[test]
fn sleep_cancellation_works() {
    async fn cancel_fn(rt: &toy_rt::Runtime, seconds: u32) {
        future_utils::poll_inner_once(async_sleep_once(rt, seconds)).await;
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, cancel_fn, 1);
}

// Verify that cancellation works
#[test]
fn two_concurrent_timers_works() {
    async fn start_concurrent(rt: &toy_rt::Runtime, _: ()) {
        let start = rt.io().now32();

        future_utils::any2void(async_sleep_once(rt, 1), async_sleep_once(rt, 5)).await;

        let elapsed = rt.io().now32() - start;

        assert!(elapsed >= 1 * 1000);
        assert!(elapsed < 5 * 1000);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, start_concurrent, ());
}

// Returns how many nodes in a full tree (e.g. all children with nodes)
fn nodes_in_tree(children: u32, depth: u32) -> u32 {
    // formula from SO: (N^L-1) / (N-1)
    (children.pow(depth) - 1) / (children - 1)
}



