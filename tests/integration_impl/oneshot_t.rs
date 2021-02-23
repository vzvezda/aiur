//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \

// Use the toy runtime
use aiur::toy_rt::{self};

// With emulated sleep test run instantly, actual sleep actually wait for specified
// amount of time.

//const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

#[test]
fn channel_works() {
    struct AsyncState {
        recv_data: u32,
    }

    async fn reader<'runtime, 'state>(
        rt: &'runtime toy_rt::Runtime,
        rx: toy_rt::Receiver<'runtime, u32>,
        state: &'state mut AsyncState,
    ) {
        state.recv_data = rx.await.unwrap();
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        {
            let mut scope = toy_rt::Scope::new_named(rt, "Messenger");
            let (mut tx, rx) = toy_rt::oneshot::<u32>(&rt);
            scope.spawn(reader(rt, rx, &mut state));
            tx.send(42).await;
        }
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data, 42);
}

#[test]
fn channel_recv_dropped() {
    struct AsyncState {
        recv_data: u32,
    }

    async fn reader<'runtime, 'state>(
        rt: &'runtime toy_rt::Runtime,
        rx: toy_rt::Receiver<'runtime, u32>,
        state: &'state mut AsyncState,
    ) {
        // do thing on sender side
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        {
            let mut scope = toy_rt::Scope::new_named(rt, "Messenger");
            let (mut tx, rx) = toy_rt::oneshot::<u32>(&rt);
            scope.spawn(reader(rt, rx, &mut state));

            assert_eq!(tx.send(42).await.unwrap_err(), 42);
        }
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data, 0);
}

#[test]
fn channel_sender_dropped() {
    struct AsyncState {
        recv_data: u32,
    }

    async fn reader<'runtime, 'state>(
        rt: &'runtime toy_rt::Runtime,
        rx: toy_rt::Receiver<'runtime, u32>,
        state: &'state mut AsyncState,
    ) {
        rx.await.expect_err("Error because sender is dropped");
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        {
            let mut scope = toy_rt::Scope::new_named(rt, "Messenger");
            let (mut tx, rx) = toy_rt::oneshot::<u32>(&rt);
            scope.spawn(reader(rt, rx, &mut state));
        }
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data, 0);
}

