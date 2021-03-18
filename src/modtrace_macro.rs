//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
//

// Module level tracing
#[macro_use]
macro_rules! modtrace {
    ($fmt_str:tt)
        => ( if (MODTRACE) { println!(concat!("aiur::", $fmt_str)) });
    ($fmt_str:tt, $($x:expr),* )
        => ( if (MODTRACE) { println!(concat!("aiur::", $fmt_str), $($x),* ) });
}

