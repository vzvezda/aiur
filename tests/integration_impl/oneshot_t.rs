//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \

// Use the toy runtime
use aiur::toy_rt::{self};
use super::future_utils::{self};

// With emulated sleep test run instantly, actual sleep actually wait for specified
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

    async fn writer<'runtime, 'state>(
        rt: &'runtime toy_rt::Runtime,
        mut tx: toy_rt::Sender<'runtime, u32>,
    ) {
        tx.send(42).await.unwrap();
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        {
            let mut scope = toy_rt::Scope::new_named(rt, "Messenger");
            let (tx, rx) = toy_rt::oneshot::<u32>(&rt);
            scope.spawn(writer(rt, tx));
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
        rx: toy_rt::Receiver<'runtime, u32>,
        state: &'state mut AsyncState,
    ) {
        state.recv_data = rx.await.unwrap();
    }

    async fn writer<'runtime>(mut tx: toy_rt::Sender<'runtime, u32>) {
        tx.send(42).await;
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        let (mut tx, rx) = toy_rt::oneshot::<u32>(&rt);
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

            // verify that sender receiver the value back as error
            assert_eq!(tx.send(42).await.unwrap_err(), 42);
        }
        state
    }

    // State transitions for this test:
    // (C,C)->(R,C)->{R,D)->{E,D)->(D,D)
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data, 0);
}

// When future that suppose to send a oneshot just dropped. In this case receiver 
// receiver an error.
#[test]
fn oneshot_sender_dropped() {
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

    // State transitions for this test:
    // (C,C)->(D,C)->(D,R}->(D,E)->(D,D)
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data, 0);
}

// Just drop sender/receiver after creation.
#[test]
fn oneshot_drop_all() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (tx, rx) = toy_rt::oneshot::<u32>(&rt);
    }

    // State transitions for this test:
    // (C,C)->(C,D)->(D,D)
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation, but drop sender first.
#[test]
fn oneshot_drop_all_alt_order() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (tx, rx) = toy_rt::oneshot::<u32>(&rt);
        drop(tx);
    }

    // State transitions for this test:
    // (C,C)->(D,C)->(D,D)
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation.
#[test]
fn oneshot_recv_from_dropped() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (tx, rx) = toy_rt::oneshot::<u32>(&rt);
        drop(tx);
        rx.await.expect_err("receiver must receive error if sender is dropped");
    }

    // State transitions for this test:
    // (C,C)->(D,C)->(D,R}->(D,E)->(D,D)
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation.
#[test]
fn oneshot_send_to_dropped() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (mut tx, rx) = toy_rt::oneshot::<u32>(&rt);
        drop(rx);
        tx.send(42).await.expect_err("send must receive error if sender is dropped");
    }

    // State transitions for this test:
    // (C,C)->(C,D)->{R,D)->(E,D)->(D,D)
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

