//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::task::Waker;
use std::time::{Duration, Instant};

use crate::EventId;
use crate::Reactor;
use crate::TemporalReactor;

/// ToyReactor comes with a init parameter called SleepMode. To improve development and testing
/// cycles sleep can work in Emulated mode when it does not actually sleep.
#[derive(Copy, Clone)]
pub enum SleepMode {
    Actual,   // Makes actual delays, e.g. sleep(5s) seconds actually waits 5 sec before wake
    Emulated, // Does not wait, shoot timer right away in a relative sorted order
}

/// ToyReactor is reactor that can only schedule timers.
///
/// It is used for testing the executor as a lot of I/O can be emulated just by sleeping 
/// (by pretending that we are reading something from network).
pub struct ToyReactor {
    // Reactor is a part of Runtime which is only allowed to work as non-unique ref, so
    // we have to make it with interior mutability.
    //
    // It probably make sense to use UnsafeCell in release mode to save some performance, but
    // this is the toy reactor.
    rimpl: RefCell<ToyReactorImpl>,
}

// The impl of reactor is more about forwarding to a method with &mut self.
// Toy reactor expired in 49 days and you have to re-launch it.
impl ToyReactor {
    /// Creates ToyReactor with actual sleep mode that would invoke [std::thread::sleep]().
    pub fn new() -> Self {
        Self::new_with_mode(SleepMode::Actual)
    }

    /// Creates ToyReactor with sleep mode provided as a parameter.
    pub fn new_with_mode(mode: SleepMode) -> Self {
        ToyReactor {
            rimpl: RefCell::new(ToyReactorImpl::new(mode)),
        }
    }

    /// Returns a monotonically increasing current time in milliseconds. It can be used
    /// for testing to verify if the time of sleep() actually passes. In emulated sleep
    /// mode the value this function returns is also emulated.
    pub fn now32(&self) -> u32 {
        self.rimpl.borrow_mut().now32()
    }
}

impl Reactor for ToyReactor {
    fn wait(&self) -> EventId {
        self.rimpl.borrow_mut().wait()
    }
}

impl TemporalReactor for ToyReactor {
    fn schedule_timer(&self, waker: Waker, event_id: EventId, duration: Duration) {
        self.rimpl
            .borrow_mut()
            .schedule_timer(waker, event_id, duration);
    }

    fn cancel_timer(&self, event_id: EventId) {
        self.rimpl.borrow_mut().cancel_timer(event_id);
    }
}

// This is the data struct that describes a scheduled timer in our toy reactor.
struct TimerNode {
    wake_on: u32,
    event_id: EventId,
    waker: Waker,
    // The BinaryHeap does not support element deletion, so we just mark if node was
    // deleted. TODO: a more optimal mark mode (e.g. nullable eventid?)
    cancelled: Cell<bool>,
}

impl TimerNode {
    fn new(now32: u32, duration: Duration, waker: Waker, event_id: EventId) -> Self {
        TimerNode {
            wake_on: now32 + Self::get_duration_u32(duration),
            event_id,
            waker,
            cancelled: Cell::new(false),
        }
    }

    fn cancel(&self) {
        self.cancelled.set(true);
    }

    // Ensure that sleep duration is no longer then MAX_TIMER_DURATION_MS (24h)
    fn get_duration_u32(duration: Duration) -> u32 {
        let duration: u128 = duration.as_millis();

        assert!(
            duration <= ToyReactor::MAX_TIMER_DURATION_MS as u128,
            "aiur: Sleep duration is too big: {}ms (max is {}ms)",
            duration,
            ToyReactor::MAX_TIMER_DURATION_MS
        );

        duration as u32
    }
}

// Ordering for TimerNode to work with binary heap (reversed)
impl PartialOrd for TimerNode {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.wake_on.cmp(&rhs.wake_on).reverse())
    }
}

impl PartialEq for TimerNode {
    fn eq(&self, rhs: &Self) -> bool {
        self.wake_on == rhs.wake_on
    }
}

impl Eq for TimerNode {}
impl Ord for TimerNode {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.partial_cmp(rhs).unwrap()
    }
}

// This private version of SleepMode that hides how the timer works: e.g. if it is
// emulated or actual system timer.
enum SleepModeImpl {
    Actual { system_now32_origin: Instant },
    Emulated { emulated_now32: u32 },
}

