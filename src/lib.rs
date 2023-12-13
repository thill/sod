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

use std::{
    borrow::Borrow,
    cell::RefCell,
    convert::Infallible,
    error::Error,
    fmt::{Debug, Display},
    marker::PhantomData,
    rc::Rc,
    sync::{Arc, Mutex},
    thread::{spawn, JoinHandle},
};

/// Provide support for `async fn` by exposing the external `async_trait` crate.
/// See [`async_trait`](mod@async_trait) for details.
#[doc(inline)]
pub use async_trait::async_trait;

pub mod idle;
pub mod thread;

/// A sync service trait
///
/// Accepts `&self` and an input, producing a `Result<Self::Output, Self::Error>`.
///
/// Conversion to [`MutService`] or [`AsyncService`]:
/// * The `into_mut` function can be used to convert a [`Service`] into a [`ServiceMut`]
/// * The `into_async` function can be used to convert a [`Service`] to a [`ServiceAsync`] when `Service::Input`, `Service::Output`, and `Service::Error` are [`Send`] and when [`Service`] is [`Send`] + [`Sync`]
pub trait Service {
    type Input;
    type Output;
    type Error;

    /// Process an input, producing a `Result<Self::Output, Self::Error>`
    fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error>;

    /// Convert this [`Service`] into a [`ServiceMut`] which impls [`MutService`]
    fn into_mut(self) -> ServiceMut<Self>
    where
        Self: Sized,
    {
        ServiceMut::from(self)
    }

    /// Convert this [`Service`] into a [`ServiceAsync`] which impls [`AsyncService`]
    fn into_async(self) -> ServiceAsync<Self>
    where
        Self: Sized + Send + Sync + 'static,
    {
        ServiceAsync::from(self)
    }

    /// Convert this [`Service`] into a [`DynService`]
    fn into_dyn<'a>(self) -> DynService<'a, Self::Input, Self::Output, Self::Error>
    where
        Self: Sized + 'static,
    {
        DynService::new(self)
    }
}

/// A mut service trait
///
/// Accepts `&mut self` and an input, producing a `Result<Self::Output, Self::Error>`
pub trait MutService {
    type Input;
    type Output;
    type Error;
    fn process(&mut self, input: Self::Input) -> Result<Self::Output, Self::Error>;

    /// Convert this [`Service`] into a [`DynMutService`]
    fn into_dyn<'a>(self) -> DynMutService<'a, Self::Input, Self::Output, Self::Error>
    where
        Self: Sized + 'static,
    {
        DynMutService::new(self)
    }
}

/// An async service trait
///
/// Uses the [async_trait](https://docs.rs/async-trait/latest/async_trait/) to accept `&self` and an input asynchronously, producing a `Result<Self::Output, Self::Error>`
#[async_trait]
pub trait AsyncService: Send + Sync {
    type Input: Send + 'static;
    type Output: Send + 'static;
    type Error: Send + 'static;
    async fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error>;
}

/// A [`MutService`] that encapsulates an underlying [`Service`], exposing it as `mut`.
pub struct ServiceMut<S: Service> {
    service: S,
}
impl<'a, S: Service> ServiceMut<S> {
    pub fn new(service: S) -> Self {
        Self { service }
    }
}
#[async_trait]
impl<'a, S: Service> MutService for ServiceMut<S> {
    type Input = S::Input;
    type Output = S::Output;
    type Error = S::Error;
    fn process(&mut self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        self.service.process(input)
    }
}
impl<'a, S: Service> From<S> for ServiceMut<S> {
    fn from(service: S) -> Self {
        Self { service }
    }
}

/// An [`AsyncService`] that encapsulates an underlying [`Service`], exposing it as `async`.
pub struct ServiceAsync<S: Service> {
    service: S,
}
impl<'a, S: Service + Send + Sync> ServiceAsync<S> {
    pub fn new(service: S) -> Self {
        Self { service }
    }
}
#[async_trait]
impl<'a, S: Service + Send + Sync> AsyncService for ServiceAsync<S>
where
    S::Input: Send + 'static,
    S::Output: Send + 'static,
    S::Error: Send + 'static,
{
    type Input = S::Input;
    type Output = S::Output;
    type Error = S::Error;
    async fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        self.service.process(input)
    }
}
impl<'a, S: Service + Send + Sync> From<S> for ServiceAsync<S> {
    fn from(service: S) -> Self {
        Self { service }
    }
}

