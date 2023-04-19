//! SOD: Service-Oriented Design
//!
//! This crate provides `Service`, `MutService`, and `AsyncService` traits and associated utilities to facilitiate [service-oriented design](https://en.wikipedia.org/wiki/Service-orientation_design_principles).
//! These traits and tools in this library provide concrete guidelines to help make a service-oriented design successful.
//!
//! In the context of this crate, a service is simply a trait that accepts an input and produces a result.
//! Traits can be composed or chained together using the `ServiceChain` found in this crate.
//!
//! This crate in and of itself does not provide mechanisms to expose services on a network or facilitiate service discovery.
//! Those implementation details are to be provided in `sod-*` crates, which will often simply encapsulate other open source libraries to expose them as services.
//! Instead, this crate provides the core mechanisms to define services in a way that helps guarantee they will be interoperable with one another at a library level.

use async_trait::async_trait;
use std::{
    cell::RefCell,
    error::Error,
    fmt::{Debug, Display},
    marker::PhantomData,
    sync::{Arc, Mutex},
};

pub mod idle;

/// A sync service trait
///
/// Accepts `&self` and an input, which produces a `Result<Self::Output, Self::Error>`
pub trait Service<I> {
    type Output;
    type Error;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error>;
}

/// A mut service trait
///
/// Accepts `&mut self` and an input, which produces a `Result<Self::Output, Self::Error>`
pub trait MutService<I> {
    type Output;
    type Error;
    fn process(&mut self, input: I) -> Result<Self::Output, Self::Error>;
}

/// An async service trait
///
/// Uses the [async_trait](https://docs.rs/async-trait/latest/async_trait/) to accept `&self` and an input asynchronously, which produces a `Result<Self::Output, Self::Error>`
#[async_trait]
pub trait AsyncService<I> {
    type Output;
    type Error;
    async fn process(&self, input: I) -> Result<Self::Output, Self::Error>;
}

/// A [`MutService`] that encapsulates an underlying [`Service`], exposing it as `mut`.
/// Any `Service` should be able to be represented as a `MutService` which simply does not mutate itself.
pub struct ServiceMut<I, S> {
    service: S,
    _phantom: PhantomData<fn(I)>,
}

impl<I, S: Service<I>> ServiceMut<I, S> {
    pub fn new(service: S) -> Self {
        Self {
            service,
            _phantom: PhantomData,
        }
    }
}
impl<I, S: Service<I>> MutService<I> for ServiceMut<I, S> {
    type Output = S::Output;
    type Error = S::Error;
    fn process(&mut self, input: I) -> Result<Self::Output, Self::Error> {
        self.service.process(input)
    }
}
impl<I, S: Service<I>> From<S> for ServiceMut<I, S> {
    fn from(service: S) -> Self {
        Self {
            service,
            _phantom: PhantomData,
        }
    }
}

/// An [`AsyncService`] that encapsulates an underlying [`Service`], exposing it as `async`.
/// Any `Service` should be able to be represented as an `AsyncService`, since any async code should be able to call any sync code.
pub struct ServiceAsync<'a, I: Send + 'a, S: Service<I>> {
    service: S,
    _phantom: PhantomData<fn(&'a I)>,
}
impl<'a, I: Send + 'a, S: Service<I> + Send + Sync> ServiceAsync<'a, I, S> {
    pub fn new(service: S) -> Self {
        Self {
            service,
            _phantom: PhantomData,
        }
    }
}
#[async_trait]
impl<'a, I: Send + 'a, S: Service<I> + Send + Sync> AsyncService<I> for ServiceAsync<'a, I, S> {
    type Output = S::Output;
    type Error = S::Error;
    async fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        self.service.process(input)
    }
}
impl<'a, I: Send + 'a, S: Service<I> + Send + Sync> From<S> for ServiceAsync<'a, I, S> {
    fn from(service: S) -> Self {
        Self {
            service,
            _phantom: PhantomData,
        }
    }
}

/// Used as a generic input to accept any [`Service`] or [`MutService`] as a [`MutService`].
///
/// This is used by `ServiceChain::start_mut(MutService)` to accept either a [`Service`] or [`MutService`].
pub trait IntoMutService<I, S: MutService<I>> {
    fn into_mut(self) -> S;
}
impl<I, S: Service<I>> IntoMutService<I, ServiceMut<I, S>> for S {
    fn into_mut(self) -> ServiceMut<I, S> {
        ServiceMut::new(self)
    }
}
impl<I, S: MutService<I>> IntoMutService<I, S> for S {
    fn into_mut(self) -> S {
        self
    }
}

/// Used as a generic input to accept any [`Service`] or [`AsyncService`] as an [`AsyncService`].
///
/// This is used by `ServiceChain::start_async(AsyncService)` to accept either a [`Service`] or [`AsyncService`].
pub trait IntoAsyncService<I, S: AsyncService<I>> {
    fn into_async(self) -> S;
}
impl<'a, I: Send + 'a, S: Service<I> + Send + Sync> IntoAsyncService<I, ServiceAsync<'a, I, S>>
    for S