impl SleepModeImpl {
    fn now32(&self) -> u32 {
        match self {
            SleepModeImpl::Actual {
                system_now32_origin,
            } => system_now32_origin.elapsed().as_millis() as u32,
            SleepModeImpl::Emulated { emulated_now32 } => *emulated_now32,
        }
    }

    // Does actual or emulated sleep depending on reactor's mode
    fn sleep(&mut self, ms: u32) {
        match self {
            SleepModeImpl::Actual { .. } => Self::actual_sleep(ms),
            SleepModeImpl::Emulated { emulated_now32 } => Self::emulated_sleep(emulated_now32, ms),
        }
    }

    // In emulated sleep we do not sleep, just increase the timer as if time has passed.
    fn emulated_sleep(emulated_now32: &mut u32, ms: u32) {
        *emulated_now32 += ms;
    }

    // In actual sleep suspend the thread
    fn actual_sleep(ms: u32) {
        std::thread::sleep(Duration::from_millis(ms as u64));
    }
}

// Init SleepModeImpl from SleepMode
impl From<SleepMode> for SleepModeImpl {
    fn from(sleep_mode: SleepMode) -> Self {
        match sleep_mode {
            SleepMode::Actual => SleepModeImpl::Actual {
                system_now32_origin: Instant::now(),
            },
            SleepMode::Emulated => SleepModeImpl::Emulated { emulated_now32: 0 },
        }
    }
}

struct ToyReactorImpl {
    // Timers are stored as binary heap, so we always know what is the first timer
    timers: BinaryHeap<TimerNode>,
    sleep_mode: SleepModeImpl,
}

impl ToyReactorImpl {
    fn new(sleep_mode: SleepMode) -> Self {
        ToyReactorImpl {
            timers: BinaryHeap::new(),
            sleep_mode: SleepModeImpl::from(sleep_mode),
        }
    }

    fn schedule_timer(&mut self, waker: Waker, event_id: EventId, duration: Duration) {
        println!("schedule_timer: {:?}", event_id);

        self.timers
            .push(TimerNode::new(self.now32(), duration, waker, event_id));
    }

    fn cancel_timer(&mut self, event_id: EventId) {
        println!("cancel_timer: {:?}", event_id);
        // It is not possible to remove an element from BinaryHeap, so we just mark the timer
        // that it was cancelled. It does not change the ordering, so it should be ok (see
        // BinaryHeap docs that says that modifying ordering is a logic error).
        //
        // Another issue is what we should do if timer we are about to delete is not exist.
        // This reactor just panic because it probably some kind of bug to be fixed.
        self.timers
            .iter()
            .find(|x| {
                println!("{:?}", x.event_id);
                x.event_id == event_id
            })
            .expect("Attempt to remove unknown timer")
            .cancel();
    }

    fn now32(&self) -> u32 {
        self.sleep_mode.now32()
    }

    // Removes a
    fn get_first_timer_to_wake(&mut self) -> TimerNode {
        // Let require runtime to invoke wait() only if there something to wait. Runtime
        // knows if there are any active tasks, so it don't invoke wait when there is nothing
        // to wake.
        //
        // skip all canceled timers
        loop {
            let timer_node = self
                .timers
                .pop()
                // Maybe when there is nothing to wait Reactor can just return a null event_id, but
                // let keep it strict for a while and see if this will work out.
                .expect(concat!(
                    "aiur: ToyReactor::wait() invoked with nothing to wait. ",
                    "It looks like some kind a bug in aiur::Runtime that it invoked wait(), ",
                    "knowing that there is nothing to wait."
                ));

            if !timer_node.cancelled.get() {
                return timer_node;
            }
        }
    }

    fn wait(&mut self) -> EventId {
        println!("toy reactor wait");
        let timer_node = self.get_first_timer_to_wake();

        let now32 = self.now32();

        // check if sleep is required for timer
        if timer_node.wake_on > now32 {
            self.sleep_mode.sleep(timer_node.wake_on - now32);
        }

        // Returns the waker and event_id to aiur::Runtime
        timer_node.waker.wake_by_ref();
        timer_node.event_id
    }
}
