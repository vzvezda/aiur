//  \ O /
//  / * \    aiur: the homeplanet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \
use crate::task::{ITask, Task};
use crate::{Reactor, Runtime};

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Waits concurrently until all futures are completed as tasks.
///
/// Internally it just select the correct `join_tasksN()` function based on the number of arguments
/// supplied. For example the `join_tasks!(rt, fut1, fut2, fut3).await` is the same as
/// [`join_tasks3`]`(rt, fut1, fut2, fut3).await`.
///
/// Please note that unlike join implementation in other crates this one returns future and
/// requires `.await` to start execution.
#[macro_export]
macro_rules! join_tasks {
    ($r:expr, $f1:expr, $f2:expr $(,)?) => {
        $crate::join_tasks2($r, $f1, $f2)
    };
    ($r:expr, $f1:expr, $f2:expr, $f3:expr $(,)?) => {
        $crate::join_tasks3($r, $f1, $f2, $f3)
    };
    ($r:expr, $f1:expr, $f2:expr, $f3:expr, $f4:expr $(,)?) => {
        $crate::join_tasks4($r, $f1, $f2, $f3, $f4)
    };
    ($r:expr, $f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr $(,)?) => {
        $crate::join_tasks5($r, $f1, $f2, $f3, $f4, $f5)
    };
    ($r:expr, $f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr $(,)?) => {
        $crate::join_tasks6($r, $f1, $f2, $f3, $f4, $f5, $f6)
    };
    ($r:expr, $f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr, $f7:expr $(,)?) => {
        $crate::join_tasks7($r, $f1, $f2, $f3, $f4, $f5, $f6, $f7)
    };
    ($r:expr, $f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $f6:expr, $f7:expr, $f8:expr $(,)?) => {
        $crate::join_tasks8($r, $f1, $f2, $f3, $f4, $f5, $f6, $f7, $f8)
    };
}

// This trait helps to reduce the amount of code in this file without writing macros. With it
// we can only have one impl Future for any number of futures.
trait TaskStorage {
    type Output;

