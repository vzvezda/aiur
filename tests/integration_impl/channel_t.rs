//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \

// Use the toy runtime
use aiur::toy_rt::{self};
use super::future_utils::{self};

// With emulated sleep tests are run instantly, with actual sleep mode it wait for specified
// amount of time.

//const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

// Spawns a task and send a oneshot value from parent to child tasks.
#[cfg(disable_this_for_a_while="mut")]
#[test]
fn channel_works() {
    struct AsyncState {
        recv_data: Vec<u32>,
    }

    async fn reader<'runtime, 'state>(
        rt: &'runtime toy_rt::Runtime,
        mut rx: toy_rt::ChReceiver<'runtime, u32>,
        state: &'state mut AsyncState,
    ) {
        while let Ok(value) = rx.next().await {
            state.recv_data.push(value);
        }
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: Vec::new() };
        {
            let mut scope = toy_rt::Scope::new_named(rt, "ChannelWorks");
            let (mut tx, rx) = toy_rt::channel::<u32>(&rt);
            scope.spawn(reader(rt, rx, &mut state));
            for i in 0..10u32 {
                tx.send(i).await;
            }
        }
        state
    }

    // State transitions for this test:
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data.len(), 10);
}

#[test]
fn channel_compiles() {
    async fn messenger(rt: &toy_rt::Runtime, _x: u32) {
        let (mut tx, mut rx) = toy_rt::channel::<u32>(&rt);
        let send_fut = tx.send(44);
        let recv_fut = rx.next();
    }

    // State transitions for this test:
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, 42);
}

// 
#[test]
fn channel_echo_server() {
    struct AsyncState {
        echo_data: u32,
    }

    async fn echo_server<'runtime, 'state>(
        rt: &'runtime toy_rt::Runtime,
        mut tx: toy_rt::ChSender<'runtime, u32>,
        mut rx: toy_rt::ChReceiver<'runtime, u32>,
    ) {
        while let Ok(value) = rx.next().await {
            tx.send(value).await.unwrap();
        }
    }

    async fn echo_client(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { echo_data: 0, };
        {
            let mut scope = toy_rt::Scope::new_named(rt, "Echo");
            let (tx1, mut rx1) = toy_rt::channel::<u32>(&rt);
            let (mut tx2, rx2) = toy_rt::channel::<u32>(&rt);
            scope.spawn(echo_server(rt, tx1, rx2));
            tx2.send(42).await.unwrap();
            state.echo_data = rx1.next().await.unwrap();
            tx2.send(8).await.unwrap();
            state.echo_data += rx1.next().await.unwrap();
        }
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, echo_client, ());

    // Verify that that sent data was actually read by receiver.
    assert_eq!(state.echo_data, 50);
}


