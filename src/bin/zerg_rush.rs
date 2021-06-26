//
use std::time::Duration;
use aiur::toy_rt::{self};


// 1 drone collect 8 minerals for 5 seconds
//async fn collect_minerals(rt: &toy_rt::Runtime, state: &State) {

async fn async_zerg_rush(_rt: &toy_rt::Runtime, _zerglings: u128) -> Duration {
    Duration::from_secs(42)
}

fn main() {
    // Select a mode while playing with the tool
    const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
    //const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

    let res = toy_rt::with_runtime_in_mode(SLEEP_MODE, async_zerg_rush, 1024);
    println!("Zerg rush has been completed in {}", res.as_secs());
}

