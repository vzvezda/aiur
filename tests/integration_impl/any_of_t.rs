//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
//
// Use the toy runtime
use aiur::toy_rt::{self};
use super::measure::{self};

use std::time::Duration;

// With emulated sleep test run instantly, actual sleep actually wait for specified
// amount of time.
//const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

// Verifies if any_of2 can be fully consumed in expected order
#[test]
fn consume_any_of2() {
    async fn measured(rt: &toy_rt::Runtime) -> Vec<u32> {
        toy_rt::pinned_any_of!(
            stream,
            toy_rt::sleep(rt, Duration::from_millis(1000)),
            toy_rt::sleep(rt, Duration::from_millis(2000))
        );

        let mut res = Vec::new();
        while let Some(v) = stream.next().await {
            match v {
                toy_rt::OneOf2::First(_) => res.push(1),
                toy_rt::OneOf2::Second(_) => res.push(2),
            }
        }

        res
    }

    async fn async_starter(rt: &toy_rt::Runtime, _: ()) {
        let start = rt.io().now32();
        let sequence = measured(rt).await;
        let elapsed = rt.io().now32() - start;

        measure::assert_duration(elapsed, 2000);
        assert_eq!(vec!(1, 2), sequence);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_starter, ());
}

// There may be also tests for any_of3,4,5,6,7 - but it does not look there is much value

// Verifies if any_of8 can be fully consumed in expected order
#[test]
fn consume_any_of8() {
    async fn measured(rt: &toy_rt::Runtime) -> Vec<u32> {
        toy_rt::pinned_any_of!(
            stream,
            toy_rt::sleep(rt, Duration::from_millis(8000)),
            toy_rt::sleep(rt, Duration::from_millis(7000)),
            toy_rt::sleep(rt, Duration::from_millis(6000)),
            toy_rt::sleep(rt, Duration::from_millis(5000)),
            toy_rt::sleep(rt, Duration::from_millis(4000)),
            toy_rt::sleep(rt, Duration::from_millis(3000)),
            toy_rt::sleep(rt, Duration::from_millis(2000)),
            toy_rt::sleep(rt, Duration::from_millis(1000)),
        );

        let mut res = Vec::new();
        while let Some(v) = stream.next().await {
            match v {
                toy_rt::OneOf8::First(_) => res.push(1),
                toy_rt::OneOf8::Second(_) => res.push(2),
                toy_rt::OneOf8::Third(_) => res.push(3),
                toy_rt::OneOf8::Fourth(_) => res.push(4),
                toy_rt::OneOf8::Fifth(_) => res.push(5),
                toy_rt::OneOf8::Sixth(_) => res.push(6),
                toy_rt::OneOf8::Seventh(_) => res.push(7),
                toy_rt::OneOf8::Eighth(_) => res.push(8),
            }
        }

        res
    }

    async fn async_starter(rt: &toy_rt::Runtime, _: ()) {
        let start = rt.io().now32();
        let sequence = measured(rt).await;
        let elapsed = rt.io().now32() - start;

        measure::assert_duration(elapsed, 8000);

        assert_eq!(vec!(8, 7, 6, 5, 4, 3, 2, 1), sequence);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_starter, ());
}
