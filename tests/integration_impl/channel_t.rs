//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
//
// This module contains channel tests via publicly exposed API. There is also private API unit
// tests for channel in crate.

// Use the toy runtime
use super::future_utils::{self};
use aiur::toy_rt::{self};

// With emulated sleep tests are run instantly, with actual sleep mode it wait for specified
// amount of time.

//const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

// Simple tests that send values from sender to reader.
#[test]
fn channel_works() {
    struct AsyncState {
        recv_data: Vec<u32>,
    }

    async fn reader<'runtime, 'state>(
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
            let (mut tx, rx) = toy_rt::channel::<u32>(&rt);

            toy_rt::join!(
                async {
                    for i in 0..10u32 {
                        tx.send(i).await.unwrap();
                    }
                    drop(tx); // drop sender to stop the reader()'s cycle
                },
                reader(rx, &mut state)
            )
            .await;
        }
        state
    }

    // State transitions for this test:
    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());

    assert_eq!(state.recv_data.len(), 10);
}

/// Just drops sender/receiver channel side after creation without pinning.
#[test]
fn channel_drop_sender_recver_unpinned_is_ok() {
    async fn drop_recv_first(rt: &toy_rt::Runtime, _: ()) {
        let (_tx, _rx) = toy_rt::channel::<u32>(&rt);
        // dropped _rx, then _tx
    }

    async fn drop_sender_first(rt: &toy_rt::Runtime, _: ()) {
        let (tx, _rx) = toy_rt::channel::<u32>(&rt);
        drop(tx); // first drop tx
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, drop_recv_first, ());
    toy_rt::with_runtime_in_mode(SLEEP_MODE, drop_sender_first, ());
}

/// It was ever first test to verify if two simultaneous channels can co-exists and it was
/// introduced with support in runtime.
#[test]
fn channel_two_channels_can_coexist() {
    struct AsyncState {
        recv1_data: u32,
        recv2_data: u32,
    }

    async fn writer<'runtime, 'state>(mut tx: toy_rt::Sender<'runtime, u32>, value: u32) {
        tx.send(value).await.unwrap();
    }

    async fn start_async(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState {
            recv1_data: 0,
            recv2_data: 0,
        };
        {
            let (tx1, mut rx1) = toy_rt::channel::<u32>(&rt);
            let (tx2, mut rx2) = toy_rt::channel::<u32>(&rt);
            toy_rt::join!(writer(tx1, 42), writer(tx2, 100), async {
                state.recv1_data = rx1.next().await.unwrap();
                state.recv2_data = rx2.next().await.unwrap();
            })
            .await;
        }
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, start_async, ());

    // Verify that that sent data was actually read by receiver.
    assert_eq!(state.recv1_data, 42);
    assert_eq!(state.recv2_data, 100);
}

/// Launches sender/receiver in a select!()-like mode, so once the first (receiver) is completed,
/// the sender is dropped without being able to extract result.
#[test]
fn channel_select_sender_receiver_gives_value_in_receiver() {
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

    async fn start_async(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { recv_data: 0 };
        let (tx, rx) = toy_rt::channel::<u32>(&rt);
        future_utils::any2void(reader(rx, &mut state), writer(tx)).await;
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, start_async, ());

    assert_eq!(state.recv_data, 42);
}

/// When recver side of the channel is dropped while having pinned sender. Test verifies that
/// Sender receives an error and the unsent value back.
#[test]
fn channel_drop_recver_with_sender_pinned_gives_error() {
    async fn reader<'runtime>(_rx: toy_rt::Recver<'runtime, u32>) {
        // do thing on recv side,
        // dropping _rx
    }

    async fn start_async(rt: &toy_rt::Runtime, _: ()) {
        let (mut tx, rx) = toy_rt::channel::<u32>(&rt);
        toy_rt::join!(reader(rx), async {
            // verify that sender receiver the value back as error
            assert_eq!(tx.send(42).await.unwrap_err(), 42);
        })
        .await;
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, start_async, ());
}

