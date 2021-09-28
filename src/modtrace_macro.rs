//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
//

// Module level tracing
macro_rules! modtrace {
    ($log:expr, $msg:tt)
        => ( if (MODTRACE) { $log.fmt(format_args!($msg)) });
    ($log:expr, $fmt_str:tt, $($x:expr),* )
        => ( if (MODTRACE) { $log.fmt(format_args!($fmt_str, $($x),*)) } );
}
