//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
//
// Tests for join!() macro
// Use the toy runtime
use aiur::toy_rt::{self};

use std::time::Duration;

// With emulated sleep test run instantly, actual sleep actually wait for specified
// amount of time.
//const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

fn assert_duration(actual: u32, expected: u32) {
    let actual = actual as i64;
    let expected = expected as i64;

    assert!(
        i64::abs(actual - expected) < 100,
        "Duration is in unexpected range: actual: {}, expected: {}, diff: {} > 100",
        actual,
        expected,
        i64::abs(actual - expected)
    );
}

// Verifies if any_of2 can be fully consumed in expected order
#[test]
fn join2_returns_tupple() {
    // a future to join: sleep for given duration and return the given number
    async fn sleep_and_ret(rt: &toy_rt::Runtime, duration: Duration, value: u32) -> u32 {
        toy_rt::sleep(rt, duration).await;
        value
    }

    async fn measured(rt: &toy_rt::Runtime) -> Vec<u32> {
        // Invoke join on features that sleeps different time and return different numbers
        let res = toy_rt::join!(
            sleep_and_ret(rt, Duration::from_millis(1000), 1),
            sleep_and_ret(rt, Duration::from_millis(2000), 2),
        )
        .await;

        // Return tuple as vec
        vec![res.0, res.1]
    }

    async fn async_starter(rt: &toy_rt::Runtime, _: ()) {
        let start = rt.io().now32();
        let sequence = measured(rt).await;
        let elapsed = rt.io().now32() - start;

        // Verify that join sleep the right time and the right tuple was returned
        assert_duration(elapsed, 2000);
        assert_eq!(vec!(1, 2), sequence);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_starter, ());
}

#[test]
fn join8_returns_tupple() {
    // a future to join: sleep for given duration and return the given number
    async fn sleep_and_ret(rt: &toy_rt::Runtime, duration: Duration, value: u32) -> u32 {
        toy_rt::sleep(rt, duration).await;
        value
    }

    async fn measured(rt: &toy_rt::Runtime) -> Vec<u32> {
        let res = toy_rt::join!(
            // Invoke join on features that sleeps different time and return different numbers
            sleep_and_ret(rt, Duration::from_millis(1000), 1),
            sleep_and_ret(rt, Duration::from_millis(2000), 2),
            sleep_and_ret(rt, Duration::from_millis(3000), 3),
            sleep_and_ret(rt, Duration::from_millis(4000), 4),
            sleep_and_ret(rt, Duration::from_millis(5000), 5),
            sleep_and_ret(rt, Duration::from_millis(6000), 6),
            sleep_and_ret(rt, Duration::from_millis(7000), 7),
            sleep_and_ret(rt, Duration::from_millis(8000), 8),
        )
        .await;

        // Return tuple as vec
        vec![res.0, res.1, res.2, res.3, res.4, res.5, res.6, res.7]
    }

    async fn async_starter(rt: &toy_rt::Runtime, _: ()) {
        let start = rt.io().now32();
        let sequence = measured(rt).await;
        let elapsed = rt.io().now32() - start;

        // Verify that join sleep the right time and the right tuple was returned
        assert_duration(elapsed, 8000);
        assert_eq!(vec!(1, 2, 3, 4, 5, 6, 7, 8), sequence);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_starter, ());
}