where
    <S as Service<I>>::Output: Send,
    <S as Service<I>>::Error: Send,
{
    fn into_async(self) -> ServiceAsync<'a, I, S> {
        ServiceAsync::new(self)
    }
}
impl<I, S: AsyncService<I>> IntoAsyncService<I, S> for S {
    fn into_async(self) -> S {
        self
    }
}

/// A [`Service`] which encapsulates a `Box<dyn Service<...>>`.
///
/// This is useful when you have a [`Service`] with a complicated compile-time type and which to pass it around with a simplified signature.
pub struct DynService<'a, I, O, E> {
    service: Box<dyn Service<I, Output = O, Error = E> + 'a>,
}
impl<'a, I, O, E> DynService<'a, I, O, E> {
    pub fn new<S: Service<I, Output = O, Error = E> + 'a>(service: S) -> Self {
        Self {
            service: Box::new(service),
        }
    }
}
impl<'a, I, O, E> Service<I> for DynService<'a, I, O, E> {
    type Output = O;
    type Error = E;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        self.service.process(input)
    }
}

/// A [`MutService`] which encapsulates a `Box<dyn MutService<...>>`.
///
/// This is useful when you have a [MutService] with a complicated compile-time type and which to pass it around with a simplified signature.
pub struct DynMutService<'a, I, O, E> {
    service: Box<dyn MutService<I, Output = O, Error = E> + 'a>,
}
impl<'a, I, O, E> DynMutService<'a, I, O, E> {
    pub fn new<S: MutService<I, Output = O, Error = E> + 'a>(service: S) -> Self {
        Self {
            service: Box::new(service),
        }
    }
}
impl<'a, I, O, E> MutService<I> for DynMutService<'a, I, O, E> {
    type Output = O;
    type Error = E;
    fn process(&mut self, input: I) -> Result<Self::Output, Self::Error> {
        self.service.process(input)
    }
}

/// An [`AsyncService`] which encapsulates a `Box<dyn AsyncService<...>>`.
///
/// This is useful when you have a [AsyncService] with a complicated compile-time type and which to pass it around with a simplified signature.
pub struct DynAsyncService<'a, I, O, E> {
    service: Box<dyn AsyncService<I, Output = O, Error = E> + 'a>,
}
impl<'a, I, O, E> DynAsyncService<'a, I, O, E> {
    pub fn new<S: AsyncService<I, Output = O, Error = E> + 'a>(service: S) -> Self {
        Self {
            service: Box::new(service),
        }
    }
}
impl<'a, I, O, E> AsyncService<I> for DynAsyncService<'a, I, O, E> {
    type Output = O;
    type Error = E;
    fn process<'b, 'async_trait>(
        &'b self,
        input: I,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<Output = Result<Self::Output, Self::Error>>
                + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'b: 'async_trait,
        Self: 'async_trait,
    {
        self.service.process(input)
    }
}

/// A [`Service`] which can accept any input that can be [`Into`]ed an ouput, always returning `Ok(output)`.
pub struct IntoService<O> {
    _phantom: PhantomData<fn(O)>,
}
impl<O> IntoService<O> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
impl<O, I: Into<O>> Service<I> for IntoService<O> {
    type Output = O;
    type Error = ();
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        Ok(input.into())
    }
}

/// A [`Service`] which can accept any input that can be [`Into`]ed an ouput, returning `Result<Self::Output, TryInto::Error>`.
pub struct TryIntoService<O> {
    _phantom: PhantomData<fn(O)>,
}
impl<O> TryIntoService<O> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
impl<O, E, I: TryInto<O, Error = E>> Service<I> for TryIntoService<O> {
    type Output = O;
    type Error = E;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        input.try_into()
    }
}

/// A [`Service`] which no-ops, passing the input as `Ok(output)`.
pub struct NoOpService<'a> {
    _phantom: PhantomData<fn(&'a ())>,
}
impl<'a> NoOpService<'a> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
impl<'a, T> Service<T> for NoOpService<'a> {
    type Output = T;
    type Error = ();
    fn process(&self, input: T) -> Result<T, ()> {
        Ok(input)
    }
}
impl<'a, T> MutService<T> for NoOpService<'a> {
    type Output = T;
    type Error = ();
    fn process(&mut self, input: T) -> Result<T, ()> {
        Ok(input)
    }
}
#[async_trait]
impl<'a, T: Send + 'a> AsyncService<T> for NoOpService<'a> {
    type Output = T;
    type Error = ();
    async fn process(&self, input: T) -> Result<T, ()> {
        Ok(input)
    }
}

/// A [`Service`], which encapsulates a [`MutService`], using [`std::cell::RefCell`] to aquire mutability in each call to `process`.
///
/// This service is never `Sync`, but may be `Send` if the underlying [`Service`] is `Send`.
pub struct RefCellService<I, S: Service<I>> {
    service: RefCell<S>,
    _phantom: PhantomData<fn(I)>,
}
impl<I, S: Service<I>> RefCellService<I, S> {
    pub fn new(service: S) -> Self {
        Self {
            service: RefCell::new(service),
            _phantom: PhantomData,
        }
    }
}
impl<I, S: Service<I>> Service<I> for RefCellService<I, S> {
    type Output = S::Output;
    type Error = S::Error;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        self.service.borrow_mut().process(input)
    }
}

