//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \

// Use the toy runtime
use super::future_utils::{self};
use aiur::toy_rt::{self};

// With emulated sleep tests are run instantly, with actual sleep mode it wait for specified
// amount of time.

//const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

// Spawns a task and send a oneshot value from parent to child tasks.
#[test]
fn oneshot_spawn_recv_works() {
    struct AsyncState {
        recv_data: u32,
    }

    async fn reader<'runtime, 'state>(
        rx: toy_rt::RecverOnce<'runtime, u32>,
        state: &'state mut AsyncState,
    ) {
        state.recv_data = rx.await.unwrap();
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        {
            let scope = toy_rt::Scope::new_named(rt, "Messenger");
            let (mut tx, rx) = toy_rt::oneshot::<u32>(&rt);
            scope.spawn(reader(rx, &mut state));
            tx.send(42).await.unwrap();
        }
        state
    }

    // State transitions for this test:
    // (C,C)->(R,C)->(R,R}->{R,E)->{R,D*}->(E,D)->(D,D)
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data, 42);
}

// Spawns a task and send a oneshot value from child to parent tasks. Interesting fact: this
// test has reveal a bug in Scope::drop()
#[test]
fn oneshot_spawn_sender_works() {
    struct AsyncState {
        recv_data: u32,
    }

    async fn writer<'runtime>(mut tx: toy_rt::SenderOnce<'runtime, u32>) {
        tx.send(42).await.unwrap();
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        {
            let scope = toy_rt::Scope::new_named(rt, "Messenger");
            let (tx, rx) = toy_rt::oneshot::<u32>(&rt);
            scope.spawn(writer(tx));
            state.recv_data = rx.await.unwrap();
        }
        state
    }

    // State transitions for this test:
    // (C,C)->(C,R)->(R,R}->{R,E)->{R,D*}->(E,D)->(D,D)
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data, 42);
}

// Launch sender/receiver in a select!()-like mode, so once the first (receiver) is complete
// the sender is dropped.
#[test]
fn oneshot_select_works() {
    struct AsyncState {
        recv_data: u32,
    }

    async fn reader<'runtime, 'state>(
        rx: toy_rt::RecverOnce<'runtime, u32>,
        state: &'state mut AsyncState,
    ) {
        state.recv_data = rx.await.unwrap();
    }

    async fn writer<'runtime>(mut tx: toy_rt::SenderOnce<'runtime, u32>) {
        tx.send(42).await.unwrap();
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        let (tx, rx) = toy_rt::oneshot::<u32>(&rt);
        future_utils::any2void(reader(rx, &mut state), writer(tx)).await;
        state
    }

    // State transitions for this test:
    // (C,C)->(R,C)->(R,R}->{R,E)->{R,D*}->(D,D)
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data, 42);
}

// When future that suppose to receive a oneshot just dropped. In this case sender
// should return the value back.
#[test]
fn oneshot_recv_dropped() {
    async fn reader<'runtime>(_rx: toy_rt::RecverOnce<'runtime, u32>) {
        // just drop _rx
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let scope = toy_rt::Scope::new_named(rt, "Messenger");
        let (mut tx, rx) = toy_rt::oneshot::<u32>(&rt);
        scope.spawn(reader(rx));

        // verify that sender receiver the value back as error
        assert_eq!(tx.send(42).await.unwrap_err(), 42);
    }

    // State transitions for this test:
    // (C,C)->(R,C)->{R,D)->{E,D)->(D,D)
    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// When future that suppose to send a oneshot just dropped. In this case receiver
// receiver an error.
#[test]
fn oneshot_sender_dropped() {
    async fn reader<'runtime>(rx: toy_rt::RecverOnce<'runtime, u32>) {
        rx.await.expect_err("Error because sender is dropped");
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let scope = toy_rt::Scope::new_named(rt, "Messenger");
        let (_tx, rx) = toy_rt::oneshot::<u32>(&rt);
        scope.spawn(reader(rx));
        // dropping _tx
    }

    // State transitions for this test:
    // (C,C)->(D,C)->(D,R}->(D,E)->(D,D)
    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation.
#[test]
fn oneshot_drop_all() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        // just drop these
        let (_tx, _rx) = toy_rt::oneshot::<u32>(&rt);
    }

    // State transitions for this test:
    // (C,C)->(C,D)->(D,D)
    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation, but drop sender first.
#[test]
fn oneshot_drop_all_alt_order() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (tx, _rx) = toy_rt::oneshot::<u32>(&rt);
        drop(tx);
    }

    // State transitions for this test:
    // (C,C)->(D,C)->(D,D)
    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation.
#[test]
fn oneshot_recv_from_dropped() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (tx, rx) = toy_rt::oneshot::<u32>(&rt);
        drop(tx);
        rx.await
            .expect_err("receiver must receive error if sender is dropped");
    }

    // State transitions for this test:
    // (C,C)->(D,C)->(D,R}->(D,E)->(D,D)
    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation.
#[test]
fn oneshot_send_to_dropped() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (mut tx, rx) = toy_rt::oneshot::<u32>(&rt);
        drop(rx);
        tx.send(42)
            .await
            .expect_err("send must receive error if sender is dropped");
    }

    // State transitions for this test:
    // (C,C)->(C,D)->{R,D)->(E,D)->(D,D)
    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// First test to verify if two simultaneous channel can co-exists and it was introduced with
// support in runtime.
#[test]
fn oneshot_two_channels() {
    struct AsyncState {
        recv1_data: u32,
        recv2_data: u32,
    }

    async fn writer<'runtime>(mut tx: toy_rt::SenderOnce<'runtime, u32>) {
        tx.send(42).await.unwrap();
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState {
            recv1_data: 0,
            recv2_data: 0,
        };
        {
            let scope = toy_rt::Scope::new_named(rt, "TwoOneshots");
            let (tx1, rx1) = toy_rt::oneshot::<u32>(&rt);
            let (tx2, rx2) = toy_rt::oneshot::<u32>(&rt);
            scope.spawn(writer(tx1));
            scope.spawn(writer(tx2));
            state.recv1_data = rx1.await.unwrap();
            state.recv2_data = rx2.await.unwrap();
        }
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    // Verify that that sent data was actually read by receiver.
    assert_eq!(state.recv1_data, 42);
    assert_eq!(state.recv2_data, 42);
}

// Finally, first echo server, although not a network based, but oneshot based.
#[test]
fn oneshot_two_channels_echo_server() {
    struct AsyncState {
        echo_data: u32,
    }

    async fn echo_server<'runtime>(
        mut tx: toy_rt::SenderOnce<'runtime, u32>,
        rx: toy_rt::RecverOnce<'runtime, u32>,
    ) {
        tx.send(rx.await.unwrap()).await.unwrap();
    }

    async fn echo_client(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { echo_data: 0 };
        {
            let scope = toy_rt::Scope::new_named(rt, "Echo");
            let (tx1, rx1) = toy_rt::oneshot::<u32>(&rt);
            let (mut tx2, rx2) = toy_rt::oneshot::<u32>(&rt);
            scope.spawn(echo_server(tx1, rx2));
            tx2.send(42).await.unwrap();
            state.echo_data = rx1.await.unwrap();
        }
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, echo_client, ());

    // Verify that that sent data was actually read by receiver.
    assert_eq!(state.echo_data, 42);
}