/// A [`Service`] which encapsulates a `Box<dyn Service<...>>`.
///
/// This is useful when you have a [`Service`] with a complicated compile-time type that needs to be passed to a function with a simplified signature.
pub struct DynService<'a, I, O, E> {
    service: Box<dyn Service<Input = I, Output = O, Error = E> + 'a>,
}
impl<'a, I, O, E> DynService<'a, I, O, E> {
    pub fn new<S: Service<Input = I, Output = O, Error = E> + 'a>(service: S) -> Self {
        Self {
            service: Box::new(service),
        }
    }
}
impl<'a, I, O, E> Service for DynService<'a, I, O, E> {
    type Input = I;
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
    service: Box<dyn MutService<Input = I, Output = O, Error = E> + 'a>,
}
impl<'a, I, O, E> DynMutService<'a, I, O, E> {
    pub fn new<S: MutService<Input = I, Output = O, Error = E> + 'a>(service: S) -> Self {
        Self {
            service: Box::new(service),
        }
    }
}
impl<'a, I, O, E> MutService for DynMutService<'a, I, O, E> {
    type Input = I;
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
    service: Box<dyn AsyncService<Input = I, Output = O, Error = E> + 'a>,
}
impl<'a, I, O, E> DynAsyncService<'a, I, O, E> {
    pub fn new<S: AsyncService<Input = I, Output = O, Error = E> + 'a>(service: S) -> Self {
        Self {
            service: Box::new(service),
        }
    }
}
impl<'a, I, O, E> AsyncService for DynAsyncService<'a, I, O, E>
where
    I: Send + 'static,
    O: Send + 'static,
    E: Send + 'static,
{
    type Input = I;
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
pub struct IntoService<O, I: Into<O>> {
    _phantom: PhantomData<fn(O, I)>,
}
impl<O, I: Into<O>> IntoService<O, I> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
impl<O, I: Into<O>> Service for IntoService<O, I> {
    type Input = I;
    type Output = O;
    type Error = Infallible;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        Ok(input.into())
    }
}

/// A [`Service`] which can accept any input that can be [`Into`]ed an ouput, returning `Result<Self::Output, TryInto::Error>`.
pub struct TryIntoService<O, I: TryInto<O>> {
    _phantom: PhantomData<fn(O, I)>,
}
impl<O, I: TryInto<O>> TryIntoService<O, I> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
impl<O, E, I: TryInto<O, Error = E>> Service for TryIntoService<O, I> {
    type Input = I;
    type Output = O;
    type Error = E;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        input.try_into()
    }
}

/// A [`Service`] which no-ops, passing the input as `Ok(output)`.
pub struct NoOpService<'a, T> {
    _phantom: PhantomData<fn(&'a T)>,
}
impl<'a, T> NoOpService<'a, T> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
impl<'a, T> Service for NoOpService<'a, T> {
    type Input = T;
    type Output = T;
    type Error = Infallible;
    fn process(&self, input: T) -> Result<T, Infallible> {
        Ok(input)
    }
}
impl<'a, T> MutService for NoOpService<'a, T> {
    type Input = T;
    type Output = T;
    type Error = Infallible;
    fn process(&mut self, input: T) -> Result<T, Infallible> {
        Ok(input)
    }
}
#[async_trait]
impl<'a, T: Send + 'static> AsyncService for NoOpService<'a, T> {
    type Input = T;
    type Output = T;
    type Error = Infallible;
    async fn process(&self, input: T) -> Result<T, Infallible> {
        Ok(input)
    }
}

/// A [`Service`], which encapsulates a [`MutService`], using [`std::cell::RefCell`] to aquire mutability in each call to `process`.
///
/// You may obtain a shared reference of this service using `RefCellService::clone(&service)`.
///
/// This service is never `Sync`, but may be `Send` if the underlying [`Service`] is `Send`.
pub struct RefCellService<S: MutService> {
    service: Rc<RefCell<S>>,
}
impl<S: MutService> RefCellService<S> {
    pub fn new(service: S) -> Self {
        Self {
            service: Rc::new(RefCell::new(service)),
        }
    }
}
impl<S: MutService> Service for RefCellService<S> {
    type Input = S::Input;
    type Output = S::Output;
    type Error = S::Error;
    fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        self.service.borrow_mut().process(input)
    }
}
impl<S: MutService> Clone for RefCellService<S> {
    fn clone(&self) -> Self {
        Self {
            service: Rc::clone(&self.service),
        }
    }
}

