//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Used as result of [AnyOfN::next()] for two futures.
pub enum OneOf2<T1, T2> {
    First(T1),
    Second(T2),
}

/// Used as result of [AnyOfN::next()] for three futures.
pub enum OneOf3<T1, T2, T3> {
    First(T1),
    Second(T2),
    Third(T3),
}

/// Used as result of [AnyOfN::next()] for four futures.
pub enum OneOf4<T1, T2, T3, T4> {
    First(T1),
    Second(T2),
    Third(T3),
    Fourth(T4),
}

/// Used as result of [AnyOfN::next()] for five futures.
pub enum OneOf5<T1, T2, T3, T4, T5> {
    First(T1),
    Second(T2),
    Third(T3),
    Fourth(T4),
    Fifth(T5),
}

/// Used as result of [AnyOfN::next()] for six futures.
pub enum OneOf6<T1, T2, T3, T4, T5, T6> {
    First(T1),
    Second(T2),
    Third(T3),
    Fourth(T4),
    Fifth(T5),
    Sixth(T6),
}

/// Used as result of [AnyOfN::next()] for seven futures.
pub enum OneOf7<T1, T2, T3, T4, T5, T6, T7> {
    First(T1),
    Second(T2),
    Third(T3),
    Fourth(T4),
    Fifth(T5),
    Sixth(T6),
    Seventh(T7),
}

/// Used as result of [AnyOfN::next()] for eight futures.
pub enum OneOf8<T1, T2, T3, T4, T5, T6, T7, T8> {
    First(T1),
    Second(T2),
    Third(T3),
    Fourth(T4),
    Fifth(T5),
    Sixth(T6),
    Seventh(T7),
    Eighth(T8),
}

/// Stream to poll several futures concurrently.
///
/// To create it use one of any_ofX() function or the macros [make_any_of!()](crate::make_any_of!) 
/// or [pinned_any_of!()](crate::pinned_any_of!).
/// Once created invoking `[AnyOfN::next()].await` polls the futures and returns the result of the
/// first completed future. The consecutive call to `[AnyOfN::next()].await` returns result of the
/// first completed of remaining futures, etc.
///
/// If you wait all futures to complete (until `[AnyOfN::next()].await` returns `None`) you
/// will have something like join!(). If you wait only one future you can have something like
/// select!().
///
/// When constructed the AnyOfN stream takes ownership over the futures. Futures are dropped
/// only when AnyOfN is dropoped.
pub struct AnyOfN<TupleT> {
    fs: TupleT, // (Fut1, Fut2, .. FutN)
    active: u8, // bitfield for completed futures, u8 is ok for now
}

impl<TupleT> AnyOfN<TupleT> {
    // If all futures in this AnyOfN has been completed
    fn is_done(&self) -> bool {
        self.active == 0
    }

    // Shared code to poll the future from self.fs
    fn poll_n<FutT: Future>(
        ctx: &mut Context,
        n: u8,
        fut: &mut FutT,
        active: &mut u8,
    ) -> Option<FutT::Output> {
        let active_flag: u8 = 1 << n;

        if *active & active_flag != 0 {
            // Unsafe is ok: the AnyOfN has to be Pin<&mut self> for next().
            let pinned = unsafe { Pin::new_unchecked(fut) };
            match pinned.poll(ctx) {
                Poll::Pending => None, // Future is still pending
                Poll::Ready(result) => {
                    // Done, clean the bit and return future result
                    *active &= !active_flag;
                    Some(result)
                }
            }
        } else {
            None // this future is no longer active
        }
    }
}

impl<FutT1, FutT2> AnyOfN<(FutT1, FutT2)>
where
    FutT1: Future,
    FutT2: Future,
{
    fn poll_f1(&mut self, ctx: &mut Context) -> Option<FutT1::Output> {
        Self::poll_n(ctx, 0, &mut self.fs.0, &mut self.active)
    }
    fn poll_f2(&mut self, ctx: &mut Context) -> Option<FutT2::Output> {
        Self::poll_n(ctx, 1, &mut self.fs.1, &mut self.active)
    }
}

