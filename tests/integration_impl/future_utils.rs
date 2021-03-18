//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \

// this is how I start creating my own version of 'futures' crate...
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

// -----------------------------------------------------------------------------------------
// PollInnerOnceFuture
//
// Future that polls the inner future only once and then signals that it is ready. We need
// this to test cancellation
// -----------------------------------------------------------------------------------------
struct PollInnerOnceFuture<FutureT: Future> {
    inner: FutureT,
}

impl<FutureT: Future> PollInnerOnceFuture<FutureT> {
    fn new(inner: FutureT) -> Self {
        PollInnerOnceFuture { inner }
    }

    fn get_inner_future(self: Pin<&mut Self>) -> Pin<&mut FutureT> {
        //  This is okay because `field` is pinned when `self` is.
        unsafe { self.map_unchecked_mut(|s| &mut s.inner) }
    }
}

impl<FutureT: Future> Future for PollInnerOnceFuture<FutureT> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let _does_not_matter = self.get_inner_future().poll(ctx);
        Poll::Ready(())
    }
}

// -----------------------------------------------------------------------------------------
// Any2Void
//
// -----------------------------------------------------------------------------------------
struct Any2Void<FutureT1: Future, FutureT2: Future> {
    f1: FutureT1,
    f2: FutureT2,
}

impl<FutureT1: Future, FutureT2: Future> Any2Void<FutureT1, FutureT2> {
    fn new(f1: FutureT1, f2: FutureT2) -> Self {
        Any2Void { f1, f2 }
    }

    fn get_f1(&mut self) -> Pin<&mut FutureT1> {
        //  This is okay because `field` is pinned when `self` is.
        unsafe { Pin::new_unchecked(&mut self.f1) }
    }

    fn get_f2(&mut self) -> Pin<&mut FutureT2> {
        //  This is okay because `field` is pinned when `self` is.
        unsafe { Pin::new_unchecked(&mut self.f2) }
    }
}

impl<FutureT1: Future, FutureT2: Future> Future for Any2Void<FutureT1, FutureT2> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };

        match this.get_f1().poll(ctx) {
            Poll::Pending => match this.get_f2().poll(ctx) {
                Poll::Pending => return Poll::Pending,
                _ => return Poll::Ready(()),
            },
            _ => return Poll::Ready(()),
        }
    }
}

// -----------------------------------------------------------------------------------------
// Public accessors to Future util structs
//
// -----------------------------------------------------------------------------------------
pub async fn poll_inner_once<FutureT: Future>(inner: FutureT) {
    PollInnerOnceFuture::new(inner).await;
}

pub async fn any2void<FutureT1: Future, FutureT2: Future>(f1: FutureT1, f2: FutureT2) {
    Any2Void::new(f1, f2).await;
}