    fn assign_parent(&self, ctx: &mut Context<'_>);
    fn poll(&self);
    fn is_completed(&self) -> bool;
    fn take_result(&self) -> Self::Output;
}

// Below are the impls of TaskStorage for 2,3,4,5,6,7,8 future tuples.

// 2 futures
impl<FutT1, FutT2> TaskStorage for (Task<FutT1>, Task<FutT2>)
where
    FutT1: Future,
    FutT2: Future,
{
    type Output = (FutT1::Output, FutT2::Output);

    fn assign_parent(&self, ctx: &mut Context<'_>) {
        if self.0.assign_parent(ctx) {
            self.1.assign_parent(ctx);
        } else {
            // all parents already assigned.
        }
    }

    fn poll(&self) {
        self.0.poll();
        self.1.poll();
    }

    fn is_completed(&self) -> bool {
        self.0.is_completed() && self.1.is_completed()
    }

    fn take_result(&self) -> Self::Output {
        (self.0.take_result(), self.1.take_result())
    }
}

// 3 futures
impl<FutT1, FutT2, FutT3> TaskStorage for (Task<FutT1>, Task<FutT2>, Task<FutT3>)
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
{
    type Output = (FutT1::Output, FutT2::Output, FutT3::Output);

    fn assign_parent(&self, ctx: &mut Context<'_>) {
        if self.0.assign_parent(ctx) {
            self.1.assign_parent(ctx);
            self.2.assign_parent(ctx);
        } else {
            // all parents already assigned.
        }
    }

    fn poll(&self) {
        self.0.poll();
        self.1.poll();
        self.2.poll();
    }

    fn is_completed(&self) -> bool {
        self.0.is_completed() && self.1.is_completed() && self.2.is_completed()
    }

    fn take_result(&self) -> Self::Output {
        (
            self.0.take_result(),
            self.1.take_result(),
            self.2.take_result(),
        )
    }
}

// 4 futures
impl<FutT1, FutT2, FutT3, FutT4> TaskStorage
    for (Task<FutT1>, Task<FutT2>, Task<FutT3>, Task<FutT4>)
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
{
    type Output = (FutT1::Output, FutT2::Output, FutT3::Output, FutT4::Output);

    fn assign_parent(&self, ctx: &mut Context<'_>) {
        if self.0.assign_parent(ctx) {
            self.1.assign_parent(ctx);
            self.2.assign_parent(ctx);
            self.3.assign_parent(ctx);
        } else {
            // all parents already assigned.
        }
    }

    fn poll(&self) {
        self.0.poll();
        self.1.poll();
        self.2.poll();
        self.3.poll();
    }

    fn is_completed(&self) -> bool {
        self.0.is_completed()
            && self.1.is_completed()
            && self.2.is_completed()
            && self.3.is_completed()
    }

    fn take_result(&self) -> Self::Output {
        (
            self.0.take_result(),
            self.1.take_result(),
            self.2.take_result(),
            self.3.take_result(),
        )
    }
}

// 5 futures
impl<FutT1, FutT2, FutT3, FutT4, FutT5> TaskStorage
    for (
        Task<FutT1>,
        Task<FutT2>,
        Task<FutT3>,
        Task<FutT4>,
        Task<FutT5>,
    )
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
{
    type Output = (
        FutT1::Output,
        FutT2::Output,
        FutT3::Output,
        FutT4::Output,
        FutT5::Output,
    );

    fn assign_parent(&self, ctx: &mut Context<'_>) {
        if self.0.assign_parent(ctx) {
            self.1.assign_parent(ctx);
            self.2.assign_parent(ctx);
            self.3.assign_parent(ctx);
            self.4.assign_parent(ctx);
        } else {
            // all parents already assigned.
        }
    }

    fn poll(&self) {
        self.0.poll();
        self.1.poll();
        self.2.poll();
        self.3.poll();
        self.4.poll();
    }

    fn is_completed(&self) -> bool {
        self.0.is_completed()
            && self.1.is_completed()
            && self.2.is_completed()
            && self.3.is_completed()
            && self.4.is_completed()
    }

    fn take_result(&self) -> Self::Output {
        (
            self.0.take_result(),
            self.1.take_result(),
            self.2.take_result(),
            self.3.take_result(),
            self.4.take_result(),
        )
    }
}

// 6 futures
impl<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6> TaskStorage
    for (
        Task<FutT1>,
        Task<FutT2>,
        Task<FutT3>,
        Task<FutT4>,
        Task<FutT5>,
        Task<FutT6>,
    )
where
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
{
    type Output = (
        FutT1::Output,
        FutT2::Output,
        FutT3::Output,
        FutT4::Output,
        FutT5::Output,
        FutT6::Output,
    );

    fn assign_parent(&self, ctx: &mut Context<'_>) {
        if self.0.assign_parent(ctx) {
            self.1.assign_parent(ctx);
            self.2.assign_parent(ctx);
            self.3.assign_parent(ctx);
            self.4.assign_parent(ctx);
            self.5.assign_parent(ctx);
        } else {
            // all parents already assigned.
        }
    }

    fn poll(&self) {
        self.0.poll();
        self.1.poll();
        self.2.poll();
        self.3.poll();
        self.4.poll();
        self.5.poll();
    }

    fn is_completed(&self) -> bool {
        self.0.is_completed()
            && self.1.is_completed()
            && self.2.is_completed()
            && self.3.is_completed()
            && self.4.is_completed()
            && self.5.is_completed()
    }

    fn take_result(&self) -> Self::Output {
        (
            self.0.take_result(),
            self.1.take_result(),
            self.2.take_result(),
            self.3.take_result(),
            self.4.take_result(),
            self.5.take_result(),
        )
    }
}

// 7 futures
impl<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7> TaskStorage
    for (
        Task<FutT1>,
        Task<FutT2>,
        Task<FutT3>,
        Task<FutT4>,
        Task<FutT5>,
        Task<FutT6>,
        Task<FutT7>,
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
    type Output = (
        FutT1::Output,
        FutT2::Output,
        FutT3::Output,
        FutT4::Output,
        FutT5::Output,
        FutT6::Output,
        FutT7::Output,
    );

    fn assign_parent(&self, ctx: &mut Context<'_>) {
        if self.0.assign_parent(ctx) {
            self.1.assign_parent(ctx);
            self.2.assign_parent(ctx);
            self.3.assign_parent(ctx);
            self.4.assign_parent(ctx);
            self.5.assign_parent(ctx);
            self.6.assign_parent(ctx);
        } else {
            // all parents already assigned.
        }
    }

    fn poll(&self) {
        self.0.poll();
        self.1.poll();
        self.2.poll();
        self.3.poll();
        self.4.poll();
        self.5.poll();
        self.6.poll();
    }

    fn is_completed(&self) -> bool {
        self.0.is_completed()
            && self.1.is_completed()
            && self.2.is_completed()
            && self.3.is_completed()
            && self.4.is_completed()
            && self.5.is_completed()
            && self.6.is_completed()
    }

    fn take_result(&self) -> Self::Output {
        (
            self.0.take_result(),
            self.1.take_result(),
            self.2.take_result(),
            self.3.take_result(),
            self.4.take_result(),
            self.5.take_result(),
            self.6.take_result(),
        )
    }
}

// 8 futures
impl<FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7, FutT8> TaskStorage
    for (
        Task<FutT1>,
        Task<FutT2>,
        Task<FutT3>,
        Task<FutT4>,
        Task<FutT5>,
        Task<FutT6>,
        Task<FutT7>,
        Task<FutT8>,
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
    type Output = (
        FutT1::Output,
        FutT2::Output,
        FutT3::Output,
        FutT4::Output,
        FutT5::Output,
        FutT6::Output,
        FutT7::Output,
        FutT8::Output,
    );

    fn assign_parent(&self, ctx: &mut Context<'_>) {
        if self.0.assign_parent(ctx) {
            self.1.assign_parent(ctx);
            self.2.assign_parent(ctx);
            self.3.assign_parent(ctx);
            self.4.assign_parent(ctx);
            self.5.assign_parent(ctx);
            self.6.assign_parent(ctx);
            self.7.assign_parent(ctx);
        } else {
            // all parents already assigned.
        }
    }

    fn poll(&self) {
        self.0.poll();
        self.1.poll();
        self.2.poll();
        self.3.poll();
        self.4.poll();
        self.5.poll();
        self.6.poll();
        self.7.poll();
    }

    fn is_completed(&self) -> bool {
        self.0.is_completed()
            && self.1.is_completed()
            && self.2.is_completed()
            && self.3.is_completed()
            && self.4.is_completed()
            && self.5.is_completed()
            && self.6.is_completed()
            && self.7.is_completed()
    }

    fn take_result(&self) -> Self::Output {
        (
            self.0.take_result(),
            self.1.take_result(),
            self.2.take_result(),
            self.3.take_result(),
            self.4.take_result(),
            self.5.take_result(),
            self.6.take_result(),
            self.7.take_result(),
        )
    }
}

// Leaf future impl for use in join_tasksN() functions.
struct TaskJoin<TaskStorageT: TaskStorage> {
    storage: TaskStorageT,
}

impl<TaskStorageT: TaskStorage> Future for TaskJoin<TaskStorageT> {
    type Output = TaskStorageT::Output;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let this = self.as_ref().get_ref();