/// A [`Service`], which encapsulates a [`MutService`], using [`std::sync::Mutex`] to aquire mutability in each call to `process`.
///
/// This service both `Send` and `Sync`.
///
/// You may obtain a shared reference of this service using `MutexService::clone(&service)`.
///
/// The service will panic if the mutex returns a poison error.
pub struct MutexService<S> {
    service: Arc<Mutex<S>>,
}
impl<S> MutexService<S> {
    pub fn new(service: S) -> Self {
        Self {
            service: Arc::new(Mutex::new(service)),
        }
    }
}
impl<S: MutService> Service for MutexService<S> {
    type Input = S::Input;
    type Output = S::Output;
    type Error = S::Error;
    fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        self.service.lock().expect("poisoned mutex").process(input)
    }
}
impl<S: MutService> Clone for MutexService<S> {
    fn clone(&self) -> Self {
        Self {
            service: Arc::clone(&self.service),
        }
    }
}

/// A [`Service`], which encapsulates an `Arc<Service<Input>>`.
///
/// This service can encapsulate a [`MutexService`], providing a `Send` + `Sync` service that can be cloned and referenced by multiple threads.
#[derive(Clone)]
pub struct ArcService<S: Service> {
    service: Arc<S>,
}
impl<S: Service> ArcService<S> {
    pub fn new(service: S) -> Self {
        Self {
            service: Arc::new(service),
        }
    }
}
impl<S: Service> Service for ArcService<S> {
    type Input = S::Input;
    type Output = S::Output;
    type Error = S::Error;
    fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        self.service.process(input)
    }
}

