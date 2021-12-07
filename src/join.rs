//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use crate::pinned_any_of;
use std::future::Future;

/// Waits concurrently until all futures are completed.
///
///
/// Internally it just select the correct `joinN()` function based on the number of agruments
/// supplied. For example the `join!(fut1, fut2, fut3).await` is the same as
/// [`join3`]`(fut1, fut2, fut3).await`.
///
/// Please note that unlike join implementation in other crates this one returns future and
/// requires `.await` to start execution.
#[macro_export]
macro_rules! join {
    ($f1:expr, $f2:expr $(,)?) => {
        $crate::join2($f1, $f2)
    };
    ($f1:expr, $f2:expr, $f3:expr $(,)?) => {
        $crate::join3($f1, $f2, $f3)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr $(,)?) => {
        $crate::join4($f1, $f2, $f3, $f4)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr $(,)?) => {
        $crate::join5($f1, $f2, $f3, $f4, $f5)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr $(,)?) => {
        $crate::join6($f1, $f2, $f3, $f4, $f5, $f6)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr, $f7:expr $(,)?) => {
        $crate::join7($f1, $f2, $f3, $f4, $f5, $f6, $f7)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr, $f7:expr, $f8:expr $(,)?) => {
        $crate::join8($f1, $f2, $f3, $f4, $f5, $f6, $f7, $f8)
    };
}

/// Polls two futures concurrently until both are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join2<FutT1, FutT2>(f1: FutT1, f2: FutT2) -> (FutT1::Output, FutT2::Output)
where
    FutT1: Future,
    FutT2: Future,
{
    pinned_any_of!(stream, f1, f2);
    let mut res = std::mem::MaybeUninit::<(FutT1::Output, FutT2::Output)>::uninit();
    unsafe {
        let ptr = res.as_mut_ptr();
        while let Some(val) = stream.next().await {
            match val {
                crate::OneOf2::First(x) => (*ptr).0 = x,
                crate::OneOf2::Second(x) => (*ptr).1 = x,
            }
        }
    }
    unsafe { res.assume_init() }
}

/// Polls three futures concurrently until all are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join3<FutT1, FutT2, FutT3>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
) -> (FutT1::Output, FutT2::Output, FutT3::Output)
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
{
    let mut res = std::mem::MaybeUninit::<(FutT1::Output, FutT2::Output, FutT3::Output)>::uninit();

    pinned_any_of!(stream, f1, f2, f3);

    unsafe {
        let ptr = res.as_mut_ptr();
        while let Some(val) = stream.next().await {
            match val {
                crate::OneOf3::First(x) => (*ptr).0 = x,
                crate::OneOf3::Second(x) => (*ptr).1 = x,
                crate::OneOf3::Third(x) => (*ptr).2 = x,
            }
        }
    }
    unsafe { res.assume_init() }
}

/// Polls four futures concurrently until all are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join4<FutT1, FutT2, FutT3, FutT4>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
    f4: FutT4,
) -> (FutT1::Output, FutT2::Output, FutT3::Output, FutT4::Output)
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
{
    let mut res = std::mem::MaybeUninit::<(
        FutT1::Output,
        FutT2::Output,
        FutT3::Output,
        FutT4::Output,
    )>::uninit();

    pinned_any_of!(stream, f1, f2, f3, f4);

    unsafe {
        let ptr = res.as_mut_ptr();
        while let Some(val) = stream.next().await {
            match val {
                crate::OneOf4::First(x) => (*ptr).0 = x,
                crate::OneOf4::Second(x) => (*ptr).1 = x,
                crate::OneOf4::Third(x) => (*ptr).2 = x,
                crate::OneOf4::Fourth(x) => (*ptr).3 = x,
            }
        }
    }
    unsafe { res.assume_init() }
}

/// Polls five futures concurrently until all are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join5<FutT1, FutT2, FutT3, FutT4, FutT5>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
    f4: FutT4,
    f5: FutT5,
) -> (
    FutT1::Output,
    FutT2::Output,
    FutT3::Output,
    FutT4::Output,
    FutT5::Output,
)
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
{
    let mut res = std::mem::MaybeUninit::<(
        FutT1::Output,
        FutT2::Output,
        FutT3::Output,
        FutT4::Output,
        FutT5::Output,
    )>::uninit();

    pinned_any_of!(stream, f1, f2, f3, f4, f5);

    unsafe {
        let ptr = res.as_mut_ptr();
        while let Some(val) = stream.next().await {
            match val {
                crate::OneOf5::First(x) => (*ptr).0 = x,
                crate::OneOf5::Second(x) => (*ptr).1 = x,
                crate::OneOf5::Third(x) => (*ptr).2 = x,
                crate::OneOf5::Fourth(x) => (*ptr).3 = x,
                crate::OneOf5::Fifth(x) => (*ptr).4 = x,
            }
        }
    }
    unsafe { res.assume_init() }
}

