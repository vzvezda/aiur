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

// Verify spawn in a linear pattern works. Linear pattern is when an async function spawns
// another async funciton which in turn spawn any async function, etc up to some depth.
//
// [spawn] - [spawn] - [...] - [spawn] - [sleep 1s]
#[test]
fn spawn_linear_works() {
    use measure::MutCounter;

    let counter = MutCounter::new(0);

    const MAX_DEPTH: u32 = 10;
    const THRESHOLD_MS: u32 = 500;

    async fn measured(rt: &toy_rt::Runtime, counter: &MutCounter) {
        let start = rt.io().now32();
        deep_dive(rt, MAX_DEPTH, counter).await;
        let elapsed = rt.io().now32() - start;

        assert!(elapsed >= 1 * 1000);
        assert!(elapsed < 1 * 1000 + THRESHOLD_MS); // use a threashold to avoid false alarms
    }

    async fn deep_dive(rt: &toy_rt::Runtime, depth: u32, counter: &MutCounter) {
        if depth == 0 {
            async_sleep_once(rt, 1).await;
        } else {
            counter.inc();
            let scope = toy_rt::Scope::new_named(rt, &format!("xx#{}", depth));
            scope.spawn(deep_dive(rt, depth - 1, counter));
        }
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, measured, &counter);
    assert_eq!(counter.get(), MAX_DEPTH, "Unexpected dive depth");
}

// Returns how many nodes in a full tree (e.g. all children with nodes)
fn nodes_in_tree(children: u32, depth: u32) -> u32 {
    // formula from SO: (N^L-1) / (N-1)
    (children.pow(depth) - 1) / (children - 1)
}

// Verify how it works when spawn tree looks like this:
//    * root task spawn 3 tasks
//    * these 3 tasks spawn each
//    * each of these tasks spawn 3 tasks
//    * etc
//    * on the deepest level task performs a sleep (1 sec), ending this recursion.
//
//             []
//      []     []     []
//    [][][] [][][] [][][]
//    ...
//
//    So there may be a lot of task spawned, but the total duration of the test should be like 1
//    second, because all tasks make the sleep paralel.
#[test]
fn spawn_tree_works() {
    use measure::MutCounter;

    let node_counter = MutCounter::new(0);

    const MAX_DEPTH: u32 = 5;
    const MAX_WIDTH: u32 = 3;
    const THRESHOLD_MS: u32 = 500;

    async fn measured(rt: &toy_rt::Runtime, node_counter: &MutCounter) {
        let start = rt.io().now32();
        super_deep_dive(rt, MAX_DEPTH, node_counter).await;
        let elapsed = rt.io().now32() - start;

        // lot of sleeps goes in parallel, so total execution time is around 1 sec
        assert!(elapsed >= 1 * 1000);
        assert!(elapsed < 1 * 1000 + THRESHOLD_MS); // use a threashold to avoid false alarms
    }

    async fn super_deep_dive(rt: &toy_rt::Runtime, depth: u32, node_counter: &MutCounter) {
        if depth == 0 {
            toy_rt::sleep(rt, Duration::from_secs(1)).await;
        // This tree layer does not do the 'node_counter.inc();'
        } else {
            node_counter.inc();
            let scope = toy_rt::Scope::new_named(rt, &format!("xx#{}", depth));
            for _i in 0..MAX_WIDTH {
                scope.spawn(super_deep_dive(rt, depth - 1, node_counter));
            }
        }
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, measured, &node_counter);
    assert_eq!(nodes_in_tree(3, 5), 121);
    assert_eq!(
        node_counter.get(),
        nodes_in_tree(MAX_WIDTH, MAX_DEPTH),
        "Unexpected node counts"
    );
}