impl<FutT1, FutT2, FutT3> AnyOfN<(FutT1, FutT2, FutT3)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
{
    fn poll_f1(&mut self, ctx: &mut Context) -> Option<FutT1::Output> {
        Self::poll_n(ctx, 0, &mut self.fs.0, &mut self.active)
    }
    fn poll_f2(&mut self, ctx: &mut Context) -> Option<FutT2::Output> {
        Self::poll_n(ctx, 1, &mut self.fs.1, &mut self.active)
    }
    fn poll_f3(&mut self, ctx: &mut Context) -> Option<FutT3::Output> {
        Self::poll_n(ctx, 2, &mut self.fs.2, &mut self.active)
    }
}

impl<FutT1, FutT2, FutT3, FutT4> AnyOfN<(FutT1, FutT2, FutT3, FutT4)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
{
    fn poll_f1(&mut self, ctx: &mut Context) -> Option<FutT1::Output> {
        Self::poll_n(ctx, 0, &mut self.fs.0, &mut self.active)
    }
    fn poll_f2(&mut self, ctx: &mut Context) -> Option<FutT2::Output> {
        Self::poll_n(ctx, 1, &mut self.fs.1, &mut self.active)
    }
    fn poll_f3(&mut self, ctx: &mut Context) -> Option<FutT3::Output> {
        Self::poll_n(ctx, 2, &mut self.fs.2, &mut self.active)
    }
    fn poll_f4(&mut self, ctx: &mut Context) -> Option<FutT4::Output> {
        Self::poll_n(ctx, 3, &mut self.fs.3, &mut self.active)
    }
}

impl<FutT1, FutT2, FutT3, FutT4, FutT5> AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
{
    fn poll_f1(&mut self, ctx: &mut Context) -> Option<FutT1::Output> {
        Self::poll_n(ctx, 0, &mut self.fs.0, &mut self.active)
    }
    fn poll_f2(&mut self, ctx: &mut Context) -> Option<FutT2::Output> {
        Self::poll_n(ctx, 1, &mut self.fs.1, &mut self.active)
    }
    fn poll_f3(&mut self, ctx: &mut Context) -> Option<FutT3::Output> {
        Self::poll_n(ctx, 2, &mut self.fs.2, &mut self.active)
    }
    fn poll_f4(&mut self, ctx: &mut Context) -> Option<FutT4::Output> {
        Self::poll_n(ctx, 3, &mut self.fs.3, &mut self.active)
    }
    fn poll_f5(&mut self, ctx: &mut Context) -> Option<FutT5::Output> {
        Self::poll_n(ctx, 4, &mut self.fs.4, &mut self.active)
    }
}

impl<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6> AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5, FutT6)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
{
    fn poll_f1(&mut self, ctx: &mut Context) -> Option<FutT1::Output> {
        Self::poll_n(ctx, 0, &mut self.fs.0, &mut self.active)
    }
    fn poll_f2(&mut self, ctx: &mut Context) -> Option<FutT2::Output> {
        Self::poll_n(ctx, 1, &mut self.fs.1, &mut self.active)
    }
    fn poll_f3(&mut self, ctx: &mut Context) -> Option<FutT3::Output> {
        Self::poll_n(ctx, 2, &mut self.fs.2, &mut self.active)
    }
    fn poll_f4(&mut self, ctx: &mut Context) -> Option<FutT4::Output> {
        Self::poll_n(ctx, 3, &mut self.fs.3, &mut self.active)
    }
    fn poll_f5(&mut self, ctx: &mut Context) -> Option<FutT5::Output> {
        Self::poll_n(ctx, 4, &mut self.fs.4, &mut self.active)
    }
    fn poll_f6(&mut self, ctx: &mut Context) -> Option<FutT6::Output> {
        Self::poll_n(ctx, 5, &mut self.fs.5, &mut self.active)
    }
}