/// A [`Service`], which encapsulates an `Arc<Service<Input>>`.
///
/// This service can encapsulate a [`MutexService`], providing a `Send` + `Sync` service that can be cloned and referenced by multiple threads.
#[derive(Clone)]
pub struct ArcAsyncService<S: AsyncService> {
    service: Arc<S>,
}
impl<S: AsyncService> ArcAsyncService<S> {
    pub fn new(service: S) -> Self {
        Self {
            service: Arc::new(service),
        }
    }
}
#[async_trait]
impl<S: AsyncService> AsyncService for ArcAsyncService<S> {
    type Input = S::Input;
    type Output = S::Output;
    type Error = S::Error;
    async fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        self.service.process(input).await
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
impl<I, O, E, F: Fn(I) -> Result<O, E>> Service for FnService<I, O, E, F> {
    type Input = I;
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
impl<I, O, E, F: Fn(I) -> Result<O, E>> MutService for FnMutService<I, O, E, F> {
    type Input = I;
    type Output = O;
    type Error = E;
    fn process(&mut self, input: I) -> Result<Self::Output, Self::Error> {
        (self.function)(input)
    }
}

/// A [`Service`], [`MutService`], or [`AsyncService`] that encapsulates two service and accepts a [`Clone`]able input, which is passed to both underlying services, returning their outputs as a tuple.
pub struct CloningForkService<S1, S2> {
    first: S1,
    second: S2,
}
impl<S1, S2> CloningForkService<S1, S2> {
    pub fn new(first: S1, second: S2) -> Self {
        Self { first, second }
    }
}
impl<S1: Service, S2: Service<Input = S1::Input, Error = S1::Error>> Service
    for CloningForkService<S1, S2>
where
    S1::Input: Clone,
{
    type Input = S1::Input;
    type Output = (S1::Output, S2::Output);
    type Error = S1::Error;
    fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        Ok((
            self.first.process(input.clone())?,
            self.second.process(input)?,
        ))
    }
}
impl<S1: MutService, S2: MutService<Input = S1::Input, Error = S1::Error>> MutService
    for CloningForkService<S1, S2>
where
    S1::Input: Clone,
{
    type Input = S1::Input;
    type Output = (S1::Output, S2::Output);
    type Error = S1::Error;
    fn process(&mut self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        Ok((
            self.first.process(input.clone())?,
            self.second.process(input)?,
        ))
    }
}
#[async_trait]
impl<S1: AsyncService, S2: AsyncService<Input = S1::Input, Error = S1::Error>> AsyncService
    for CloningForkService<S1, S2>
where
    S1::Input: Clone + Sync,
{
    type Input = S1::Input;
    type Output = (S1::Output, S2::Output);
    type Error = S1::Error;
    async fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        Ok((
            self.first.process(input.clone()).await?,
            self.second.process(input).await?,
        ))
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
impl<
        'a,
        I: 'a,
        E,
        S1: Service<Input = &'a I, Error = E>,
        S2: Service<Input = &'a I, Error = E>,
    > Service for RefForkService<I, S1, S2>
{
    type Input = &'a I;
    type Output = (S1::Output, S2::Output);
    type Error = E;
    fn process(&self, input: &'a I) -> Result<Self::Output, Self::Error> {
        Ok((self.first.process(input)?, self.second.process(input)?))
    }
}
impl<
        'a,
        I: 'a,
        E,
        S1: MutService<Input = &'a I, Error = E>,
        S2: MutService<Input = &'a I, Error = E>,
    > MutService for RefForkService<I, S1, S2>
{
    type Input = &'a I;
    type Output = (S1::Output, S2::Output);
    type Error = E;
    fn process(&mut self, input: &'a I) -> Result<Self::Output, Self::Error> {
        Ok((self.first.process(input)?, self.second.process(input)?))
    }
}

/// A [`Service`], [`MutService`], or [`AsyncService`], which encapsulates a `Service<(), Output = Option<T>>`, `MutService<(), Output = Option<T>>`, or `AsyncService<(), Output = Option<T>>`, blocking with the given idle function until a value is returned or the idle function returns an error.
///
/// When the underlying `Service<()>` returns None, the given idle function will be called.
/// The idle function will be called repeatedly, given the attempt number as input, until Some(T) is returned by the underlying service.
/// The idle function may return `Err(RetryError::Interrupted)` to return an error and avoid blocking forever.
///
/// See the [`idle`] module for some provided idle functions.
pub struct PollService<E, S, F>
where
    F: Fn(usize) -> Result<(), RetryError<E>>,
{
    service: S,
    idle: F,
    _phantom: PhantomData<fn(E)>,
}
impl<E, S, F> PollService<E, S, F>
where
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
impl<O, E, S, F> Service for PollService<E, S, F>
where
    S: Service<Input = (), Output = Option<O>, Error = E>,
    F: Fn(usize) -> Result<(), RetryError<E>>,
{
    type Input = ();
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

impl<O, E, S, F> MutService for PollService<E, S, F>
where
    S: MutService<Input = (), Output = Option<O>, Error = E>,
    F: Fn(usize) -> Result<(), RetryError<E>>,
{
    type Input = ();
    type Output = O;
    type Error = RetryError<S::Error>;
    fn process(&mut self, _: ()) -> Result<Self::Output, Self::Error> {
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
#[async_trait]
impl<O, E, S, F> AsyncService for PollService<E, S, F>
where
    O: Send + 'static,
    E: Send + 'static,
    S: AsyncService<Input = (), Output = Option<O>, Error = E> + Send + Sync,
    F: Fn(usize) -> Result<(), RetryError<E>> + Send + Sync,
{
    type Input = ();
    type Output = O;
    type Error = RetryError<S::Error>;
    async fn process(&self, _: ()) -> Result<Self::Output, Self::Error> {
        let mut attempt = 0;
        loop {
            match self.service.process(()).await {
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
/// This allows a [`RetryService`] to wrap a `Service`.
pub trait Retryable<I, E> {
    fn parse_retry(&self, err: E) -> Result<I, RetryError<E>>;
}

/// A [`Service`], [`MutService`], or [`AsyncService`], which encapsulates a [`Retryable`], blocking and retrying until a value is returned, an un-retryable error is encountered, or the idle function returns an `Err`.
///
/// When the underlying service's `Service::process` function returns an Err, it is passed to the given `Retryable`, which must return an `Ok(Input)` to retry or an `Err` to return immediately.
/// Between retries, the given `idle` function is called, given the attempt number as input, until `Ok(Output)` is returned by the underlying `Service` or `Err` is returned by the `Retryable` or `idle` function.
///
/// See the [`idle`] module for some provided idle functions.
pub struct RetryService<E, S, F>
where
    F: Fn(usize) -> Result<(), RetryError<E>>,
{
    service: S,
    idle: F,
}
impl<E, S, F> RetryService<E, S, F>
where
    F: Fn(usize) -> Result<(), RetryError<E>>,
{
    pub fn new(service: S, idle: F) -> Self {
        Self { service, idle }
    }
}
impl<S, F> Service for RetryService<S::Error, S, F>
where
    S: Service + Retryable<S::Input, S::Error>,
    F: Fn(usize) -> Result<(), RetryError<S::Error>>,
{
    type Input = S::Input;
    type Output = S::Output;
    type Error = RetryError<S::Error>;
    fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
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
#[async_trait]
impl<S, F> AsyncService for RetryService<S::Error, S, F>
where
    S: AsyncService + Retryable<S::Input, S::Error> + Send + Sync,
    F: Fn(usize) -> Result<(), RetryError<S::Error>> + Send + Sync,
{
    type Input = S::Input;
    type Output = S::Output;
    type Error = RetryError<S::Error>;
    async fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        let mut input = input;
        let mut attempt = 0;
        loop {
            match self.service.process(input).await {
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

/// A [`Service`], which encapsulates a [`Retryable`], producing `None` when a retryable event is encounterd.
///
/// This may be used to drive non-blocking duty-cycles in a service chain, continuously passing None through the service chain when no input is available.
pub struct RetryToOptionService<S> {
    service: S,
}
impl<S> RetryToOptionService<S> {
    pub fn new(service: S) -> Self {
        Self { service }
    }
}
impl<S> Service for RetryToOptionService<S>
where
    S: Service + Retryable<S::Input, S::Error>,
{
    type Input = S::Input;
    type Output = Option<S::Output>;
    type Error = RetryError<S::Error>;
    fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
        match self.service.process(input) {
            Ok(v) => Ok(Some(v)),
            Err(err) => match self.service.parse_retry(err) {
                Ok(_) => Ok(None),
                Err(err) => Err(err),
            },
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

/// Clone a [`Borrow<T>`] input, producing the cloned `T` value as output
pub struct CloneService<T: Clone, B: Borrow<T>> {
    _phantom: PhantomData<fn(T, B)>,
}
impl<T: Clone, B: Borrow<T>> CloneService<T, B> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
impl<T: Clone, B: Borrow<T>> Service for CloneService<T, B> {
    type Input = B;
    type Output = T;
    type Error = Infallible;
    fn process(&self, input: B) -> Result<Self::Output, Self::Error> {
        Ok(input.borrow().clone())
    }
}

/// Iterate over [`Vec<T>`] input, passing each `T` to an underlying [`Service`], returning `Vec<Output>`.
pub struct IntoIterService<S: Service> {
    service: S,
}
impl<S: Service> IntoIterService<S> {
    pub fn new(service: S) -> Self {
        Self { service }
    }
}
impl<T, S: Service<Input = T>> Service for IntoIterService<S> {
    type Input = Vec<T>;
    type Output = Vec<S::Output>;
    type Error = S::Error;
    fn process(&self, input: Vec<T>) -> Result<Self::Output, Self::Error> {
        let mut output = Vec::with_capacity(input.len());
        for e in input.into_iter() {
            output.push(self.service.process(e)?);
        }
        Ok(output)
    }
}

/// A [`Service`] that processes a [`Option<T>`] as input, processing with an underlying [`Service<Input = T>`]
/// when the input is [`Some`], producing [`Option<S::Output>`] as output.
///
/// When `None` is passed as input, `None` will be produced as output.
/// When `Some(T)` is passed as input, `Some(S::Output)` will be produced as output.
pub struct MaybeProcessService<S: Service> {
    service: S,
}
impl<S: Service> MaybeProcessService<S> {
    pub fn new(service: S) -> Self {
        Self { service }
    }
}
impl<T, S: Service<Input = T>> Service for MaybeProcessService<S> {
    type Input = Option<T>;
    type Output = Option<S::Output>;
    type Error = S::Error;
    fn process(&self, input: Option<T>) -> Result<Self::Output, Self::Error> {
        match input {
            None => Ok(None),
            Some(input) => Ok(Some(self.service.process(input)?)),
        }
    }
}

/// A [`Service`] that accepts a `FnOnce()` as input, which is passed to [`spawn()`], and produces a [`JoinHandle`] as output.
pub struct SpawnService<F> {
    _phantom: PhantomData<fn(F)>,
}
impl<F> SpawnService<F> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}
impl<T: Send + 'static, F: FnOnce() -> T + Send + 'static> Service for SpawnService<F> {
    type Input = F;
    type Output = JoinHandle<T>;
    type Error = Infallible;
    fn process(&self, input: F) -> Result<Self::Output, Self::Error> {
        Ok(spawn(input))
    }
}

/// A [`Service`] that will return `Ok(Input)` when the provided function returns true, or  or `Err(Stopped)` when the provided function returns false.
pub struct StopService<I, KeepRunningFunc: Fn() -> bool> {
    f: KeepRunningFunc,
    _phantom: PhantomData<fn(I)>,
}
impl<I, KeepRunningFunc: Fn() -> bool> StopService<I, KeepRunningFunc> {
    pub fn new(f: KeepRunningFunc) -> Self {
        Self {
            f,
            _phantom: PhantomData,
        }
    }
}
impl<I, KeepRunningFunc: Fn() -> bool> Service for StopService<I, KeepRunningFunc> {
    type Input = I;
    type Output = I;
    type Error = Stopped;
    fn process(&self, input: I) -> Result<Self::Output, Self::Error> {
        match (self.f)() {
            true => Ok(input),
            false => Err(Stopped),
        }
    }
}

/// A generic error that indicates stoppage
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stopped;
impl Display for Stopped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Stopped")
    }
}
impl Error for Stopped {}

/// A chain of [`Service`], [`MutService`], or [`AsyncService`] implementations, which is itself a single [`Service`], [`MutService`], or [`AsyncService`] that accepts the first service in the chain's input and produces the the last service in the chain's output.
/// When any service in the chain returns an `Err`, the chain will break early, encapsulate the error in a `ServiceChainError`, and return `Err(ServiceChainError)` immediately.
///
/// `ServiceChain::start(Service)` will start a service chain of [`Service`] impls.
/// `ServiceChain::start_mut(Service)` will start a service chain of [`MutService`] impls.
/// `ServiceChain::start_async(Service)` will start a service chain of [`AsyncService`] impls.
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
/// impl Service for AddService {
///     type Input = usize;
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
pub struct ServiceChain<P, S> {
    prev: P,
    service: S,
}
impl<'a, S: Service> ServiceChain<NoOpService<'a, S::Input>, S> {
    /// Start a new service chain using the given [`Service`] as the first service in the chain.
    /// This will return a [`ServiceChainBuilder`] that will allow you to link more [`Service`]s to finish building the [`ServiceChain`].
    pub fn start(service: S) -> ServiceChainBuilder<NoOpService<'a, S::Input>, S> {
        ServiceChainBuilder::start(service)
    }
}
impl<'a, S: MutService> ServiceChain<NoOpService<'a, S::Input>, S> {
    /// Start a new mutable service chain using the given [`MutService`] as the first service in the chain.
    /// This will return a [`ServiceChainBuilder`] that will allow you to link more [`MutService`]s to finish building the [`ServiceChain`].
    pub fn start_mut(service: S) -> MutServiceChainBuilder<NoOpService<'a, S::Input>, S> {
        MutServiceChainBuilder::start(service)
    }
}
impl<'a, S: AsyncService> ServiceChain<NoOpService<'a, S::Input>, S> {
    /// Start a new async service chain using the given [`AsyncService`] as the first service in the chain.
    /// This will return a [`ServiceChainBuilder`] that will allow you to link more [`AsyncService`]s to finish building the [`ServiceChain`].
    pub fn start_async(service: S) -> AsyncServiceChainBuilder<NoOpService<'a, S::Input>, S> {
        AsyncServiceChainBuilder::start(service)
    }
}
impl<P: Service, S: Service<Input = P::Output>> Service for ServiceChain<P, S>
where
    P::Error: Debug + 'static,
    S::Error: Debug + 'static,
{
    type Input = P::Input;
    type Output = S::Output;
    type Error = ServiceChainError<Box<dyn Debug>>;
    fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
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
impl<P: MutService, S: MutService<Input = P::Output>> MutService for ServiceChain<P, S>
where
    P::Error: Debug + 'static,
    S::Error: Debug + 'static,
{
    type Input = P::Input;
    type Output = S::Output;
    type Error = ServiceChainError<Box<dyn Debug>>;
    fn process(&mut self, input: Self::Input) -> Result<Self::Output, Self::Error> {
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
impl<P: AsyncService + Send + Sync, S: AsyncService<Input = P::Output> + Send + Sync> AsyncService
    for ServiceChain<P, S>
where
    P::Error: Debug + Send + 'static,
    S::Error: Debug + Send + 'static,
    P::Output: Send,
    S::Output: Send,
{
    type Input = P::Input;
    type Output = S::Output;
    type Error = ServiceChainError<Box<dyn Debug + Send>>;
    async fn process(&self, input: Self::Input) -> Result<Self::Output, Self::Error> {
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
pub struct ServiceChainBuilder<P: Service, S: Service<Input = P::Output>> {
    chain: ServiceChain<P, S>,
}
impl<'a, S: Service> ServiceChainBuilder<NoOpService<'a, S::Input>, S> {
    /// from ServiceChain::start()
    fn start(service: S) -> ServiceChainBuilder<NoOpService<'a, S::Input>, S> {
        ServiceChainBuilder {
            chain: ServiceChain {
                prev: NoOpService::new(),
                service,
            },
        }
    }
}
impl<P: Service, S: Service<Input = P::Output>> ServiceChainBuilder<P, S>
where
    P::Error: Debug + 'static,
    S::Error: Debug + 'static,
{
    /// Append another [`Service`] to the end of the service chain.
    pub fn next<NS: Service<Input = S::Output>>(
        self,
        service: NS,
    ) -> ServiceChainBuilder<ServiceChain<P, S>, NS> {
        ServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service,
            },
        }
    }
}
impl<P: Service, S: Service<Input = P::Output>> ServiceChainBuilder<P, S>
where
    P::Error: Debug + 'static,
    S::Error: Debug + 'static,
    S::Output: Clone,
{
    /// Fork the service chain to the given two services by cloning the input.
    pub fn fork_clone<
        E,
        NS1: Service<Input = S::Output, Error = E>,
        NS2: Service<Input = S::Output, Error = E>,
    >(
        self,
        first: NS1,
        second: NS2,
    ) -> ServiceChainBuilder<ServiceChain<P, S>, CloningForkService<NS1, NS2>> {
        ServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service: CloningForkService::new(first, second),
            },
        }
    }
}
impl<'a, P: Service + 'a, S: Service<Input = P::Output> + 'a> ServiceChainBuilder<P, S>
where
    P::Error: Debug + 'static,
    S::Error: Debug + 'static,
{
    /// End and return the resulting [`ServiceChain`].
    pub fn end(self) -> ServiceChain<P, S> {
        self.chain
    }
}

/// Returned by `ServiceChain::start_mut` to build a mut service chain.
/// Use the `next(self, MutService)` function to append more services to the [`ServiceChain`].
/// Use the `end(self)` function to finish building and return the resulting [`ServiceChain`].
pub struct MutServiceChainBuilder<P: MutService, S: MutService<Input = P::Output>> {
    chain: ServiceChain<P, S>,
}
impl<'a, S: MutService> MutServiceChainBuilder<NoOpService<'a, S::Input>, S> {
    /// from ServiceChain::start_mut()
    fn start(service: S) -> MutServiceChainBuilder<NoOpService<'a, S::Input>, S> {
        MutServiceChainBuilder {
            chain: ServiceChain {
                prev: NoOpService::new(),
                service: service,
            },
        }
    }
}
impl<P: MutService, S: MutService<Input = P::Output>> MutServiceChainBuilder<P, S>
where
    P::Error: Debug + 'static,
    S::Error: Debug + 'static,
{
    /// Append another [`MutService`] to the end of the service chain
    pub fn next<NS: MutService<Input = S::Output>>(
        self,
        service: NS,
    ) -> MutServiceChainBuilder<ServiceChain<P, S>, NS> {
        MutServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service,
            },
        }
    }
}
impl<P: MutService, S: MutService<Input = P::Output>> MutServiceChainBuilder<P, S>
where
    P::Error: Debug + 'static,
    S::Error: Debug + 'static,
    S::Output: Clone,
{
    /// Fork the service chain to the given two services by cloning the input.
    pub fn fork_clone<
        E,
        NS1: MutService<Input = S::Output, Error = E>,
        NS2: MutService<Input = S::Output, Error = E>,
    >(
        self,
        first: NS1,
        second: NS2,
    ) -> MutServiceChainBuilder<ServiceChain<P, S>, CloningForkService<NS1, NS2>> {
        MutServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service: CloningForkService::new(first, second),
            },
        }
    }
}
impl<'a, P: MutService + 'a, S: MutService<Input = P::Output> + 'a> MutServiceChainBuilder<P, S>
where
    P::Error: Debug + 'static,
    S::Error: Debug + 'static,
{
    /// End and return the resulting [`ServiceChain`].
    pub fn end(self) -> ServiceChain<P, S> {
        self.chain
    }
}

/// Returned by `ServiceChain::start_async` to build an async service chain.
/// Use the `next(self, AsyncService)` function to append more services to the [`ServiceChain`].
/// Use the `end(self)` function to finish building and return the resulting [`ServiceChain`].
pub struct AsyncServiceChainBuilder<P: AsyncService, S: AsyncService<Input = P::Output>> {
    chain: ServiceChain<P, S>,
}
impl<'a, S: AsyncService> AsyncServiceChainBuilder<NoOpService<'a, S::Input>, S> {
    /// from ServiceChain::start_async()
    fn start(service: S) -> AsyncServiceChainBuilder<NoOpService<'a, S::Input>, S> {
        AsyncServiceChainBuilder {
            chain: ServiceChain {
                prev: NoOpService::new(),
                service,
            },
        }
    }
}
impl<P: AsyncService + Send + Sync, S: AsyncService<Input = P::Output> + Send + Sync>
    AsyncServiceChainBuilder<P, S>
where
    P::Error: Debug + Send,
    S::Error: Debug + Send,
    P::Output: Send,
    S::Output: Send,
{
    /// Append another [`AsyncService`] to the end of the service chain.
    pub fn next<NS: AsyncService<Input = S::Output>>(
        self,
        service: NS,
    ) -> AsyncServiceChainBuilder<ServiceChain<P, S>, NS> {
        AsyncServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service: service,
            },
        }
    }
}
impl<P: AsyncService + Send + Sync, S: AsyncService<Input = P::Output> + Send + Sync>
    AsyncServiceChainBuilder<P, S>
where
    P::Error: Debug + Send,
    S::Error: Debug + Send,
    P::Output: Send,
    S::Output: Send + Clone + Sync,
{
    /// Fork the service chain to the given two services by cloning the input.
    pub fn fork_clone<
        NS1: AsyncService<Input = S::Output> + Send + Sync,
        NS2: AsyncService<Input = S::Output, Error = NS1::Error> + Send + Sync,
    >(
        self,
        first: NS1,
        second: NS2,
    ) -> AsyncServiceChainBuilder<ServiceChain<P, S>, CloningForkService<NS1, NS2>>
    where
        <NS1 as AsyncService>::Output: Send + Sync,
        <NS2 as AsyncService>::Output: Send + Sync,
    {
        AsyncServiceChainBuilder {
            chain: ServiceChain {
                prev: self.chain,
                service: CloningForkService::new(first, second),
            },
        }
    }
}
impl<
        'a,
        P: AsyncService + Send + Sync + 'a,
        S: AsyncService<Input = P::Output> + Send + Sync + 'a,
    > AsyncServiceChainBuilder<P, S>
where
    P::Error: Send + Debug + 'static,
    S::Error: Send + Debug + 'static,
    P::Output: Send + 'a,
    S::Output: Send + 'a,
{
    /// End and return the resulting [`ServiceChain`].
    pub fn end(self) -> ServiceChain<P, S> {
        self.chain
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::*;

    struct AddService {
        n: usize,
    }
    impl AddService {
        pub fn new(n: usize) -> Self {
            Self { n }
        }
    }
    impl Service for AddService {
        type Input = usize;
        type Output = usize;
        type Error = Infallible;
        fn process(&self, input: usize) -> Result<usize, Infallible> {
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
    impl MutService for AppendService {
        type Input = usize;
        type Output = usize;
        type Error = Infallible;
        fn process(&mut self, input: usize) -> Result<usize, Infallible> {
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
        let result = block_on(chain.process(100)).unwrap();
        assert_eq!(107, result);
    }
}