// Same as above but, but each tree node not only spawn children, but also make sleep(). This
// is how we verify that executor works fine when there there are spawn (executor) and sleep
// (reactor) activity in each task.
#[test]
fn spawn_mixed_tree_works() {
    use measure::MutCounter;

    let node_counter = MutCounter::new(0);

    const MAX_DEPTH: u32 = 5;
    const MAX_WIDTH: u32 = 3;
    const THRESHOLD_MS: u32 = 500;

    async fn measured(rt: &toy_rt::Runtime, node_counter: &MutCounter) {
        let start = rt.io().now32();
        sleep_and_deep_dive(rt, MAX_DEPTH, node_counter).await;
        let elapsed = rt.io().now32() - start;

        // On each layer this is 1 second of sleep and there is also one additional layer of sleep
        assert!(elapsed >= 6 * 1000);
        assert!(elapsed < 6 * 1000 + THRESHOLD_MS); // use threshold to avoid false alarms
    }

    async fn sleep_and_deep_dive(rt: &toy_rt::Runtime, depth: u32, node_counter: &MutCounter) {
        if depth > 0 {
            node_counter.inc();
            let scope = toy_rt::Scope::new_named(rt, &format!("xx#{}", depth));
            for _i in 0..MAX_WIDTH {
                toy_rt::sleep(rt, Duration::from_millis((1000 / MAX_WIDTH + 1).into())).await;
                scope.spawn(sleep_and_deep_dive(rt, depth - 1, node_counter));
            }
        } else {
            toy_rt::sleep(rt, Duration::from_secs(1)).await;
        }
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, measured, &node_counter);
    assert_eq!(nodes_in_tree(3, 5), 121);
    assert_eq!(
        node_counter.get(),
        nodes_in_tree(MAX_WIDTH, MAX_DEPTH),
        "Unexpected node counts"
    );
}

// Just like in previous spawn tests this test makes a tree but without any reactor's io (e.g.
// no sleep). This test verifies that it works fine when no i/o involved.
#[test]
fn spawn_only_works() {
    use measure::MutCounter;

    let node_counter = MutCounter::new(0);

    const MAX_DEPTH: u32 = 5;
    const MAX_WIDTH: u32 = 3;
    const THRESHOLD_MS: u32 = 500;

    async fn measured(rt: &toy_rt::Runtime, node_counter: &MutCounter) {
        let start = rt.io().now32();
        deep_dive_without_sleep(rt, MAX_DEPTH, node_counter).await;
        let elapsed = rt.io().now32() - start;

        // No sleep, so there should be no waits.
        assert!(elapsed < THRESHOLD_MS);
    }

    async fn deep_dive_without_sleep(rt: &toy_rt::Runtime, depth: u32, node_counter: &MutCounter) {
        if depth > 0 {
            node_counter.inc();
            let scope = toy_rt::Scope::new_named(rt, &format!("xx#{}", depth));
            for _i in 0..MAX_WIDTH {
                scope.spawn(deep_dive_without_sleep(rt, depth - 1, node_counter));
            }
        }
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, measured, &node_counter);
    assert_eq!(nodes_in_tree(3, 5), 121);
    assert_eq!(
        node_counter.get(),
        nodes_in_tree(MAX_WIDTH, MAX_DEPTH),
        "Unexpected node counts"
    );
}


// Test that should compile 
#[test]
fn spawn_soundness_ok() {
    async fn target<'runtime, 'param>(_rt: &'runtime toy_rt::Runtime, param: &'param mut u32) {
        *param = 42;
    }

    async fn spawn_sound(rt: &toy_rt::Runtime, flag: bool) {
        let mut value : u32 = 0;

        if flag {
            let scope = toy_rt::Scope::new_named(rt, "soundness_ok_test");
            // it should be fine to use '&mut value' here because it outlives the 'scope'.
            scope.spawn(target(rt, &mut value));
        }

        assert_eq!(value, 42);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, spawn_sound, true);
}

// this should not compile. To verify compile with attr commented
#[cfg(soundness_check="mut")]
#[test]
fn spawn_soundness_mut_fail() {
    async fn target<'runtime, 'param>(rt: &'runtime toy_rt::Runtime, param: &'param mut u32) {
        *param = 42;
    }

    async fn spawn_unsound_mut(rt: &toy_rt::Runtime, flag: bool) {
        let scope = toy_rt::Scope::new_named(rt, "soundness_fail_test");
        if flag {
            let mut value : u32 = 0;
            // it is not ok to use '&mut value' here because the future returned by target()
            // outlives the 'value'. This code must not compile if aiur API is sound.
            scope.spawn(target(rt, &mut value));
        }
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, spawn_unsound_mut, true);
}

// this should not compile. To verify compile with attr commented
#[cfg(soundness_check="ref")]
#[test]
fn spawn_soundness_ref_fail() {
    async fn target<'runtime, 'param>(rt: &'runtime toy_rt::Runtime, param: &'param u32) {
        println!("{}", *param);
    }

    async fn spawn_unsound_ref(rt: &toy_rt::Runtime, flag: bool) {
        let scope = toy_rt::Scope::new_named(rt, "soundness_fail_test");
        if flag {
            let mut value : u32 = 0;
            // it is not ok to use '&mut value' here because the future returned by target()
            // outlives the 'value'. This code must not compile if aiur API is sound.
            scope.spawn(target(rt, &value));
        }
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, spawn_unsound_ref, true);
}


