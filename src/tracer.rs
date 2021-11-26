//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use core::fmt::Arguments;

/// A C-like callback which is a pointer to logger function and data.
///
/// App can specify tracing function when creating [`crate::Runtime`] using
/// [`crate::with_runtime_base()`] function, so all the tracing goes to app specific output.
///
/// Why using C-like callback? Well, I don't want aiur to depend on any logger crate, so API
/// has to be constructed from scratch. I have tried the approach that app should provide
/// `trait Tracer` and it made everything much more messy internally.
#[derive(Copy, Clone)]
pub struct Tracer {
    log_fn: fn(usize, Arguments),
    data: usize,
}

// Private impl of Tracer used in tests to print traces into stdout
fn local_print(_data: usize, args: Arguments) {
    // We can improve to avoid dynamic memory, but this function is private and only used for
    // testing.
    println!("aiur/{}", args.to_string());
}

// Impl of Tracer used in this crate when no traces are required
fn local_nothing(_data: usize, _args: Arguments) {}

impl Tracer {
    /// Constructs the tracer from data and function pointer. On a trace event aiur will invoke
    /// provided `log_fn` function with `data` in first argument.
    ///
    /// It is important that log_fn does not use the Runtime while processing the log events,
    /// this would cause panic when trying to borrow [RefCell](std::cell::RefCell) inside Runtime.
    pub fn new(data: usize, log_fn: fn(usize, Arguments)) -> Self {
        Self {
            log_fn: log_fn,
            data: data,
        }
    }

    /// Constructs the tracer when traces goes nowhere.
    pub fn new_empty() -> Self {
        Self::new(0, local_nothing)
    }

    // Constructs the tracer for testing that prints! the event.
    pub(crate) fn new_testing() -> Self {
        Self::new(0, local_print)
    }

    //
    pub(crate) fn fmt(&self, args: Arguments) {
        (self.log_fn)(self.data, args);
    }
}
