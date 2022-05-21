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


// Verifies if any_task2 can be fully consumed in expected order
#[test]
fn consume_any_task2() {
    async fn measured(rt: &toy_rt::Runtime) -> Vec<u32> {
        let stream = toy_rt::any_task2(
            rt,
            toy_rt::sleep(rt, Duration::from_millis(1000)),
            toy_rt::sleep(rt, Duration::from_millis(2000))
        );

        toy_rt::pin_local!(stream);

        let mut res = Vec::new();
        while let Some(v) = stream.next().await {
            dbg!("next");
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
