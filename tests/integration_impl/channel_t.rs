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
        mut rx: toy_rt::Recver<'runtime, u32>,
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
            let scope = toy_rt::Scope::new_named(rt, "ChannelWorks");
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

// First test to verify if two simultaneous channel can co-exists and it was introduced with
// support in runtime.
#[test]
fn channel_two_channels() {
    struct AsyncState {
        recv1_data: u32,
        recv2_data: u32,
    }

    async fn writer<'runtime, 'state>(mut tx: toy_rt::Sender<'runtime, u32>, value: u32) {
        tx.send(value).await.unwrap();
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState {
            recv1_data: 0,
            recv2_data: 0,
        };
        {
            let scope = toy_rt::Scope::new_named(rt, "TwoOneshots");
            let (tx1, mut rx1) = toy_rt::channel::<u32>(&rt);
            let (tx2, mut rx2) = toy_rt::channel::<u32>(&rt);
            scope.spawn(writer(tx1, 42));
            scope.spawn(writer(tx2, 100));
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
        mut rx: toy_rt::Recver<'runtime, u32>,
        state: &'state mut AsyncState,
    ) {
        state.recv_data = rx.next().await.unwrap();
    }

    async fn writer<'runtime>(mut tx: toy_rt::Sender<'runtime, u32>) {
        tx.send(42).await.unwrap();
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        let (tx, rx) = toy_rt::channel::<u32>(&rt);
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
    async fn reader<'runtime>(_rx: toy_rt::Recver<'runtime, u32>) {
        // do thing on recv side
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
            let scope = toy_rt::Scope::new_named(rt, "Messenger");
            let (mut tx, rx) = toy_rt::channel::<u32>(&rt);
            scope.spawn(reader(rx));

            // verify that sender receiver the value back as error
            assert_eq!(tx.send(42).await.unwrap_err(), 42);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// When future that suppose to send a channel just dropped. In this case receiver
// receiver an error.
#[test]
fn channel_sender_dropped() {
    async fn reader<'runtime>(mut rx: toy_rt::Recver<'runtime, u32>) {
        rx.next()
            .await
            .expect_err("Error because sender is dropped");
    }

    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let scope = toy_rt::Scope::new_named(rt, "Messenger");
        let (_tx, rx) = toy_rt::channel::<u32>(&rt);
        scope.spawn(reader(rx));
        // dropping _tx
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation.
#[test]
fn channel_drop_all() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (_tx, _rx) = toy_rt::channel::<u32>(&rt);
        // dropped _rx, then _tx
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation, but drop sender first.
#[test]
fn channel_drop_all_alt_order() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (tx, _rx) = toy_rt::channel::<u32>(&rt);
        drop(tx); // first drop tx
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation.
#[test]
fn channel_recv_from_dropped() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (tx, mut rx) = toy_rt::channel::<u32>(&rt);
        drop(tx);
        rx.next()
            .await
            .expect_err("receiver must receive error if sender is dropped");
    }

    // State transitions for this test:
    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Just drop sender/receiver after creation.
#[test]
fn channel_send_to_dropped() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (mut tx, rx) = toy_rt::channel::<u32>(&rt);
        drop(rx);
        tx.send(42)
            .await
            .expect_err("send must receive error if sender is dropped");
    }

    // State transitions for this test:
    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

// Echo server that is able send response as long as sender alive.
#[test]
fn channel_echo_server() {
    struct AsyncState {
        echo_data: u32,
    }

    async fn echo_server<'runtime, 'state>(
        mut tx: toy_rt::Sender<'runtime, u32>,
        mut rx: toy_rt::Recver<'runtime, u32>,
    ) {
        while let Ok(value) = rx.next().await {
            tx.send(value).await.unwrap();
        }
    }

    async fn echo_client(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { echo_data: 0 };
        {
            let scope = toy_rt::Scope::new_named(rt, "Echo");
            let (tx1, mut rx1) = toy_rt::channel::<u32>(&rt);
            let (mut tx2, rx2) = toy_rt::channel::<u32>(&rt);
            scope.spawn(echo_server(tx1, rx2));
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

// This test produce a lot of channel exchange when there are several channels 
// and channels has multiple senders.
#[test]
fn channel_multi_senders() {
    async fn generate<'rt>(mut tx: toy_rt::Sender<'rt, u32>) {
        for i in 1..5 {
            tx.send(i).await.unwrap()
        }
    }

    async fn accumulate(rt: &toy_rt::Runtime, name: &'static str) {
        let scope = toy_rt::Scope::new_named(rt, name);
        let (tx, mut rx) = toy_rt::channel::<u32>(&rt);
        scope.spawn(generate(tx.clone()));
        scope.spawn(generate(tx.clone()));
        scope.spawn(generate(tx.clone()));
        drop(tx);

        let mut accum = 0;
        while let Ok(value) = rx.next().await {
            accum += value;
        }

        assert_eq!(accum, 30);
    }

    async fn start_multi_senders(rt: &toy_rt::Runtime, _: ()) {
        // Each accumulate is 1 recv + 3 senders
        let scope = toy_rt::Scope::new_named(rt, "parent");
        scope.spawn(accumulate(rt, "accum1"));
        scope.spawn(accumulate(rt, "accum2"));
        scope.spawn(accumulate(rt, "accum3"));
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, start_multi_senders, ());
}


#[test]
fn channel_drop_peer() {
    async fn send_3<'rt>(mut tx: toy_rt::Sender<'rt, u32>) {
        tx.send(1).await.unwrap();
        tx.send(2).await.unwrap();
        tx.send(3).await.unwrap();
    }

    async fn recv_4<'rt>(mut rx: toy_rt::Recver<'rt, u32>) {
        assert_eq!(rx.next().await.unwrap(), 1);
        assert_eq!(rx.next().await.unwrap(), 2);
        assert_eq!(rx.next().await.unwrap(), 3);
        rx.next().await.unwrap(); // feature will be dropped on await point
    }

    async fn recv_2<'rt>(mut rx: toy_rt::Recver<'rt, u32>) {
        assert_eq!(rx.next().await.unwrap(), 1);
        assert_eq!(rx.next().await.unwrap(), 2);
    }

    async fn start_dropping_peers(rt: &toy_rt::Runtime, _: ()) {
        // When a dropper receiver
        let (tx, rx) = toy_rt::channel::<u32>(&rt);
        future_utils::any2void(send_3(tx), recv_4(rx)).await;
        // When a drop
        let (tx, rx) = toy_rt::channel::<u32>(&rt);
        future_utils::any2void(send_3(tx), recv_2(rx)).await;
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, start_dropping_peers, ());
}