impl<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7>
    AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
    FutT7: Future,
{
    fn poll_f1(&mut self, ctx: &mut Context) -> Option<FutT1::Output> {
        Self::poll_n(ctx, 0, &mut self.fs.0, &mut self.active)
    }
    fn poll_f2(&mut self, ctx: &mut Context) -> Option<FutT2::Output> {
        Self::poll_n(ctx, 1, &mut self.fs.1, &mut self.active)
    }
    fn poll_f3(&mut self, ctx: &mut Context) -> Option<FutT3::Output> {
        Self::poll_n(ctx, 2, &mut self.fs.2, &mut self.active)
    }
    fn poll_f4(&mut self, ctx: &mut Context) -> Option<FutT4::Output> {
        Self::poll_n(ctx, 3, &mut self.fs.3, &mut self.active)
    }
    fn poll_f5(&mut self, ctx: &mut Context) -> Option<FutT5::Output> {
        Self::poll_n(ctx, 4, &mut self.fs.4, &mut self.active)
    }
    fn poll_f6(&mut self, ctx: &mut Context) -> Option<FutT6::Output> {
        Self::poll_n(ctx, 5, &mut self.fs.5, &mut self.active)
    }
    fn poll_f7(&mut self, ctx: &mut Context) -> Option<FutT7::Output> {
        Self::poll_n(ctx, 6, &mut self.fs.6, &mut self.active)
    }
}

impl<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7, FutT8>
    AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7, FutT8)>
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
    fn poll_f1(&mut self, ctx: &mut Context) -> Option<FutT1::Output> {
        Self::poll_n(ctx, 0, &mut self.fs.0, &mut self.active)
    }
    fn poll_f2(&mut self, ctx: &mut Context) -> Option<FutT2::Output> {
        Self::poll_n(ctx, 1, &mut self.fs.1, &mut self.active)
    }
    fn poll_f3(&mut self, ctx: &mut Context) -> Option<FutT3::Output> {
        Self::poll_n(ctx, 2, &mut self.fs.2, &mut self.active)
    }
    fn poll_f4(&mut self, ctx: &mut Context) -> Option<FutT4::Output> {
        Self::poll_n(ctx, 3, &mut self.fs.3, &mut self.active)
    }
    fn poll_f5(&mut self, ctx: &mut Context) -> Option<FutT5::Output> {
        Self::poll_n(ctx, 4, &mut self.fs.4, &mut self.active)
    }
    fn poll_f6(&mut self, ctx: &mut Context) -> Option<FutT6::Output> {
        Self::poll_n(ctx, 5, &mut self.fs.5, &mut self.active)
    }
    fn poll_f7(&mut self, ctx: &mut Context) -> Option<FutT7::Output> {
        Self::poll_n(ctx, 6, &mut self.fs.6, &mut self.active)
    }
    fn poll_f8(&mut self, ctx: &mut Context) -> Option<FutT8::Output> {
        Self::poll_n(ctx, 7, &mut self.fs.7, &mut self.active)
    }
}

pub struct NextOfN<'any, TupleT> {
    any: &'any mut AnyOfN<TupleT>,
}

impl<'any, FutT1, FutT2> Future for NextOfN<'any, (FutT1, FutT2)>
where
    FutT1: Future,
    FutT2: Future,
{
    type Output = Option<OneOf2<FutT1::Output, FutT2::Output>>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if this.any.is_done() {
            Poll::Ready(None)
        } else if let Some(x) = this.any.poll_f1(ctx) {
            Poll::Ready(Some(OneOf2::First(x)))
        } else if let Some(x) = this.any.poll_f2(ctx) {
            Poll::Ready(Some(OneOf2::Second(x)))
        } else {
            Poll::Pending
        }
    }
}

impl<'any, FutT1, FutT2, FutT3> Future for NextOfN<'any, (FutT1, FutT2, FutT3)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
{
    type Output = Option<OneOf3<FutT1::Output, FutT2::Output, FutT3::Output>>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if this.any.is_done() {
            Poll::Ready(None)
        } else if let Some(x) = this.any.poll_f1(ctx) {
            Poll::Ready(Some(OneOf3::First(x)))
        } else if let Some(x) = this.any.poll_f2(ctx) {
            Poll::Ready(Some(OneOf3::Second(x)))
        } else if let Some(x) = this.any.poll_f3(ctx) {
            Poll::Ready(Some(OneOf3::Third(x)))
        } else {
            Poll::Pending
        }
    }
}