/// When sender side of the channel is dropped while having pinned recver. In this case receiver
/// gets an error about disconnected channel.
#[test]
fn channel_drop_sender_with_recver_pinned_gives_error() {
    async fn reader<'runtime>(mut rx: toy_rt::Recver<'runtime, u32>) {
        rx.next()
            .await
            .expect_err("Expected error because sender is dropped");
    }

    async fn start_async(rt: &toy_rt::Runtime, _: ()) {
        let (tx, rx) = toy_rt::channel::<u32>(&rt);
        toy_rt::join!(reader(rx), async move { drop(tx) }).await;
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, start_async, ());
}

/// Drops the sender side and verifies if pinned recver gets an error.
#[test]
fn channel_recv_from_dropped_gives_error() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (tx, mut rx) = toy_rt::channel::<u32>(&rt);
        drop(tx);
        rx.next()
            .await
            .expect_err("receiver must receive error if sender is dropped");
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

/// Drops the recver side and verifies if pinned sender gets an error.
#[test]
fn channel_send_to_dropped_gives_error() {
    async fn messenger(rt: &toy_rt::Runtime, _: ()) {
        let (mut tx, rx) = toy_rt::channel::<u32>(&rt);
        drop(rx);
        tx.send(42)
            .await
            .expect_err("send must receive error if sender is dropped");
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, messenger, ());
}

/// My second echo server that is able send received values back as long as its recv channel is
/// alive.
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
            println!("Echo server value {}", value);
            tx.send(value).await.unwrap();
        }
    }

    async fn echo_client(rt: &toy_rt::Runtime, _: ()) -> AsyncState {
        let mut state = AsyncState { echo_data: 0 };
        {
            let (tx1, mut rx1) = toy_rt::channel::<u32>(&rt);
            let (mut tx2, rx2) = toy_rt::channel::<u32>(&rt);
            toy_rt::join!(echo_server(tx1, rx2), async {
                println!("Echo client sending 42");
                tx2.send(42).await.unwrap();
                println!("Echo client sending 42 ok");
                state.echo_data = rx1.next().await.unwrap();
                println!("Echo client recv 42 ok, sending 8");
                tx2.send(8).await.unwrap();
                println!("Echo client sending 8 ok");
                state.echo_data += rx1.next().await.unwrap();
                println!("Echo client recevied");
                // need to drop sender to signal to echo_server() that it can exit
                drop(tx2);
            })
            .await;
        }
        state
    }

    let state = toy_rt::with_runtime_in_mode(SLEEP_MODE, echo_client, ());

    // Verify that that sent data was actually read by receiver.
    assert_eq!(state.echo_data, 42 + 8);
}

/// This test produces a some channel exchanges with multiple channels and while each channel
/// has multiple senders.
#[test]
fn channel_many_channel_many_senders_actually_works() {
    async fn generate<'rt>(mut tx: toy_rt::Sender<'rt, u32>) {
        for i in 1..5 {
            tx.send(i).await.unwrap()
        }
    }

    async fn accumulate(rt: &toy_rt::Runtime, name: &'static str) {
        let (tx, mut rx) = toy_rt::channel::<u32>(&rt);
        toy_rt::join!(
            generate(tx.clone()),
            generate(tx.clone()),
            generate(tx.clone()),
            async {
                drop(tx);

                let mut accum = 0;
                while let Ok(value) = rx.next().await {
                    accum += value;
                }

                assert_eq!(accum, 30);
            }
        )
        .await;
    }

    async fn start_multi_senders(rt: &toy_rt::Runtime, _: ()) {
        // Each accumulate is 1 recv + 3 senders
        {
            toy_rt::join!(
                accumulate(rt, "accum1"),
                accumulate(rt, "accum3"),
                accumulate(rt, "accum2")
            )
            .await;
        }
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, start_multi_senders, ());
}

/// Drops a pinned peer while channel exchange in progress
#[test]
fn channel_drop_pinned_peer_during_exchange_is_ok() {
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
        // 3 sends in sender, 4 in receiver. The send_3 exits first making the recv_4 future
        // dropped.
        let (tx, rx) = toy_rt::channel::<u32>(&rt);
        future_utils::any2void(send_3(tx), recv_4(rx)).await;

        // 3 sends in sender, 2 in receiver. The recv_2 exists first making the send3 future
        // dropped.
        let (tx, rx) = toy_rt::channel::<u32>(&rt);
        future_utils::any2void(send_3(tx), recv_2(rx)).await;
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, start_dropping_peers, ());
}