/// A [`Service`], which encapsulates a [`MutService`], using [`std::sync::Mutex`] to aquire mutability in each call to `process`.
///
/// This service both `Send` and `Sync`.
pub struct MutexService<I, S: Service<I>> {
    service: Mutex<S>,
    _phantom: PhantomData<fn(I)>,
}
impl<I, S: Service<I>> MutexService<I, S> {
    pub fn new(service: S) -> Self {
        Self {
            service: Mutex::new(service),
            _phantom: PhantomData,
        }
    }
}
impl<I, S: Service<I>> Service<I> for MutexService<I, S> {
    type Output = S::Output;
    type Error = S::Error;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        self.service.lock().expect("poisoned mutex").process(input)
    }
}

/// A [`Service`], which encapsulates an `Arc<Service<Input>>`.
///
/// This service can encapsulate a [`MutexService`], providing a `Send` + `Sync` service that can be cloned and referenced by multiple threads.
#[derive(Clone)]
pub struct ArcService<I, S: Service<I>> {
    service: Arc<S>,
    _phantom: PhantomData<fn(I)>,
}
impl<I, S: Service<I>> ArcService<I, S> {
    pub fn new(service: S) -> Self {
        Self {
            service: Arc::new(service),
            _phantom: PhantomData,
        }
    }
}
impl<I, S: Service<I>> Service<I> for ArcService<I, S> {
    type Output = S::Output;
    type Error = S::Error;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        self.service.process(input)
    }
}

/// A [`Service`], which encapsulates a [`Fn`].
pub struct FnService<I, O, E, F: Fn(I) -> Result<O, E>> {
    function: F,
    _phantom: PhantomData<fn(I, O, E)>,
}
impl<I, O, E, F: Fn(I) -> Result<O, E>> FnService<I, O, E, F> {
    pub fn new(function: F) -> Self {
        Self {
            function,
            _phantom: PhantomData,
        }
    }
}
impl<I, O, E, F: Fn(I) -> Result<O, E>> Service<I> for FnService<I, O, E, F> {
    type Output = O;
    type Error = E;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        (self.function)(input)
    }
}

/// A [`Service`], which encapsulates a [`FnMut`].
pub struct FnMutService<I, O, E, F: FnMut(I) -> Result<O, E>> {
    function: F,
    _phantom: PhantomData<fn(I, O, E)>,
}
impl<I, O, E, F: FnMut(I) -> Result<O, E>> FnMutService<I, O, E, F> {
    pub fn new(function: F) -> Self {
        Self {
            function,
            _phantom: PhantomData,
        }
    }
}
impl<I, O, E, F: Fn(I) -> Result<O, E>> MutService<I> for FnMutService<I, O, E, F> {
    type Output = O;
    type Error = E;
    fn process(&mut self, input: I) -> Result<Self::Output, Self::Error> {
        (self.function)(input)
    }
}

/// A [`Service`], [`MutService`], or [`AsyncService`] that encapsulates two service and accepts a [`Clone`]able input, which is passed to both underlying services, returning their outputs as a tuple.
pub struct CloningForkService<I: Clone, S1, S2> {
    first: S1,
    second: S2,
    _phantom: PhantomData<fn(I)>,
}
impl<I: Clone, S1, S2> CloningForkService<I, S1, S2> {
    pub fn new(first: S1, second: S2) -> Self {
        Self {
            first,
            second,
            _phantom: PhantomData,
        }
    }
}
impl<I: Clone, E, S1: Service<I, Error = E>, S2: Service<I, Error = E>> Service<I>
    for CloningForkService<I, S1, S2>
{
    type Output = (S1::Output, S2::Output);
    type Error = E;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        Ok((
            self.first.process(input.clone())?,
            self.second.process(input)?,
        ))
    }
}
impl<I: Clone, E, S1: MutService<I, Error = E>, S2: MutService<I, Error = E>> MutService<I>
    for CloningForkService<I, S1, S2>
{
    type Output = (S1::Output, S2::Output);
    type Error = E;
    fn process(&mut self, input: I) -> Result<Self::Output, Self::Error> {
        Ok((
            self.first.process(input.clone())?,
            self.second.process(input)?,
        ))
    }
}
#[async_trait]
impl<
        'a,
        I: Clone + Send + 'a,
        E: Debug + Send + 'a,
        S1: AsyncService<I, Error = E> + Send + Sync,
        S2: AsyncService<I, Error = E> + Send + Sync,
    > AsyncService<I> for CloningForkService<I, S1, S2>
