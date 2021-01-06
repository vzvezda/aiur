//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::time::Duration;

// Use the toy runtime
use aiur::toy_rt::{self};

// With emulated sleep test run instantly, actual sleep actually wait for specified 
// amount of time.

//const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Actual;
const SLEEP_MODE: toy_rt::SleepMode = toy_rt::SleepMode::Emulated;

// Just makes sure that with_runtime() somehow works
#[test]
fn toy_runtime_fn_works() {
    async fn async_42(_rt: &toy_rt::Runtime, _: ()) -> u32 {
        42
    }

    assert_eq!(toy_rt::with_runtime_in_mode(SLEEP_MODE, async_42, ()), 42);
}

// Verifies that with_runtime() works when InitT is a reference
#[test]
fn toy_runtime_reference_compiles() {
    let mut param : u32 = 0;

    async fn async_42ref(_rt: &toy_rt::Runtime, pref: &mut u32) {
        *pref = 42;
    }

    // First version of with_runtime() machinery did not work with parameter references,
    // so this test was introduced. No with with_runtime() works more less as expected.
    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_42ref, &mut param);
    assert_eq!(param, 42);
}

// Verifies that with_runtime() works when InitT is a reference and RetT is a reference as well
#[test]
fn toy_runtime_reference_in_return_type_compiles() {
    let param = (40u32, 2u8);

    // async function takes a param a reference and return a reference as well
    async fn async_ref_returned<'i>(_rt: &toy_rt::Runtime, pref: &'i (u32, u8)) -> &'i u8 {
        &(*pref).1
    }
    let ret = toy_rt::with_runtime_in_mode(SLEEP_MODE, async_ref_returned, &param);
    assert_eq!(*ret, 2);
}

// Verifies that with_runtime() works with mutable references
#[test]
fn toy_runtime_mutable_references_in_param_return() {
    let mut param = (0u32, 0u8);

    // async function takes a param a reference and return a reference as well
    async fn async_ref_returned<'i>(_rt: &toy_rt::Runtime, pref: &'i mut (u32, u8)) -> &'i mut u8 {
        (*pref).0 = 42;
        (*pref).1 = 42;

        &mut (*pref).1
    }
    let ret = toy_rt::with_runtime_in_mode(SLEEP_MODE, async_ref_returned, &mut param);
    assert_eq!(*ret, 42);
    assert_eq!(param.0, 42);
    assert_eq!(param.1, 42);
    //assert_eq!(*ret, 42); // - does not compile, immutable borrow above
}

async fn async_sleep_once(rt: &toy_rt::Runtime, seconds: u32) {
    let now = rt.io().now32();
    toy_rt::sleep(rt, Duration::from_secs(seconds as u64)).await;
    assert!(
        rt.io().now32() >= now + seconds * 1000,
        "Delay did not happen"
    );
}

// Make sure that sleep is somehow works
#[test]
fn simple_sleep_works() {
    toy_rt::with_runtime_in_mode(SLEEP_MODE, async_sleep_once, 1);
}

// Make sure that consequential sleeps work
#[test]
fn consequential_sleeps_works() {
    // verify that we can schedule timer one by one
    async fn multi_sleep(rt: &toy_rt::Runtime, seconds: u32) {
        async_sleep_once(rt, seconds).await;
        async_sleep_once(rt, seconds).await;
        async_sleep_once(rt, seconds).await;
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, multi_sleep, 1);
}

mod measure {
    use std::cell::Cell;

    pub(super) struct MutCounter {
        counter: Cell<u32>,
    }

    impl MutCounter {
        pub(super) fn new(init: u32) -> Self {
            MutCounter {
                counter: Cell::new(init),
            }
        }

        pub(super) fn inc(&self) {
            self.counter.set(self.get() + 1);
        }

        pub(super) fn dec(&self) {
            self.counter.set(self.get() - 1);
        }

        pub(super) fn get(&self) -> u32 {
            self.counter.get()
        }
    }
}

// this is how I start creating my own version of 'futures' crate...
mod future_utils {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    // -----------------------------------------------------------------------------------------
    // PollInnerOnceFuture
    //
    // Future that polls the inner future only once and then signals that it is ready. We need
    // this to test cancellation
    // -----------------------------------------------------------------------------------------
    struct PollInnerOnceFuture<FutureT: Future> {
        inner: FutureT,
    }

