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

// Spawns a task and send a channel value from parent to child tasks.
#[cfg(disable_this_for_a_while = "mut")]
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
        let mut state = AsyncState {
            recv_data: Vec::new(),
        };
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

// First test to verify if two simultaneous channel can co-exists and it was introduced with
// support in runtime.
#[test]
fn channel_two_channels() {
    struct AsyncState {
        recv1_data: u32,
        recv2_data: u32,
    }

    async fn writer<'runtime, 'state>(
        rt: &'runtime toy_rt::Runtime,
        mut tx: toy_rt::ChSender<'runtime, u32>,
        value: u32,
    ) {
        tx.send(value).await.unwrap();
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState {
            recv1_data: 0,
            recv2_data: 0,
        };
        {
            let mut scope = toy_rt::Scope::new_named(rt, "TwoOneshots");
            let (tx1, mut rx1) = toy_rt::channel::<u32>(&rt);
            let (tx2, mut rx2) = toy_rt::channel::<u32>(&rt);
            scope.spawn(writer(rt, tx1, 42));
            scope.spawn(writer(rt, tx2, 100));
            state.recv1_data = rx1.next().await.unwrap();
            state.recv2_data = rx2.next().await.unwrap();
        }
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    // Verify that that sent data was actually read by receiver.
    assert_eq!(state.recv1_data, 42);
    assert_eq!(state.recv2_data, 100);
}

// Launch sender/receiver in a select!()-like mode, so once the first (receiver) is complete
// the sender is dropped. 
#[test]
fn channel_select_works() {
    struct AsyncState {
        recv_data: u32,
    }

    async fn reader<'runtime, 'state>(
        mut rx: toy_rt::ChReceiver<'runtime, u32>,
        state: &'state mut AsyncState,
    ) {
        state.recv_data = rx.next().await.unwrap();
    }

    async fn writer<'runtime>(mut tx: toy_rt::ChSender<'runtime, u32>) {
        tx.send(42).await.unwrap();
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        let (mut tx, rx) = toy_rt::channel::<u32>(&rt);
        future_utils::any2void(reader(rx, &mut state), writer(tx)).await;
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data, 42);
}


// When future that suppose to receive a channel just dropped. In this case sender
// should return the value back.
#[test]
fn channel_recv_dropped() {
    struct AsyncState {
        recv_data: u32,
    }

    async fn reader<'runtime, 'state>(
        rt: &'runtime toy_rt::Runtime,
        rx: toy_rt::ChReceiver<'runtime, u32>,
        state: &'state mut AsyncState,
    ) {
        // do thing on recv side
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        {
            let mut scope = toy_rt::Scope::new_named(rt, "Messenger");
            let (mut tx, rx) = toy_rt::channel::<u32>(&rt);
            scope.spawn(reader(rt, rx, &mut state));

            // verify that sender receiver the value back as error
            assert_eq!(tx.send(42).await.unwrap_err(), 42);
        }
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data, 0);
}

// When future that suppose to send a channel just dropped. In this case receiver 
// receiver an error.
#[test]
fn channel_sender_dropped() {
    struct AsyncState {
        recv_data: u32,
    }

    async fn reader<'runtime, 'state>(
        rt: &'runtime toy_rt::Runtime,
        mut rx: toy_rt::ChReceiver<'runtime, u32>,
        state: &'state mut AsyncState,
    ) {
        rx.next().await.expect_err("Error because sender is dropped");
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        {
            let mut scope = toy_rt::Scope::new_named(rt, "Messenger");
            let (mut tx, rx) = toy_rt::channel::<u32>(&rt);
            scope.spawn(reader(rt, rx, &mut state));
        }
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data, 0);
}

// Just drop sender/receiver after creation.
#[test]
fn channel_drop_all() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (tx, rx) = toy_rt::channel::<u32>(&rt);
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation, but drop sender first.
#[test]
fn channel_drop_all_alt_order() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (tx, rx) = toy_rt::channel::<u32>(&rt);
        drop(tx);
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation.
#[test]
fn channel_recv_from_dropped() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (tx, mut rx) = toy_rt::channel::<u32>(&rt);
        drop(tx);
        rx.next().await.expect_err("receiver must receive error if sender is dropped");
    }

    // State transitions for this test:
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation.
#[test]
fn channel_send_to_dropped() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (mut tx, rx) = toy_rt::channel::<u32>(&rt);
        drop(rx);
        tx.send(42).await.expect_err("send must receive error if sender is dropped");
    }

    // State transitions for this test:
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}



// Echo server that is able send response as long as sender alive.
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
        let mut state = AsyncState { echo_data: 0 };
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
    assert_eq!(state.echo_data, 42 + 8);
}