where
    <S1 as AsyncService<I>>::Output: Send + 'a,
    <S2 as AsyncService<I>>::Output: Send + 'a,
{
    type Output = (S1::Output, S2::Output);
    type Error = E;
    async fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        let fut1 = self.first.process(input.clone());
        let fut2 = self.second.process(input);
        let o1 = fut1.await?;
        let o2 = fut2.await?;
        Ok((o1, o2))
    }
}

/// A [`Service`], [`MutService`], or [`AsyncService`] that encapsulates two service and accepts a input as a reference, which is passed to both underlying services, returning their outputs as a tuple.
pub struct RefForkService<I, S1, S2> {
    first: S1,
    second: S2,
    _phantom: PhantomData<fn(I)>,
}
impl<I, S1, S2> RefForkService<I, S1, S2> {
    pub fn new(first: S1, second: S2) -> Self {
        Self {
            first,
            second,
            _phantom: PhantomData,
        }
    }
}
impl<'a, I: 'a, E, S1: Service<&'a I, Error = E>, S2: Service<&'a I, Error = E>> Service<&'a I>
    for RefForkService<I, S1, S2>
{
    type Output = (S1::Output, S2::Output);
    type Error = E;
    fn process(&self, input: &'a I) -> Result<Self::Output, Self::Error> {
        Ok((self.first.process(input)?, self.second.process(input)?))
    }
}
impl<'a, I: 'a, E, S1: MutService<&'a I, Error = E>, S2: MutService<&'a I, Error = E>>
    MutService<&'a I> for RefForkService<I, S1, S2>
{
    type Output = (S1::Output, S2::Output);
    type Error = E;
    fn process(&mut self, input: &'a I) -> Result<Self::Output, Self::Error> {
        Ok((self.first.process(input)?, self.second.process(input)?))
    }
}

/// A [`Service`], which encapsulates an [`AsyncService`], using [`futures::executor::block_on`] to process it to completion, returning the underlying result synchronously.
pub struct BlockingService<'a, I: Send + 'a, S: AsyncService<I>> {
    service: S,
    _phantom: PhantomData<fn(&'a I)>,
}
impl<'a, I: Send + 'a, S: AsyncService<I>> BlockingService<'a, I, S> {
    pub fn new(service: S) -> Self {
        Self {
            service,
            _phantom: PhantomData,
        }
    }
}
impl<'a, I: Send + 'a, S: AsyncService<I>> Service<I> for BlockingService<'a, I, S> {
    type Output = S::Output;
    type Error = S::Error;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        futures::executor::block_on(self.service.process(input))
    }
}

/// A [`Service`], which encapsulates a `Service<(), Output = Option<T>>`, blocking with the given idle function until a value is returned or the idle function returns an error.
///
/// When the underlying `Service<()>` returns None, the given idle function will be called.
/// The idle function will be called repeatedly, given the attempt number as input, until Some(T) is returned by the underlying service.
/// The idle function may return `Err(RetryError::Interrupted)` to return an error and avoid blocking forever.
///
/// See the [`idle`] module for some provided idle functions.
pub struct PollService<O, E, S, F>
where
    E: Debug,
    S: Service<(), Output = Option<O>, Error = E>,
    F: Fn(usize) -> Result<(), RetryError<E>>,
{
    service: S,
    idle: F,
    _phantom: PhantomData<fn(E)>,
}
impl<O, E, S, F> PollService<O, E, S, F>
where
    E: Debug,
    S: Service<(), Output = Option<O>, Error = E>,
    F: Fn(usize) -> Result<(), RetryError<E>>,
{
    pub fn new(service: S, idle: F) -> Self {
        Self {
            service,
            idle,
            _phantom: PhantomData,
        }
    }
}
impl<O, E, S, F> Service<()> for PollService<O, E, S, F>
where
    E: Debug,
    S: Service<(), Output = Option<O>, Error = E>,
    F: Fn(usize) -> Result<(), RetryError<E>>,
{
    type Output = O;
    type Error = RetryError<S::Error>;
    fn process(&self, _: ()) -> Result<Self::Output, Self::Error> {
        let mut attempt = 0;
        loop {
            match self.service.process(()) {
                Ok(Some(v)) => return Ok(v),
                Ok(None) => {
                    if let Err(err) = (self.idle)(attempt) {
                        return Err(err);
                    }
                }
                Err(err) => return Err(RetryError::ServiceError(err)),
            }
            attempt += 1;
        }
    }
}

/// To be implemented by non-blocking services which may return the moved input in a resulting `Err` to be retried.
///
/// This allows a [`RetryService`] to wrap a `RetryableService`.
pub trait RetryableService<I, E>: Service<I, Error = E> {
    fn parse_retry(&self, err: E) -> Result<I, RetryError<E>>;
}

