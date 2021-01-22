//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use std::future::Future;
use std::marker::PhantomData;
use std::task::{Context, Poll};
use std::pin::Pin;

use crate::reactor::Reactor;
use crate::runtime::Runtime;
use crate::scope::Scope;

pub struct Sender<'runtime, T, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    data: Option<T>,
}

pub struct Receiver<'runtime, T, ReactorT: Reactor> {
    rt: &'runtime Runtime<ReactorT>,
    data: Option<T>,
}

impl<'runtime, T, ReactorT: Reactor> Sender<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>) -> Self {
        Sender {
            rt,
            data: None,
        }
    }

    pub async fn send(value: T) {
        todo!()
    }
}

impl<'runtime, T, ReactorT: Reactor> Receiver<'runtime, T, ReactorT> {
    fn new(rt: &'runtime Runtime<ReactorT>) -> Self {
        Receiver {
            rt,
            data: None,
        }
    }
}

impl<'runtime, T, ReactorT: Reactor> Future for Receiver<'runtime, T, ReactorT> {
    type Output = Result<T, bool>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        todo!()
    }
}

pub fn oneshot<'runtime, T, ReactorT: Reactor>(
    rt: &'runtime Runtime<ReactorT>,
) -> (Sender<'runtime, T, ReactorT>, Receiver<'runtime, T, ReactorT>) {
    (Sender::new(rt) , Receiver::new(rt))
}