impl<'any, FutT1, FutT2, FutT3, FutT4> Future for NextOfN<'any, (FutT1, FutT2, FutT3, FutT4)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
{
    type Output = Option<OneOf4<FutT1::Output, FutT2::Output, FutT3::Output, FutT4::Output>>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if this.any.is_done() {
            Poll::Ready(None)
        } else if let Some(x) = this.any.poll_f1(ctx) {
            Poll::Ready(Some(OneOf4::First(x)))
        } else if let Some(x) = this.any.poll_f2(ctx) {
            Poll::Ready(Some(OneOf4::Second(x)))
        } else if let Some(x) = this.any.poll_f3(ctx) {
            Poll::Ready(Some(OneOf4::Third(x)))
        } else if let Some(x) = this.any.poll_f4(ctx) {
            Poll::Ready(Some(OneOf4::Fourth(x)))
        } else {
            Poll::Pending
        }
    }
}

impl<'any, FutT1, FutT2, FutT3, FutT4, FutT5> Future
    for NextOfN<'any, (FutT1, FutT2, FutT3, FutT4, FutT5)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
{
    type Output =
        Option<OneOf5<FutT1::Output, FutT2::Output, FutT3::Output, FutT4::Output, FutT5::Output>>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if this.any.is_done() {
            Poll::Ready(None)
        } else if let Some(x) = this.any.poll_f1(ctx) {
            Poll::Ready(Some(OneOf5::First(x)))
        } else if let Some(x) = this.any.poll_f2(ctx) {
            Poll::Ready(Some(OneOf5::Second(x)))
        } else if let Some(x) = this.any.poll_f3(ctx) {
            Poll::Ready(Some(OneOf5::Third(x)))
        } else if let Some(x) = this.any.poll_f4(ctx) {
            Poll::Ready(Some(OneOf5::Fourth(x)))
        } else if let Some(x) = this.any.poll_f5(ctx) {
            Poll::Ready(Some(OneOf5::Fifth(x)))
        } else {
            Poll::Pending
        }
    }
}

impl<'any, FutT1, FutT2, FutT3, FutT4, FutT5, FutT6> Future
    for NextOfN<'any, (FutT1, FutT2, FutT3, FutT4, FutT5, FutT6)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
{
    type Output = Option<
        OneOf6<
            FutT1::Output,
            FutT2::Output,
            FutT3::Output,
            FutT4::Output,
            FutT5::Output,
            FutT6::Output,
        >,
    >;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if this.any.is_done() {
            Poll::Ready(None)
        } else if let Some(x) = this.any.poll_f1(ctx) {
            Poll::Ready(Some(OneOf6::First(x)))
        } else if let Some(x) = this.any.poll_f2(ctx) {
            Poll::Ready(Some(OneOf6::Second(x)))
        } else if let Some(x) = this.any.poll_f3(ctx) {
            Poll::Ready(Some(OneOf6::Third(x)))
        } else if let Some(x) = this.any.poll_f4(ctx) {
            Poll::Ready(Some(OneOf6::Fourth(x)))
        } else if let Some(x) = this.any.poll_f5(ctx) {
            Poll::Ready(Some(OneOf6::Fifth(x)))
        } else if let Some(x) = this.any.poll_f6(ctx) {
            Poll::Ready(Some(OneOf6::Sixth(x)))
        } else {
            Poll::Pending
        }
    }
}