/// A [`Service`], which encapsulates a [`RetryableService`], blocking and retrying until a value is returned, an un-retryable error is encountered, or the idle function returns an `Err`.
///
/// When the underlying service's `Service::process` function returns an Err, it is passed to the given `RetryableService`, which must return an `Ok(Input)` to retry or an `Err` to return immediately.
/// Between retries, the given `idle` function is called, given the attempt number as input, until `Ok(Output)` is returned by the underlying `Service` or `Err` is returned by the `RetryableService` or `idle` function.
///
/// See the [`idle`] module for some provided idle functions.
pub struct RetryService<I, E, S, F>
where
    S: RetryableService<I, E>,
    F: Fn(usize) -> Result<(), RetryError<E>>,
{
    service: S,
    idle: F,
    _phantom: PhantomData<fn(I, E)>,
}
impl<I, E, S, F> RetryService<I, E, S, F>
where
    S: RetryableService<I, E>,
    F: Fn(usize) -> Result<(), RetryError<E>>,
{
    pub fn new(service: S, idle: F) -> Self {
        Self {
            service,
            idle,
            _phantom: PhantomData,
        }
    }
}
impl<I, E, S, F> Service<I> for RetryService<I, E, S, F>
where
    S: RetryableService<I, E>,
    F: Fn(usize) -> Result<(), RetryError<E>>,
{
    type Output = S::Output;
    type Error = RetryError<S::Error>;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        let mut input = input;
        let mut attempt = 0;
        loop {
            match self.service.process(input) {
                Ok(v) => return Ok(v),
                Err(err) => match (self.idle)(attempt) {
                    Ok(()) => match self.service.parse_retry(err) {
                        Ok(v) => input = v,
                        Err(err) => return Err(err),
                    },
                    Err(err) => return Err(err),
                },
            }
            attempt += 1;
        }
    }
}

/// Used by idle and retry services to interrupt a poll or retry loop
#[derive(Clone)]
pub enum RetryError<E> {
    Interrupted,
    ServiceError(E),
}
impl<E: PartialEq> PartialEq for RetryError<E> {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::Interrupted => match other {
                Self::Interrupted => true,
                Self::ServiceError(_) => false,
            },
            Self::ServiceError(err) => match other {
                Self::Interrupted => false,
                Self::ServiceError(other_err) => err == other_err,
            },
        }
    }
}
impl<E: Debug> Debug for RetryError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interrupted => f.write_str("Interrupted"),
            Self::ServiceError(e) => write!(f, "{e:?}"),
        }
    }
}

/// A chain of [`Service`], [`MutService`], or [`AsyncService`] implementations, which is itself a single [`Service`], [`MutService`], or [`AsyncService`] that accepts the first service in the chain's input and produces the the last service in the chain's output.
/// When any service in the chain returns an `Err`, the chain will break early, encapsulate the error in a `ServiceChainError`, and return `Err(ServiceChainError)` immediately.
///
/// `ServiceChain::start(Service)` will start a service chain of [`Service`]s.
/// `ServiceChain::start_mut(Service)` will start a service chain of [`MutService`]s, using [`IntoMutService`] to chain together the services.
/// `ServiceChain::start_async(Service)` will start a service chain of [`AsyncService`]s, using [`IntoAsyncService`] to chain together the services.
///
/// Example of a series of `AddService`s chained together to produce a final result.
/// ```
/// use sod::{Service, ServiceChain};
///
/// struct AddService {
///     n: usize,
/// }
/// impl AddService {
///     pub fn new(n: usize) -> Self {
///         Self { n }
///     }
/// }
/// impl Service<usize> for AddService {
///     type Output = usize;
///     type Error = ();
///     fn process(&self, input: usize) -> Result<usize, ()> {
///         Ok(input + self.n)
///     }
/// }
///
/// let chain = ServiceChain::start(AddService::new(1))
///     .next(AddService::new(2))
///     .next(AddService::new(4))
///     .end();
/// let result = chain.process(100).unwrap();
/// assert_eq!(107, result);
/// ```
pub struct ServiceChain<I, P, S> {
    prev: P,
    service: S,
    _phantom: PhantomData<fn(I)>,
}
impl<'a, I, S: Service<I>> ServiceChain<I, NoOpService<'a>, S> {
    /// Start a new service chain using the given [`Service`] as the first service in the chain.
    /// This will return a [`ServiceChainBuilder`] that will allow you to link more [`Service`]s to finish building the [`ServiceChain`].
    pub fn start(service: S) -> ServiceChainBuilder<I, NoOpService<'a>, S> {
        ServiceChainBuilder::start(service)
    }
}
impl<'a, I, S: MutService<I>> ServiceChain<I, NoOpService<'a>, S> {
    /// Start a new mutable service chain using the given [`MutService`] as the first service in the chain.
    /// This will return a [`ServiceChainBuilder`] that will allow you to link more [`MutService`]s to finish building the [`ServiceChain`].
    pub fn start_mut(service: S) -> MutServiceChainBuilder<I, NoOpService<'a>, S> {
        MutServiceChainBuilder::start(service)
    }
}
impl<'a, I: Send + 'a, S: AsyncService<I>> ServiceChain<I, NoOpService<'a>, S> {
    /// Start a new async service chain using the given [`AsyncService`] as the first service in the chain.
    /// This will return a [`ServiceChainBuilder`] that will allow you to link more [`AsyncService`]s to finish building the [`ServiceChain`].
    pub fn start_async(service: S) -> AsyncServiceChainBuilder<I, NoOpService<'a>, S> {
        AsyncServiceChainBuilder::start(service)
    }
}
impl<I, P: Service<I>, S: Service<P::Output>> Service<I> for ServiceChain<I, P, S>
where
    <P as Service<I>>::Error: Debug + 'static,
    <S as Service<P::Output>>::Error: Debug + 'static,
{
    type Output = S::Output;
    type Error = ServiceChainError<Box<dyn Debug>>;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        let input = match self.prev.process(input) {
            Ok(o) => o,
            Err(e) => return Err(ServiceChainError::new(Box::new(e))),
        };
        let output = match self.service.process(input) {
            Ok(o) => o,
            Err(e) => return Err(ServiceChainError::new(Box::new(e))),
        };
        Ok(output)
    }
}
impl<I, P: MutService<I>, S: MutService<P::Output>> MutService<I> for ServiceChain<I, P, S>
where
    <P as MutService<I>>::Error: Debug + 'static,
    <S as MutService<P::Output>>::Error: Debug + 'static,
{
    type Output = S::Output;
    type Error = ServiceChainError<Box<dyn Debug>>;
    fn process(&mut self, input: I) -> Result<Self::Output, Self::Error> {
        let input = match self.prev.process(input) {
            Ok(o) => o,
            Err(e) => return Err(ServiceChainError::new(Box::new(e))),
        };
        let output = match self.service.process(input) {
            Ok(o) => o,
            Err(e) => return Err(ServiceChainError::new(Box::new(e))),
        };
        Ok(output)
    }
}
#[async_trait]
impl<I: Send, P: AsyncService<I> + Send + Sync, S: AsyncService<P::Output> + Send + Sync>
    AsyncService<I> for ServiceChain<I, P, S>
