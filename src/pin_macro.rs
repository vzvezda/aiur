//  \ O /
//  / * \    aiur: the home planet for the famous executors
// |' | '|   (c) 2020 - present, Vladimir Zvezda
//   / \

/// Pins the stack variable.
///
/// Macro makes the stack variable provided as argument to be the type of `std::pin::Pin<&mut T>`.
/// ```
/// # use core::pin::Pin;
/// # use aiur::pin_local;
/// # struct Context { }
/// # let ctx = Context {};
/// # struct MyFuture { };
/// # impl MyFuture {
/// #   fn poll(self: Pin<&mut Self>, ctx: Context) -> u32 { 42 }
/// # }
/// // MyFuture implements std::future::Future
/// let fut = MyFuture { /*..*/ };
/// // let _ = fut.poll(ctx); cannot do this as poll requires pinned value
/// pin_local!(fut);
/// // Now we can invoke poll because the type of `fut` now is `std::pin::Pin<&mut MyFuture>`
/// let _ = fut.poll(ctx);
/// ```
///
/// Internally it just shadows the original variable with pinned pointer:
/// ```
/// # let mut var_name = 42;
/// let mut var_name = unsafe { core::pin::Pin::new_unchecked(&mut var_name) };
/// ```
/// As original value is shadowed, there is no both pin and unpin references of the same data
/// exist which justifies the usage of unsafe is ok.
///
/// The similar macro exists in other crates, e.g.  `pin_mut!()` in pin-utils or `pin!()` in tokio.
#[macro_export]
macro_rules! pin_local {
    ($var:ident) => {
        // Verify the ownership
        let mut $var = $var;
        // Use the same name for pinned reference so there is no pin and unpin version of the
        // same data simultaneously exists.
        #[allow(unused_mut)]
        let mut $var = unsafe { core::pin::Pin::new_unchecked(&mut $var) };
    };
}