/// Polls six futures concurrently until all are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join6<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
    f4: FutT4,
    f5: FutT5,
    f6: FutT6,
) -> (
    FutT1::Output,
    FutT2::Output,
    FutT3::Output,
    FutT4::Output,
    FutT5::Output,
    FutT6::Output,
)
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
{
    let mut res = std::mem::MaybeUninit::<(
        FutT1::Output,
        FutT2::Output,
        FutT3::Output,
        FutT4::Output,
        FutT5::Output,
        FutT6::Output,
    )>::uninit();

    pinned_any_of!(stream, f1, f2, f3, f4, f5, f6);

    unsafe {
        let ptr = res.as_mut_ptr();
        while let Some(val) = stream.next().await {
            match val {
                crate::OneOf6::First(x) => (*ptr).0 = x,
                crate::OneOf6::Second(x) => (*ptr).1 = x,
                crate::OneOf6::Third(x) => (*ptr).2 = x,
                crate::OneOf6::Fourth(x) => (*ptr).3 = x,
                crate::OneOf6::Fifth(x) => (*ptr).4 = x,
                crate::OneOf6::Sixth(x) => (*ptr).5 = x,
            }
        }
    }
    unsafe { res.assume_init() }
}

/// Polls seven futures concurrently until all are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join7<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
    f4: FutT4,
    f5: FutT5,
    f6: FutT6,
    f7: FutT7,
) -> (
    FutT1::Output,
    FutT2::Output,
    FutT3::Output,
    FutT4::Output,
    FutT5::Output,
    FutT6::Output,
    FutT7::Output,
)
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
    FutT7: Future,
{
    let mut res = std::mem::MaybeUninit::<(
        FutT1::Output,
        FutT2::Output,
        FutT3::Output,
        FutT4::Output,
        FutT5::Output,
        FutT6::Output,
        FutT7::Output,
    )>::uninit();

    pinned_any_of!(stream, f1, f2, f3, f4, f5, f6, f7);

    unsafe {
        let ptr = res.as_mut_ptr();
        while let Some(val) = stream.next().await {
            match val {
                crate::OneOf7::First(x) => (*ptr).0 = x,
                crate::OneOf7::Second(x) => (*ptr).1 = x,
                crate::OneOf7::Third(x) => (*ptr).2 = x,
                crate::OneOf7::Fourth(x) => (*ptr).3 = x,
                crate::OneOf7::Fifth(x) => (*ptr).4 = x,
                crate::OneOf7::Sixth(x) => (*ptr).5 = x,
                crate::OneOf7::Seventh(x) => (*ptr).6 = x,
            }
        }
    }
    unsafe { res.assume_init() }
}

/// Polls eight futures concurrently until all are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join8<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7, FutT8>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
    f4: FutT4,
    f5: FutT5,
    f6: FutT6,
    f7: FutT7,
    f8: FutT8,
) -> (
    FutT1::Output,
    FutT2::Output,
    FutT3::Output,
    FutT4::Output,
    FutT5::Output,
    FutT6::Output,
    FutT7::Output,
    FutT8::Output,
)
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
    FutT7: Future,
    FutT8: Future,
{
    let mut res = std::mem::MaybeUninit::<(
        FutT1::Output,
        FutT2::Output,
        FutT3::Output,
        FutT4::Output,
        FutT5::Output,
        FutT6::Output,
        FutT7::Output,
        FutT8::Output,
    )>::uninit();

    pinned_any_of!(stream, f1, f2, f3, f4, f5, f6, f7, f8);

    unsafe {
        let ptr = res.as_mut_ptr();
        while let Some(val) = stream.next().await {
            match val {
                crate::OneOf8::First(x) => (*ptr).0 = x,
                crate::OneOf8::Second(x) => (*ptr).1 = x,
                crate::OneOf8::Third(x) => (*ptr).2 = x,
                crate::OneOf8::Fourth(x) => (*ptr).3 = x,
                crate::OneOf8::Fifth(x) => (*ptr).4 = x,
                crate::OneOf8::Sixth(x) => (*ptr).5 = x,
                crate::OneOf8::Seventh(x) => (*ptr).6 = x,
                crate::OneOf8::Eighth(x) => (*ptr).7 = x,
            }
        }
    }
    unsafe { res.assume_init() }
}