where
    <P as AsyncService<I>>::Error: Debug + Send + 'static,
    <S as AsyncService<P::Output>>::Error: Debug + Send + 'static,
    <P as AsyncService<I>>::Output: Send,
    <S as AsyncService<P::Output>>::Output: Send,
{
    type Output = S::Output;
    type Error = ServiceChainError<Box<dyn Debug + Send>>;
    async fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        let input = match self.prev.process(input).await {
            Ok(o) => o,
            Err(e) => return Err(ServiceChainError::new(Box::new(e))),
        };
        let output = match self.service.process(input).await {
            Ok(o) => o,
            Err(e) => return Err(ServiceChainError::new(Box::new(e))),
        };
        Ok(output)
    }
}

/// Returned by [`ServiceChain`] when a service in the chain returns an `Err` [`Result`].
pub struct ServiceChainError<C: Debug> {
    cause: C,
}
impl<C: Debug> ServiceChainError<C> {
    fn new(cause: C) -> Self {
        Self { cause }
    }
}
impl<C: Debug> Error for ServiceChainError<C> {}
impl<C: Debug> Debug for ServiceChainError<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceChainError")
            .field("cause", &self.cause)
            .finish()
    }
}
impl<C: Debug> Display for ServiceChainError<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ServiceChainError")
    }
}

/// Returned by `ServiceChain::start` to build a sync service chain.
/// Use the `next(self, Service)` function to append more services to the [`ServiceChain`].
/// Use the `end(self)` function to finish building and return the resulting [`ServiceChain`].
pub struct ServiceChainBuilder<I, P: Service<I>, S: Service<P::Output>> {
    chain: ServiceChain<I, P, S>,
}
impl<'a, I, S: Service<I>> ServiceChainBuilder<I, NoOpService<'a>, S> {
    /// from ServiceChain::start()
    fn start(service: S) -> ServiceChainBuilder<I, NoOpService<'a>, S> {
        ServiceChainBuilder {
            chain: ServiceChain {
                prev: NoOpService::new(),
                service,
                _phantom: PhantomData,
            },
        }
    }
}
impl<I, P: Service<I>, S: Service<P::Output>> ServiceChainBuilder<I, P, S>
where
    <P as Service<I>>::Error: Debug,
    <S as Service<P::Output>>::Error: Debug,
{
    /// Append another [`Service`] to the end of the service chain.
    pub fn next<NS: Service<S::Output>>(
        self,
        service: NS,
    ) -> ServiceChainBuilder<I, ServiceChain<I, P, S>, NS> {
        ServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service,
                _phantom: PhantomData,
            },
        }
    }
}
impl<I, P: Service<I>, S: Service<P::Output>> ServiceChainBuilder<I, P, S>
where
    <P as Service<I>>::Error: Debug,
    <S as Service<P::Output>>::Error: Debug,
    <S as Service<P::Output>>::Output: Clone,
{
    /// Fork the service chain to the given two services by cloning the input.
    pub fn fork_clone<E, NS1: Service<S::Output, Error = E>, NS2: Service<S::Output, Error = E>>(
        self,
        first: NS1,
        second: NS2,
    ) -> ServiceChainBuilder<I, ServiceChain<I, P, S>, CloningForkService<S::Output, NS1, NS2>>
    {
        ServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service: CloningForkService::new(first, second),
                _phantom: PhantomData,
            },
        }
    }
}
impl<'a, I: 'a, P: Service<I> + 'a, S: Service<P::Output> + 'a> ServiceChainBuilder<I, P, S>
where
    <P as Service<I>>::Error: Debug + 'static,
    <S as Service<P::Output>>::Error: Debug + 'static,
{
    /// End and return the resulting [`ServiceChain`].
    pub fn end(self) -> ServiceChain<I, P, S> {
        self.chain
    }
    /// End and return the resulting [`ServiceChain`] as a [`DynService`].
    /// A resulting [`ServiceChain`] is likely to have a complex compile-time type.
    /// Wrapping in a [`DynService`] simplifies the type signature, making it easier to return or pass as an input into another function.
    pub fn end_dyn(self) -> DynService<'a, I, S::Output, ServiceChainError<Box<dyn Debug>>> {
        DynService::new(self.chain)
    }
}