        // assign_parent has to be done of first poll. On subsequent polls introduce this
        // one additional boolean check.
        this.storage.assign_parent(ctx);
        this.storage.poll();

        // It is important to verify is_completed() on tasks after all polls because it is
        // possible the poll of the seconds future would make the first future completed because
        // of a nested loop.
        if this.storage.is_completed() {
            Poll::Ready(this.storage.take_result())
        } else {
            Poll::Pending
        }
    }
}

/// Polls two futures concurrently as tasks until both are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join_tasks2<ReactorT, FutT1, FutT2>(
    rt: &Runtime<ReactorT>,
    f1: FutT1,
    f2: FutT2,
) -> (FutT1::Output, FutT2::Output)
where
    ReactorT: Reactor,
    FutT1: Future,
    FutT2: Future,
{
    let awoken_ptr = rt.get_awoken_ptr();

    TaskJoin {
        storage: (Task::new(f1, awoken_ptr), Task::new(f2, awoken_ptr)),
    }
    .await
}

/// Polls three futures concurrently as tasks until both are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join_tasks3<ReactorT, FutT1, FutT2, FutT3>(
    rt: &Runtime<ReactorT>,
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
) -> (FutT1::Output, FutT2::Output, FutT3::Output)
where
    ReactorT: Reactor,
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
{
    let awoken_ptr = rt.get_awoken_ptr();

    TaskJoin {
        storage: (
            Task::new(f1, awoken_ptr),
            Task::new(f2, awoken_ptr),
            Task::new(f3, awoken_ptr),
        ),
    }
    .await
}

