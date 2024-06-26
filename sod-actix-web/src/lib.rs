//! # sod-actix-web
//!
//! This crate provides [`sod::Service`] abstractions around [`actix_web`] via [`Handler`] implementations.
//!
//! # Handlers
//!
//! The [`ServiceHandler`] acts as an [`actix_web`] [`Handler`], dispatching requests to an underlying
//! [`sod::AsyncService`] or [`sod::Service`] implementation.
//!
//! ## Service I/O
//!
//! The input to the underlying [`AsyncService`] is directly compatible with the native [`FromRequest`] trait
//! in [`actix_web`]. As such, a tuple of [`FromRequest`] impls can be handled as input for an [`AsyncService`].
//!
//! The output from the underlying [`AsyncService`] must implement the native [`Responder`] trait from [`actix_web`].
//! This means that all output type from the service should be compatible with all output types from [`actix_web`].
//! This should include a simple [`String`] or full [`actix_web::HttpResponse`].
//!
//! ## Greet Server Example
//!
//! The following example mirrors the default [`actix_web`] greeter example, except it uses the service abstraction
//! provided by this library:
//!
//! ```rust,no_run
//! use actix_web::{web, App, HttpServer};
//! use sod::Service;
//! use sod_actix_web::ServiceHandler;
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     struct GreetService;
//!     impl Service for GreetService {
//!         type Input = web::Path<String>;
//!         type Output = String;
//!         type Error = std::convert::Infallible;
//!         fn process(&self, name: web::Path<String>) -> Result<Self::Output, Self::Error> {
//!             Ok(format!("Hello {name}!"))
//!         }
//!     }
//!
//!     HttpServer::new(|| {
//!         App::new().service(
//!             web::resource("/greet/{name}").route(web::get().to(ServiceHandler::new(GreetService.into_async()))),
//!         )
//!     })
//!     .bind(("127.0.0.1", 8080))?
//!     .run()
//!     .await
//! }
//! ```
//!
//! ## Math Server Example
//!
//! The following example is slightly more advanced, demonstrating how [`AsyncService`] and a tuple of inputs may be used:
//!
//! ```rust,no_run
//! use std::{io::Error, io::ErrorKind};
//! use actix_web::{web, App, HttpServer};
//! use serde_derive::Deserialize;
//! use sod::{async_trait, AsyncService};
//! use sod_actix_web::ServiceHandler;
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     #[derive(Debug, Deserialize)]
//!     pub struct MathParams {
//!         a: i64,
//!         b: i64,
//!     }
//!
//!     struct MathService;
//!     #[async_trait]
//!     impl AsyncService for MathService {
//!         type Input = (web::Path<String>, web::Query<MathParams>);
//!         type Output = String;
//!         type Error = Error;
//!         async fn process(
//!             &self,
//!             (func, params): (web::Path<String>, web::Query<MathParams>),
//!         ) -> Result<Self::Output, Self::Error> {
//!             let value = match func.as_str() {
//!                 "add" => params.a + params.b,
//!                 "sub" => params.a - params.b,
//!                 "mul" => params.a * params.b,
//!                 "div" => params.a / params.b,
//!                 _ => return Err(Error::new(ErrorKind::Other, "invalid func")),
//!             };
//!             Ok(format!("{value}"))
//!         }
//!     }
//!
//!     HttpServer::new(|| {
//!         App::new().service(
//!             web::resource("/math/{func}").route(web::get().to(ServiceHandler::new(MathService))),
//!         )
//!     })
//!     .bind(("127.0.0.1", 8080))?
//!     .run()
//!     .await
//! }
//! ```
//!
//! # WebSockets
//!
//! WebSocket [`sod::Service`] abstractions are provided in the [`ws`] module.

use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc};

use actix_web::{FromRequest, Handler, Responder, ResponseError};
use sod::AsyncService;

mod sealed;
pub mod ws;

/// The highest level abstraction provided by this library. It is used to encapsulate underlying [`sod::Service`]
/// impls with an [`actix_web`] [`Handler`] that can be natively wired into an Actix [`actix_web::App`].
///
/// Input tuples of [`FromRequest`] and outputs of [`Responder`] the responder trait make this directly compatible
/// with the native Actix request and response types.
///
/// See the this module's documentation for details and examples.
pub struct ServiceHandler<Args, S>
where
    Args: FromRequest + 'static,
    S: AsyncService<Input = Args> + 'static,
    S::Output: Responder + 'static,
    S::Error: ResponseError + 'static,
{
    service: Arc<S>,
    _phantom: PhantomData<fn(Args)>,
}
impl<Args, S> ServiceHandler<Args, S>
where
    Args: FromRequest + 'static,
    S: AsyncService<Input = Args> + 'static,
    S::Output: Responder + 'static,
    S::Error: ResponseError + 'static,
{
    /// Encapsulate the given [`sod::AsyncService`] or [`sod::Service`] to be used as an [`actix_web::Handler`]
    pub fn new(service: S) -> Self {
        Self {
            service: Arc::new(service),
            _phantom: PhantomData,
        }
    }
}
impl<Args, S> Clone for ServiceHandler<Args, S>
where
    Args: FromRequest + 'static,
    S: AsyncService<Input = Args> + 'static,
    S::Output: Responder + 'static,
    S::Error: ResponseError + 'static,
{
    fn clone(&self) -> Self {
        Self {
            service: Arc::clone(&self.service),
            _phantom: PhantomData,
        }
    }
}
impl<Args, S> Handler<Args> for ServiceHandler<Args, S>
where
    Args: FromRequest + Send + 'static,
    S: AsyncService<Input = Args> + Send + Sync + 'static,
    S::Output: Responder + Send + 'static,
    S::Error: ResponseError + Send + 'static,
{
    type Output = Result<S::Output, S::Error>;
    type Future = Pin<Box<dyn Future<Output = Result<S::Output, S::Error>> + Send>>;
    fn call(&self, args: Args) -> Self::Future {
        let service = Arc::clone(&self.service);
        Box::pin(async move { service.process(args).await })
    }
}
