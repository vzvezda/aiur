//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use core::fmt::Arguments;

#[derive(Clone)]
pub struct Tracer {
    log_fn: fn(usize, Arguments),
    data: usize,
}

fn local_print(_data: usize, args: Arguments) {
    // We can improve to avoid dynamic memory, but this function currently only used for
    // testing.
    println!("aiur/{}", args.to_string());
}

fn local_nothing(_data: usize, _args: Arguments) {}

impl Tracer {
    pub fn new(data: usize, log_fn: fn(usize, Arguments)) -> Self {
        Self {
            log_fn: log_fn,
            data: data,
        }
    }

    pub fn new_empty() -> Self {
        Self::new(0, local_nothing)
    }

    pub(crate) fn new_testing() -> Self {
        Self::new(0, local_print)
    }

    pub fn fmt(&self, args: Arguments) {
        (self.log_fn)(self.data, args);
    }
}