impl<'any, FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7> Future
    for NextOfN<'any, (FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
    FutT7: Future,
{
    type Output = Option<
        OneOf7<
            FutT1::Output,
            FutT2::Output,
            FutT3::Output,
            FutT4::Output,
            FutT5::Output,
            FutT6::Output,
            FutT7::Output,
        >,
    >;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if this.any.is_done() {
            Poll::Ready(None)
        } else if let Some(x) = this.any.poll_f1(ctx) {
            Poll::Ready(Some(OneOf7::First(x)))
        } else if let Some(x) = this.any.poll_f2(ctx) {
            Poll::Ready(Some(OneOf7::Second(x)))
        } else if let Some(x) = this.any.poll_f3(ctx) {
            Poll::Ready(Some(OneOf7::Third(x)))
        } else if let Some(x) = this.any.poll_f4(ctx) {
            Poll::Ready(Some(OneOf7::Fourth(x)))
        } else if let Some(x) = this.any.poll_f5(ctx) {
            Poll::Ready(Some(OneOf7::Fifth(x)))
        } else if let Some(x) = this.any.poll_f6(ctx) {
            Poll::Ready(Some(OneOf7::Sixth(x)))
        } else if let Some(x) = this.any.poll_f7(ctx) {
            Poll::Ready(Some(OneOf7::Seventh(x)))
        } else {
            Poll::Pending
        }
    }
}

impl<'any, FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7, FutT8> Future
    for NextOfN<'any, (FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7, FutT8)>
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
    type Output = Option<
        OneOf8<
            FutT1::Output,
            FutT2::Output,
            FutT3::Output,
            FutT4::Output,
            FutT5::Output,
            FutT6::Output,
            FutT7::Output,
            FutT8::Output,
        >,
    >;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        if this.any.is_done() {
            Poll::Ready(None)
        } else if let Some(x) = this.any.poll_f1(ctx) {
            Poll::Ready(Some(OneOf8::First(x)))
        } else if let Some(x) = this.any.poll_f2(ctx) {
            Poll::Ready(Some(OneOf8::Second(x)))
        } else if let Some(x) = this.any.poll_f3(ctx) {
            Poll::Ready(Some(OneOf8::Third(x)))
        } else if let Some(x) = this.any.poll_f4(ctx) {
            Poll::Ready(Some(OneOf8::Fourth(x)))
        } else if let Some(x) = this.any.poll_f5(ctx) {
            Poll::Ready(Some(OneOf8::Fifth(x)))
        } else if let Some(x) = this.any.poll_f6(ctx) {
            Poll::Ready(Some(OneOf8::Sixth(x)))
        } else if let Some(x) = this.any.poll_f7(ctx) {
            Poll::Ready(Some(OneOf8::Seventh(x)))
        } else if let Some(x) = this.any.poll_f8(ctx) {
            Poll::Ready(Some(OneOf8::Eighth(x)))
        } else {
            Poll::Pending
        }
    }
}

/// Stream to run two futures concurrently.
impl<FutT1, FutT2> AnyOfN<(FutT1, FutT2)>
where
    FutT1: Future,
    FutT2: Future,
{
    /// Returns the result of the first completed future or None if all futures of the stream
    /// has been completed.
    pub async fn next(self: &mut Pin<&mut Self>) -> Option<OneOf2<FutT1::Output, FutT2::Output>> {
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        (NextOfN { any: this }).await
    }
}

/// Stream to run three futures concurrently.
impl<FutT1, FutT2, FutT3> AnyOfN<(FutT1, FutT2, FutT3)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
{
    /// Returns the result of the first completed future or None if all futures of the stream
    /// has been completed.
    pub async fn next(
        self: &mut Pin<&mut Self>,
    ) -> Option<OneOf3<FutT1::Output, FutT2::Output, FutT3::Output>> {
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        (NextOfN { any: this }).await
    }
}

/// Stream to run four futures concurrently.
impl<FutT1, FutT2, FutT3, FutT4> AnyOfN<(FutT1, FutT2, FutT3, FutT4)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
{
    /// Returns the result of the first completed future or None if all futures of the stream
    /// has been completed.
    pub async fn next(
        self: &mut Pin<&mut Self>,
    ) -> Option<OneOf4<FutT1::Output, FutT2::Output, FutT3::Output, FutT4::Output>> {
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        (NextOfN { any: this }).await
    }
}

