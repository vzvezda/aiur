//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::pinned_any_of;

///////////////////////////////////////
///
///

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

struct Join4<FutureT1, FutureT2, FutureT3, FutureT4>
where
    FutureT1: Future,
    FutureT2: Future,
    FutureT3: Future,
    FutureT4: Future,
{
    f1: FutureT1,
    f2: FutureT2,
    f3: FutureT3,
    f4: FutureT4,

    r1: Option<FutureT1::Output>,
    r2: Option<FutureT2::Output>,
    r3: Option<FutureT3::Output>,
    r4: Option<FutureT4::Output>,
}

impl<FutureT1, FutureT2, FutureT3, FutureT4> Join4<FutureT1, FutureT2, FutureT3, FutureT4>
where
    FutureT1: Future,
    FutureT2: Future,
    FutureT3: Future,
    FutureT4: Future,
{
    //  unsafe in get is okay because `field` is pinned when `self` is.
    fn get_f1(&mut self) -> Pin<&mut FutureT1> {
        unsafe { Pin::new_unchecked(&mut self.f1) }
    }
    fn get_f2(&mut self) -> Pin<&mut FutureT2> {
        unsafe { Pin::new_unchecked(&mut self.f2) }
    }
    fn get_f3(&mut self) -> Pin<&mut FutureT3> {
        unsafe { Pin::new_unchecked(&mut self.f3) }
    }
    fn get_f4(&mut self) -> Pin<&mut FutureT4> {
        unsafe { Pin::new_unchecked(&mut self.f4) }
    }
}

impl<FutureT1, FutureT2, FutureT3, FutureT4> Future
    for Join4<FutureT1, FutureT2, FutureT3, FutureT4>
where
    FutureT1: Future,
    FutureT2: Future,
    FutureT3: Future,
    FutureT4: Future,
{
    type Output = (
        FutureT1::Output,
        FutureT2::Output,
        FutureT3::Output,
        FutureT4::Output,
    );

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if this.r1.is_none() {
            match this.get_f1().poll(ctx) {
                Poll::Pending => (),
                Poll::Ready(res1) => this.r1 = Some(res1),
            }
        }

        if this.r2.is_none() {
            match this.get_f2().poll(ctx) {
                Poll::Pending => (),
                Poll::Ready(res2) => this.r2 = Some(res2),
            }
        }

        if this.r3.is_none() {
            match this.get_f3().poll(ctx) {
                Poll::Pending => (),
                Poll::Ready(res3) => this.r3 = Some(res3),
            }
        }

        if this.r4.is_none() {
            match this.get_f4().poll(ctx) {
                Poll::Pending => (),
                Poll::Ready(res4) => this.r4 = Some(res4),
            }
        }

        if this.r1.is_none() || this.r2.is_none() || this.r3.is_none() || this.r4.is_none() {
            Poll::Pending
        } else {
            Poll::Ready((
                this.r1.take().unwrap(),
                this.r2.take().unwrap(),
                this.r3.take().unwrap(),
                this.r4.take().unwrap(),
            ))
        }
    }
}

#[must_use = "futures do nothing unless you `.await` or poll them"]
pub fn join4<FutureT1, FutureT2, FutureT3, FutureT4>(
    f1: FutureT1,
    f2: FutureT2,
    f3: FutureT3,
    f4: FutureT4,
) -> impl Future<
    Output = (
        FutureT1::Output,
        FutureT2::Output,
        FutureT3::Output,
        FutureT4::Output,
    ),
>
where
    FutureT1: Future,
    FutureT2: Future,
    FutureT3: Future,
    FutureT4: Future,
{
    Join4 {
        f1,
        f2,
        f3,
        f4,
        r1: None,
        r2: None,
        r3: None,
        r4: None,
    }
}
