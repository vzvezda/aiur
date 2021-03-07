
// tracing for development
//#[macro_export]
#[macro_use]
macro_rules! modtrace {
    ($fmt_str:tt)
        => ( if (MODTRACE) { println!(concat!("aiur::", $fmt_str)) });
    ($fmt_str:tt, $($x:expr),* )
        => ( if (MODTRACE) { println!(concat!("aiur::", $fmt_str), $($x),* ) });
}