/// Stream to run five futures concurrently.
impl<FutT1, FutT2, FutT3, FutT4, FutT5> AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
{
    /// Returns the result of the first completed future or None if all futures of the stream
    /// has been completed.
    pub async fn next(
        self: &mut Pin<&mut Self>,
    ) -> Option<OneOf5<FutT1::Output, FutT2::Output, FutT3::Output, FutT4::Output, FutT5::Output>>
    {
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        (NextOfN { any: this }).await
    }
}

/// Stream to run six futures concurrently.
impl<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6> AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5, FutT6)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
{
    /// Returns the result of the first completed future or None if all futures of the stream
    /// has been completed.
    pub async fn next(
        self: &mut Pin<&mut Self>,
    ) -> Option<
        OneOf6<
            FutT1::Output,
            FutT2::Output,
            FutT3::Output,
            FutT4::Output,
            FutT5::Output,
            FutT6::Output,
        >,
    > {
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        (NextOfN { any: this }).await
    }
}

/// Stream to run seven futures concurrently.
impl<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7>
    AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
    FutT7: Future,
{
    /// Returns the result of the first completed future or None if all futures of the stream
    /// has been completed.
    pub async fn next(
        self: &mut Pin<&mut Self>,
    ) -> Option<
        OneOf7<
            FutT1::Output,
            FutT2::Output,
            FutT3::Output,
            FutT4::Output,
            FutT5::Output,
            FutT6::Output,
            FutT7::Output,
        >,
    > {
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        (NextOfN { any: this }).await
    }
}

/// Stream to run eight futures concurrently.
impl<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7, FutT8>
    AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7, FutT8)>
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
    /// Returns the result of the first completed future or None if all futures of the stream
    /// has been completed.
    pub async fn next(
        self: &mut Pin<&mut Self>,
    ) -> Option<
        OneOf8<
            FutT1::Output,
            FutT2::Output,
            FutT3::Output,
            FutT4::Output,
            FutT5::Output,
            FutT6::Output,
            FutT7::Output,
            FutT8::Output,
        >,
    > {
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        (NextOfN { any: this }).await
    }
}

/// Creates the [AnyOfN] stream to poll two futures.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub fn any_of2<FutT1, FutT2>(f1: FutT1, f2: FutT2) -> AnyOfN<(FutT1, FutT2)>
where
    FutT1: Future,
    FutT2: Future,
{
    AnyOfN {
        fs: (f1, f2),
        active: 0b0011,
    }
}

/// Creates the [AnyOfN] stream to poll three futures.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub fn any_of3<FutT1, FutT2, FutT3>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
) -> AnyOfN<(FutT1, FutT2, FutT3)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
{
    AnyOfN {
        fs: (f1, f2, f3),
        active: 0b0111,
    }
}

/// Creates the [AnyOfN] stream to poll four futures.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub fn any_of4<FutT1, FutT2, FutT3, FutT4>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
    f4: FutT4,
) -> AnyOfN<(FutT1, FutT2, FutT3, FutT4)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
{
    AnyOfN {
        fs: (f1, f2, f3, f4),
        active: 0b1111,
    }
}

/// Creates the [AnyOfN] stream to poll five futures.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub fn any_of5<FutT1, FutT2, FutT3, FutT4, FutT5>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
    f4: FutT4,
    f5: FutT5,
) -> AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
{
    AnyOfN {
        fs: (f1, f2, f3, f4, f5),
        active: 0b0001_1111,
    }
}

/// Creates the [AnyOfN] stream to poll six futures.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub fn any_of6<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
    f4: FutT4,
    f5: FutT5,
    f6: FutT6,
) -> AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5, FutT6)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
{
    AnyOfN {
        fs: (f1, f2, f3, f4, f5, f6),
        active: 0b0011_1111,
    }
}

/// Creates the [AnyOfN] stream to poll seven futures.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub fn any_of7<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
    f4: FutT4,
    f5: FutT5,
    f6: FutT6,
    f7: FutT7,
) -> AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7)>
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
    FutT7: Future,
{
    AnyOfN {
        fs: (f1, f2, f3, f4, f5, f6, f7),
        active: 0b0111_1111,
    }
}