    impl<FutureT: Future> PollInnerOnceFuture<FutureT> {
        fn new(inner: FutureT) -> Self {
            PollInnerOnceFuture { inner }
        }

        fn get_inner_future(self: Pin<&mut Self>) -> Pin<&mut FutureT> {
            //  This is okay because `field` is pinned when `self` is.
            unsafe { self.map_unchecked_mut(|s| &mut s.inner) }
        }
    }

    impl<FutureT: Future> Future for PollInnerOnceFuture<FutureT> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
            self.get_inner_future().poll(ctx);
            Poll::Ready(())
        }
    }

    // -----------------------------------------------------------------------------------------
    // Any2Void
    //
    // -----------------------------------------------------------------------------------------
    struct Any2Void<FutureT1: Future, FutureT2: Future> {
        f1: FutureT1,
        f2: FutureT2,
    }

    impl<FutureT1: Future, FutureT2: Future> Any2Void<FutureT1, FutureT2> {
        fn new(f1: FutureT1, f2: FutureT2) -> Self {
            Any2Void { f1, f2 }
        }

        fn get_f1(&mut self) -> Pin<&mut FutureT1> {
            //  This is okay because `field` is pinned when `self` is.
            unsafe { Pin::new_unchecked(&mut self.f1) }
        }

        fn get_f2(&mut self) -> Pin<&mut FutureT2> {
            //  This is okay because `field` is pinned when `self` is.
            unsafe {
                Pin::new_unchecked(&mut self.f2)
            }
        }
    }

    impl<FutureT1: Future, FutureT2: Future> Future for Any2Void<FutureT1, FutureT2> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
            let this = unsafe { self.get_unchecked_mut() };

            match this.get_f1().poll(ctx) {
                Poll::Pending => match this.get_f2().poll(ctx) {
                    Poll::Pending => return Poll::Pending,
                    _ => return Poll::Ready(()),
                },
                _ => return Poll::Ready(())
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Public accessors to Future util structs
    //
    // -----------------------------------------------------------------------------------------
    pub async fn poll_inner_once<FutureT: Future>(inner: FutureT) {
        PollInnerOnceFuture::new(inner).await;
    }

    pub async fn any2void<FutureT1: Future, FutureT2: Future>(f1: FutureT1, f2: FutureT2) {
        Any2Void::new(f1, f2).await;
    }
}

// Verify that cancellation works
#[test]
fn sleep_cancellation_works() {
    async fn cancel_fn(rt: &toy_rt::Runtime, seconds: u32) {
        future_utils::poll_inner_once(async_sleep_once(rt, seconds)).await;
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, cancel_fn, 1);
}

// Verify that cancellation works
#[test]
fn two_concurrent_timers_works() {
    async fn start_concurrent(rt: &toy_rt::Runtime, _: ()) {

        let start = rt.io().now32();

        future_utils::any2void(
            async_sleep_once(rt, 1),
            async_sleep_once(rt, 5)
            ).await;

        let elapsed = rt.io().now32() - start;

        assert!(elapsed >= 1*1000);
        assert!(elapsed <  5*1000);
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, start_concurrent, ());
}

// Verify spawn in a linear pattern works. Linear pattern is when an async function spawns
// another async funciton which in turn spawn any async function, etc up to some depth.
#[test]
fn spawn_linear_works() {
    use measure::MutCounter;

    let counter = MutCounter::new(0);

    const MAX_DEPTH: u32 = 10;

    async fn measured(rt: &toy_rt::Runtime, counter: &MutCounter) {
        let start = rt.io().now32();
        deep_dive(rt, MAX_DEPTH, counter).await;
        let elapsed = rt.io().now32() - start;
        assert!(elapsed >= 1 * 1000);
    }

    async fn deep_dive(rt: &toy_rt::Runtime, depth: u32, counter: &MutCounter) {
        if depth == 0 {
            async_sleep_once(rt, 1).await;
        } else {
            counter.inc();
            let mut scope = toy_rt::Scope::new(rt);
            scope.spawn(deep_dive(rt, depth - 1, counter));
        }
    }

    toy_rt::with_runtime_in_mode(SLEEP_MODE, measured, &counter);
    assert_eq!(counter.get(), MAX_DEPTH, "Unexpected dive depth");
}