/// Polls four futures concurrently as tasks until both are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join_tasks4<ReactorT, FutT1, FutT2, FutT3, FutT4>(
    rt: &Runtime<ReactorT>,
    f1: FutT1,
    f2: FutT2,
    f3: FutT3,
    f4: FutT4,
) -> (FutT1::Output, FutT2::Output, FutT3::Output, FutT4::Output)
where
    ReactorT: Reactor,
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
{
    let awoken_ptr = rt.get_awoken_ptr();

    TaskJoin {
        storage: (
            Task::new(f1, awoken_ptr),
            Task::new(f2, awoken_ptr),
            Task::new(f3, awoken_ptr),
            Task::new(f4, awoken_ptr),
        ),
    }
    .await
}

/// Polls five futures concurrently as tasks until both are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join_tasks5<ReactorT, FutT1, FutT2, FutT3, FutT4, FutT5>(
    rt: &Runtime<ReactorT>,
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
    ReactorT: Reactor,
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
{
    let awoken_ptr = rt.get_awoken_ptr();

    TaskJoin {
        storage: (
            Task::new(f1, awoken_ptr),
            Task::new(f2, awoken_ptr),
            Task::new(f3, awoken_ptr),
            Task::new(f4, awoken_ptr),
            Task::new(f5, awoken_ptr),
        ),
    }
    .await
}

/// Polls six futures concurrently as tasks until both are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join_tasks6<ReactorT, FutT1, FutT2, FutT3, FutT4, FutT5, FutT6>(
    rt: &Runtime<ReactorT>,
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
    ReactorT: Reactor,
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
{
    let awoken_ptr = rt.get_awoken_ptr();

    TaskJoin {
        storage: (
            Task::new(f1, awoken_ptr),
            Task::new(f2, awoken_ptr),
            Task::new(f3, awoken_ptr),
            Task::new(f4, awoken_ptr),
            Task::new(f5, awoken_ptr),
            Task::new(f6, awoken_ptr),
        ),
    }
    .await
}

/// Polls seven futures concurrently as tasks until both are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join_tasks7<ReactorT, FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7>(
    rt: &Runtime<ReactorT>,
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
    ReactorT: Reactor,
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
    FutT7: Future,
{
    let awoken_ptr = rt.get_awoken_ptr();

    TaskJoin {
        storage: (
            Task::new(f1, awoken_ptr),
            Task::new(f2, awoken_ptr),
            Task::new(f3, awoken_ptr),
            Task::new(f4, awoken_ptr),
            Task::new(f5, awoken_ptr),
            Task::new(f6, awoken_ptr),
            Task::new(f7, awoken_ptr),
        ),
    }
    .await
}

/// Polls eight futures concurrently as tasks until both are completed.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub async fn join_tasks8<ReactorT, FutT1, FutT2, FutT3, FutT4, FutT5, FutT6, FutT7, FutT8>(
    rt: &Runtime<ReactorT>,
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
    ReactorT: Reactor,
    FutT1: Future,
    FutT2: Future,
    FutT3: Future,
    FutT4: Future,
    FutT5: Future,
    FutT6: Future,
    FutT7: Future,
    FutT8: Future,
{
    let awoken_ptr = rt.get_awoken_ptr();

    TaskJoin {
        storage: (
            Task::new(f1, awoken_ptr),
            Task::new(f2, awoken_ptr),
            Task::new(f3, awoken_ptr),
            Task::new(f4, awoken_ptr),
            Task::new(f5, awoken_ptr),
            Task::new(f6, awoken_ptr),
            Task::new(f7, awoken_ptr),
            Task::new(f8, awoken_ptr),
        ),
    }
    .await
}
