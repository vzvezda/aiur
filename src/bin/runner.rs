//  ( /   @ @    ()  Aiur - the home planet of some famous executors
//   \  __| |__  /   (c) 2020 - present, Vladimir Zvezda
//    -/   "   \-
//
use aiur::{self};

async fn async_main(_rt: &aiur::Runtime) -> u32 {
    42
}

fn main() {
    // We cannot use a closure here as for rust 1.44, see
    // https://github.com/rust-lang/rust/issues/58052
    let res = aiur::with_runtime(async_main);

    println!("Result is {}", res);
}
