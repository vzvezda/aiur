//
use std::time::Duration;
use aiur::toy_rt::{self};

struct Drone {
}

#[derive(Copy, Clone)]
struct State {
    minerals: u32,
    drones: u8,
}

impl State {
    fn new() -> Self {
        State { minerals: 0, drones: 4 }
    }
}

// 1 drone collect 8 minerals for 5 seconds
async fn collect_minerals(rt: &toy_rt::Runtime, state: &State) {
    let mut result = state.clone();
    toy_rt::sleep(rt, Duration::from_secs(1)).await;
}


async fn async_zerg_rush(rt: &toy_rt::Runtime, _zerglings: u128) -> Duration {

    let mut state = State::new(); 

    //let (send, recv) = ;
    let mut scope = toy_rt::Scope::new(rt);

    for i in 0..state.drones {
        scope.spawn(collect_minerals(rt, &state));
    }
    /*

    loop {
        state = recv.receive().await;

        println!("Collected: {} minerals", state.minerals);
        if state.minerals >= 200 {
            break;
        }
    }

    toy_rt::sleep(rt, Duration::from_secs(5)).await;
    */

    Duration::from_secs(42)
}

fn main() {
    // Select a mode while playing with the tool
    const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
    //const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

    let res = toy_rt::with_runtime_in_mode(SLEEP_MODE, async_zerg_rush, 1024);
    println!("Zerg rush has been completed in {}", res.as_secs());
}

