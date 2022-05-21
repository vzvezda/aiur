//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
//
// Shared code for tests

use aiur::toy_rt::{self};
use std::time::Duration;
use std::future::Future;

pub async fn sleep_and_ret(rt: &toy_rt::Runtime, duration: Duration, value: u32) -> u32 {
    toy_rt::sleep(rt, duration).await;
    value
}

///
pub fn assert_duration(actual: u32, expected: u32) {
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

pub async fn expect_future_duration<FutT, ResT>(
    rt: &toy_rt::Runtime,
    future: FutT,
    expected_duration: u32,
) -> ResT
where
    FutT: Future<Output = ResT>,
{
    let start = rt.io().now32();
    let result = future.await;
    let elapsed = rt.io().now32() - start;
    assert_duration(elapsed, expected_duration);
    result
}