/// Creates the [AnyOfN] stream to poll eigth futures.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub fn any_of8<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7, FutT8>(
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
    f4: FutT4,
    f5: FutT5,
    f6: FutT6,
    f7: FutT7,
    f8: FutT8,
) -> AnyOfN<(FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7, FutT8)>
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
    AnyOfN {
        fs: (f1, f2, f3, f4, f5, f6, f7, f8),
        active: 0b1111_1111,
    }
}

/// Creates [AnyOfN] stream from supplied futures.
///
/// Internally it just select the correct any_ofN() function based on the number of agruments
/// supplied. For example the `make_any_of!(fut1, fut2, fut3)` is the same as
/// [`any_of3`]`(fut1, fut2, fut3)`.
#[macro_export]
macro_rules! make_any_of {
    ($f1:expr, $f2:expr $(,)?) => {
        $crate::any_of2($f1, $f2)
    };
    ($f1:expr, $f2:expr, $f3:expr $(,)?) => {
        $crate::any_of3($f1, $f2, $f3)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr $(,)?) => {
        $crate::any_of4($f1, $f2, $f3, $f4)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr $(,)?) => {
        $crate::any_of5($f1, $f2, $f3, $f4, $f5)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr $(,)?) => {
        $crate::any_of6($f1, $f2, $f3, $f4, $f5, $f6)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr, $f7:expr $(,)?) => {
        $crate::any_of7($f1, $f2, $f3, $f4, $f5, $f6, $f7)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr, $f7:expr, $f8:expr $(,)?) => {
        $crate::any_of8($f1, $f2, $f3, $f4, $f5, $f6, $f7, $f8)
    };
}

/// Makes a pinned version of [AnyOfN] stream.
///
/// It accept a streams variable name as first argument and up to 8 futures. For example, the
/// `pinned_any_of!(stream, fut1, fut2)` is expanded to something like:
///
/// ```
/// # use aiur::make_any_of;
/// # use aiur::pin_local;
/// # use aiur::OneOf2;
/// # async fn test_fn() {}
/// # let fut1 = test_fn();
/// # let fut2 = test_fn();
///
/// let stream = make_any_of!(fut1, fut2);
/// pin_local!(stream);
/// ```
///
/// // So you now have `stream` name pinned and you can wait for any of two futures to complete:
/// ```ignore
/// match stream.next().await.unwrap() {
///    OneOf2::First(_) => println!("fut1 made it"),
///    OneOf2::Second(_) => println!("fut2 made it"),
/// }
/// ```
#[macro_export]
macro_rules! pinned_any_of {
    ($var:ident, $f1:expr, $f2:expr $(,)?) => {
        let $var = $crate::any_of2($f1, $f2);
        $crate::pin_local!($var);
    };
    ($var:ident, $f1:expr, $f2:expr, $f3:expr $(,)?) => {
        let $var = $crate::any_of3($f1, $f2, $f3);
        $crate::pin_local!($var);
    };
    ($var:ident, $f1:expr, $f2:expr, $f3:expr, $f4:expr $(,)?) => {
        let $var = $crate::any_of4($f1, $f2, $f3, $f4);
        $crate::pin_local!($var);
    };
    ($var:ident, $f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr $(,)?) => {
        let $var = $crate::any_of5($f1, $f2, $f3, $f4, $f5);
        $crate::pin_local!($var);
    };
    ($var:ident, $f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr $(,)?) => {
        let $var = $crate::any_of6($f1, $f2, $f3, $f4, $f5, $f6);
        $crate::pin_local!($var);
    };
    ($var:ident, $f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr, $f7:expr $(,)?) => {
        let $var = $crate::any_of7($f1, $f2, $f3, $f4, $f5, $f6, $f7);
        $crate::pin_local!($var);
    };
    ($var:ident, $f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr, $f7:expr, $f8:expr $(,)?) => {
        let $var = $crate::any_of8($f1, $f2, $f3, $f4, $f5, $f6, $f7, $f8);
        $crate::pin_local!($var);
    };
}