/// Returned by `ServiceChain::start_mut` to build a mut service chain.
/// Use the `next(self, IntoMutService)` function to append more services to the [`ServiceChain`].
/// Use the `end(self)` function to finish building and return the resulting [`ServiceChain`].
pub struct MutServiceChainBuilder<I, P: MutService<I>, S: MutService<P::Output>> {
    chain: ServiceChain<I, P, S>,
}
impl<'a, I, S: MutService<I>> MutServiceChainBuilder<I, NoOpService<'a>, S> {
    /// from ServiceChain::start_mut()
    fn start<T: IntoMutService<I, S>>(service: T) -> MutServiceChainBuilder<I, NoOpService<'a>, S> {
        MutServiceChainBuilder {
            chain: ServiceChain {
                prev: NoOpService::new(),
                service: service.into_mut(),
                _phantom: PhantomData,
            },
        }
    }
}
impl<I, P: MutService<I>, S: MutService<P::Output>> MutServiceChainBuilder<I, P, S>
where
    <P as MutService<I>>::Error: Debug,
    <S as MutService<P::Output>>::Error: Debug,
{
    /// Append another [`MutService`] to the end of the service chain, using [`IntoMutService`] to accept either a [`Service`] or [`MutService`].
    pub fn next<NS: MutService<S::Output>, T: IntoMutService<S::Output, NS>>(
        self,
        service: T,
    ) -> MutServiceChainBuilder<I, ServiceChain<I, P, S>, NS> {
        MutServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service: service.into_mut(),
                _phantom: PhantomData,
            },
        }
    }
}
impl<I, P: MutService<I>, S: MutService<P::Output>> MutServiceChainBuilder<I, P, S>
where
    <P as MutService<I>>::Error: Debug,
    <S as MutService<P::Output>>::Error: Debug,
    <S as MutService<P::Output>>::Output: Clone,
{
    /// Fork the service chain to the given two services by cloning the input.
    pub fn fork_clone<
        E,
        NS1: MutService<S::Output, Error = E>,
        NS2: MutService<S::Output, Error = E>,
    >(
        self,
        first: NS1,
        second: NS2,
    ) -> MutServiceChainBuilder<I, ServiceChain<I, P, S>, CloningForkService<S::Output, NS1, NS2>>
    {
        MutServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service: CloningForkService::new(first, second),
                _phantom: PhantomData,
            },
        }
    }
}
impl<'a, I: 'a, P: MutService<I> + 'a, S: MutService<P::Output> + 'a>
    MutServiceChainBuilder<I, P, S>
where
    <P as MutService<I>>::Error: Debug + 'static,
    <S as MutService<P::Output>>::Error: Debug + 'static,
{
    /// End and return the resulting [`ServiceChain`].
    pub fn end(self) -> ServiceChain<I, P, S> {
        self.chain
    }
    /// End and return the resulting [`ServiceChain`] as a [`DynMutService`].
    /// A resulting [`ServiceChain`] is likely to have a complex compile-time type.
    /// Wrapping in a [`DynMutService`] simplifies the type signature, making it easier to return or pass as an input into another function.
    pub fn end_dyn(self) -> DynMutService<'a, I, S::Output, ServiceChainError<Box<dyn Debug>>> {
        DynMutService::new(self.chain)
    }
}

/// Returned by `ServiceChain::start_async` to build an async service chain.
/// Use the `next(self, IntoAsyncService)` function to append more services to the [`ServiceChain`].
/// Use the `end(self)` function to finish building and return the resulting [`ServiceChain`].
pub struct AsyncServiceChainBuilder<I: Send, P: AsyncService<I>, S: AsyncService<P::Output>> {
    chain: ServiceChain<I, P, S>,
}
impl<'a, I: Send, S: AsyncService<I>> AsyncServiceChainBuilder<I, NoOpService<'a>, S> {
    /// from ServiceChain::start_async()
    fn start<T: IntoAsyncService<I, S>>(
        service: T,
    ) -> AsyncServiceChainBuilder<I, NoOpService<'a>, S> {
        AsyncServiceChainBuilder {
            chain: ServiceChain {
                prev: NoOpService::new(),
                service: service.into_async(),
                _phantom: PhantomData,
            },
        }
    }
}
impl<I: Send, P: AsyncService<I> + Send + Sync, S: AsyncService<P::Output> + Send + Sync>
    AsyncServiceChainBuilder<I, P, S>
