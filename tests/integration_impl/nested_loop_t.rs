//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
//
// Use the toy runtime
use super::measure::{self};
use aiur::toy_rt::{self};

use std::time::Duration;

// With emulated sleep test run instantly, actual sleep actually wait for specified
// amount of time.
//const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

// Invoke just invoke nested loop in a async code without branches.
#[test]
fn trivial_nested_loop() {
    async fn async_starter(rt: &toy_rt::Runtime, _: ()) {
        rt.nested_loop(toy_rt::sleep(rt, Duration::from_millis(1000)));
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_starter, ());
}

// Verifies that join of two tasks actually executes two futures in parallel
#[test]
fn join_tasks2() {
    async fn measured(rt: &toy_rt::Runtime) -> Vec<u32> {
        let (v1, v2) = toy_rt::join_tasks2(
            rt,
            measure::sleep_and_ret(rt, Duration::from_millis(1000), 1),
            measure::sleep_and_ret(rt, Duration::from_millis(2000), 2),
        )
        .await;

        vec![v1, v2]
    }

    async fn async_starter(rt: &toy_rt::Runtime, _: ()) {
        let sequence = measure::expect_future_duration(rt, measured(rt), 2000).await;
        assert_eq!(vec!(1, 2), sequence);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_starter, ());
}

// Makes this task tree:
//
// join_task2---Sleep(1s)
//            \-NestedLoop(Sleep(2s)
#[test]
fn nested_loop_in_join_tasks2() {
    async fn frozable(rt: &toy_rt::Runtime) -> u32 {
        rt.nested_loop(measure::sleep_and_ret(rt, Duration::from_millis(2000), 2))
    }

    async fn measured(rt: &toy_rt::Runtime) -> Vec<u32> {
        let (v1, v2) = toy_rt::join_tasks2(
            rt,
            measure::sleep_and_ret(rt, Duration::from_millis(1000), 1),
            frozable(rt),
        )
        .await;

        vec![v1, v2]
    }

    async fn async_starter(rt: &toy_rt::Runtime, _: ()) {
        let sequence = measure::expect_future_duration(rt, measured(rt), 2000).await;
        assert_eq!(vec!(1, 2), sequence);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_starter, ());
}

// There is only one task but two futures running concurrent using join:
//
// fut_join ----> a:sleep(2s)
//            \-> sleep(1s) -> nested_loop(sleep(2s))
//
// While nested loop happens the frozen task receive event for a:sleep(), so the rt support
// for postponing frozen events should work.
#[test]
fn frozen_events_postponed() {
    async fn task1(rt: &toy_rt::Runtime) -> u32 {
        measure::sleep_and_ret(rt, Duration::from_millis(2000), 1).await
    }
    async fn task2(rt: &toy_rt::Runtime) -> u32 {
        toy_rt::sleep(&rt, Duration::from_millis(1000)).await;
        rt.nested_loop(measure::sleep_and_ret(rt, Duration::from_millis(2000), 2))
    }

    async fn async_starter(rt: &toy_rt::Runtime, _: ()) {
        let res = toy_rt::join!(task1(rt), task2(rt)).await;
        assert_eq!((1, 2), res);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_starter, ());
}
