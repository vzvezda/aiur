//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use super::measure::{self};
use aiur::toy_rt::{self};

use std::time::Duration;

// With emulated sleep test run instantly, actual sleep actually wait for specified
// amount of time.
//const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

// This test creates this any_of structure
//   ---stream = any_of(f1:Timer1(1s), f2:nested_loop(Timer2(2s)))
//   ---stream.next().await:
//      f1 event happens first but added to frozen_events because task is blocked by nested loop
//      f2 completes and retuned by stream.next
//   ---drop(stream)
//      So we have f1 future dropped while the event is completed (but frozen).
//   
// When toy_rt timer future did not use the proper check for canceling frozen events this
// test was crashing with "reactor cancel unknown timer" panic. 
#[test]
fn cancel_frozen_event() {
    async fn freezable(rt: &toy_rt::Runtime) {
        let _x = rt.nested_loop(measure::sleep_and_ret(rt, Duration::from_millis(2000), 2));
    }

    async fn measured(rt: &toy_rt::Runtime) {
        toy_rt::pinned_any_of!(
            stream,
            toy_rt::sleep(rt, Duration::from_millis(1000)),
            freezable(rt),
        );

        match stream.next().await.unwrap() {
            toy_rt::OneOf2::First(_) => {
                assert!(false, "Test expects the Second() to complete first")
            }
            toy_rt::OneOf2::Second(_) => {}
        }
    }

    async fn async_starter(rt: &toy_rt::Runtime, _: ()) {
        measured(rt).await;
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_starter, ());
}