where
    <P as AsyncService<I>>::Error: Debug + Send,
    <S as AsyncService<P::Output>>::Error: Debug + Send,
    <P as AsyncService<I>>::Output: Send,
    <S as AsyncService<P::Output>>::Output: Send,
{
    /// Append another [`AsyncService`] to the end of the service chain, using [`IntoAsyncService`] to accept either a [`Service`] or [`AsyncService`].
    pub fn next<NS: AsyncService<S::Output>, T: IntoAsyncService<S::Output, NS>>(
        self,
        service: T,
    ) -> AsyncServiceChainBuilder<I, ServiceChain<I, P, S>, NS> {
        AsyncServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service: service.into_async(),
                _phantom: PhantomData,
            },
        }
    }
}
impl<I: Send, P: AsyncService<I> + Send + Sync, S: AsyncService<P::Output> + Send + Sync>
    AsyncServiceChainBuilder<I, P, S>
where
    <P as AsyncService<I>>::Error: Debug + Send,
    <S as AsyncService<P::Output>>::Error: Debug + Send,
    <P as AsyncService<I>>::Output: Send,
    <S as AsyncService<P::Output>>::Output: Send + Clone,
{
    /// Fork the service chain to the given two services by cloning the input.
    pub fn fork_clone<
        E: Debug + Send,
        NS1: AsyncService<S::Output, Error = E> + Send + Sync,
        NS2: AsyncService<S::Output, Error = E> + Send + Sync,
    >(
        self,
        first: NS1,
        second: NS2,
    ) -> AsyncServiceChainBuilder<I, ServiceChain<I, P, S>, CloningForkService<S::Output, NS1, NS2>>
    where
        <NS1 as AsyncService<S::Output>>::Output: Send,
        <NS2 as AsyncService<S::Output>>::Output: Send,
    {
        AsyncServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service: CloningForkService::new(first, second),
                _phantom: PhantomData,
            },
        }
    }
}
impl<
        'a,
        I: Send + 'a,
        P: AsyncService<I> + Send + Sync + 'a,
        S: AsyncService<P::Output> + Send + Sync + 'a,
    > AsyncServiceChainBuilder<I, P, S>
where
    <P as AsyncService<I>>::Error: Send + Debug + 'static,
    <S as AsyncService<P::Output>>::Error: Send + Debug + 'static,
    <P as AsyncService<I>>::Output: Send + 'a,
    <S as AsyncService<P::Output>>::Output: Send + 'a,
{
    /// End and return the resulting [`ServiceChain`].
    pub fn end(self) -> ServiceChain<I, P, S> {
        self.chain
    }
    /// End and return the resulting [`ServiceChain`] as a [`DynAsyncService`].
    /// A resulting [`ServiceChain`] is likely to have a complex compile-time type.
    /// Wrapping in a [`DynAsyncService`] simplifies the type signature, making it easier to return or pass as an input into another function.
    pub fn end_dyn(
        self,
    ) -> DynAsyncService<'a, I, S::Output, ServiceChainError<Box<dyn Debug + Send>>> {
        DynAsyncService::new(self.chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AddService {
        n: usize,
    }
    impl AddService {
        pub fn new(n: usize) -> Self {
            Self { n }
        }
    }
    impl Service<usize> for AddService {
        type Output = usize;
        type Error = ();
        fn process(&self, input: usize) -> Result<usize, ()> {
            Ok(input + self.n)
        }
    }

    struct AppendService {
        n: usize,
    }
    impl AppendService {
        pub fn new() -> Self {
            Self { n: 0 }
        }
    }
    impl MutService<usize> for AppendService {
        type Output = usize;
        type Error = ();
        fn process(&mut self, input: usize) -> Result<usize, ()> {
            self.n += input;
            Ok(self.n)
        }
    }

    #[test]
    fn service_chain() {
        let chain = ServiceChain::start(AddService::new(1))
            .next(AddService::new(2))
            .next(AddService::new(4))
            .end();
        let result = chain.process(100).unwrap();
        assert_eq!(107, result);
    }

    #[test]
    fn mut_service_chain() {
        let mut chain = ServiceChain::start_mut(AppendService::new()).end();
        chain.process(1).unwrap();
        chain.process(2).unwrap();
        let result = chain.process(4).unwrap();
        assert_eq!(7, result);
    }

    #[test]
    fn async_service_chain() {
        let chain = ServiceChain::start_async(ServiceAsync::new(AddService::new(1)))
            .next(ServiceAsync::new(AddService::new(2)))
            .next(ServiceAsync::new(AddService::new(4)))
            .end();
        let executor = BlockingService::new(chain);
        let result = executor.process(100).unwrap();
        assert_eq!(107, result);
    }
}
