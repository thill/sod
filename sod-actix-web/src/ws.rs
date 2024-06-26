//! # websockets
//!
//! An [`actix_web`] [`Handler`] that instantiates [`MutService`] impls to handle individual sessions.
//!
//! ## Session Factory
//!
//! The [`WsSessionFactory`] is the entry point to this module. It is used to create session services
//! as connections are established by client, and acts as a [`actix_web`] [`Handler`].
//!
//! The [`WsSessionFactory`] encapsulates 2 functions to be defined be the user.
//! - `F: Fn(&HttpRequest) -> Result<S, Error> + 'static` - The factory that produces session services,
//!   which can produce either an Ok([`Service`]), Ok([`MutService`]), or Err([`Error`]).
//! - `E: Fn(&mut S, S::Error)-> Result<(), S::Error>+ Unpin + 'static` - The error handler, which
//!   is used as a callback to handle errors returned by your service implementation.
//!
//! Actix Wiring:
//! ```rust,compile_fail
//! web::resource("/echo").route(web::get().to(WsSessionFactory::new(
//!   |_req| Ok(EchoService),
//!   |_service, err| println!("ERROR: {err}"),
//! ))),
//! ```
//!
//! ## Session Services
//!
//! The underlying session [`MutService`] impls that are produced by the session service factory
//! must accept a [`WsSessionEvent`] as input and produce a [`Option<WsMessage>`] as output.
//!
//! - [`WsSessionEvent`] input alerts the session of session lifecycle events and received messages.
//! - [`Option<WsMessage>`] output can optionally send response payloads to a session.
//!
//! ## Error Handling
//!
//! The [`WsSessionFactory`] requires an `Fn(&mut S, S::Error) -> Result<(), S::Error>` error handler
//! function to be provided by the user where `S: MutService` or `S: Service`. Since actix uses an
//! asynchronous thread-pool behind the scenes to handle websocket requests, [`Service`] [`Err`] results
//! are not able to bubble up outside of the underlying [`StreamHandler`].
//!
//! Instead of make assumptions about how a user wants to handle errors returned by a service, that is
//! left entirely up to the user via the error handler. When the error handler returns [`Ok(())`], no action
//! will be taken against the underlying session. When the [`Err`] is returned by the error handler, the
//! session will be closed.
//!
//! A common error handler impl is to log the error and close the session:
//! ```rust,compile_fail
//! |_, err| {
//!     log::error!("Session Error: {err}");
//!     Err(err)
//! }
//! ```
//!
//! ## WsSendService
//!
//! To produce messages to a session outside of input event handling, use the [`WsSendService`]
//! provided by the initial [`WsSessionEvent::Started`] event. You may take ownership of the
//! [`WsSendService`] to asynchronously produce messages to the service outside of the session's
//! input handler service, which will return a [`SendError`] once the session has been closed/shutdown.
//!
//! ## Ping/Pong
//!
//! Pong replies are automatically sent by this framework, so you may ignore Ping requests for the
//! purpose of Ping/Pong responses.
//!
//! ## Echo Example
//! ```rust,no_run
//! use std::convert::Infallible;
//! use actix_web::{web, App, HttpServer};
//! use sod::MutService;
//! use sod_actix_web::ws::{WsMessage, WsSessionEvent, WsSessionFactory};
//!
//! #[actix_web::main]
//! async fn main() -> std::io::Result<()> {
//!     struct EchoService;
//!     impl MutService for EchoService {
//!         type Input = WsSessionEvent;
//!         type Output = Option<WsMessage>;
//!         type Error = Infallible;
//!         fn process(&mut self, event: WsSessionEvent) -> Result<Self::Output, Self::Error> {
//!             Ok(match event {
//!                 WsSessionEvent::Started(_) => {
//!                     Some(WsMessage::Text("Welcome to EchoServer!".to_owned()))
//!                 }
//!                 WsSessionEvent::Message(message) => match message {
//!                     WsMessage::Binary(data) => Some(WsMessage::Binary(data)),
//!                     WsMessage::Text(text) => Some(WsMessage::Text(text)),
//!                     _ => None, // note: pongs are sent automatically
//!                 },
//!                 _ => None,
//!             })
//!         }
//!     }
//!
//!     HttpServer::new(|| {
//!         App::new().service(
//!             web::resource("/echo").route(web::get().to(WsSessionFactory::new(
//!                 |_| Ok(EchoService),
//!                 |_, err| {
//!                     println!("ERROR: {err}");
//!                     Err(err)
//!                 },
//!             ))),
//!         )
//!     })
//!     .bind(("127.0.0.1", 8080))?
//!     .run()
//!     .await
//! }
//! ```

use std::{marker::PhantomData, sync::Arc};

use actix::{prelude::SendError, Actor, AsyncContext, Recipient, StreamHandler};
use actix_web::{web, Error, Handler, HttpRequest, HttpResponse};
use actix_web_actors::ws::{self, CloseCode, CloseReason};
use sod::{MutService, Service};

use crate::sealed::SettableFuture;

/// The entry point to this module. It is used to create session services as connections are
/// established by client, and acts as a [`actix_web`] [`Handler`].
///
/// See the this module's documentation for details and examples.
pub struct WsSessionFactory<O, S, F, E>
where
    O: Into<Option<WsMessage>> + Unpin + 'static,
    S: MutService<Input = WsSessionEvent, Output = O> + Unpin + 'static,
    F: Fn(&HttpRequest) -> Result<S, Error> + 'static,
    E: Fn(&mut S, S::Error) -> Result<(), S::Error> + Unpin + 'static,
{
    factory: Arc<F>,
    error_handler: Arc<E>,
    _phantom: PhantomData<fn(O, S)>,
}
impl<O, S, F, E> WsSessionFactory<O, S, F, E>
where
    O: Into<Option<WsMessage>> + Unpin + 'static,
    S: MutService<Input = WsSessionEvent, Output = O> + Unpin + 'static,
    F: Fn(&HttpRequest) -> Result<S, Error> + 'static,
    E: Fn(&mut S, S::Error) -> Result<(), S::Error> + Unpin + 'static,
{
    /// Encapsulate the given [`sod::Service`] or [`sod::MutService`] factory and error handler,
    /// making the service factory compatible with a native [`actix_web::Handler`] and underlying
    /// session services compabile with a native [`actix::Handler`] and [`actix::StreamHandler`].
    pub fn new(factory: F, error_handler: E) -> Self {
        Self {
            factory: Arc::new(factory),
            error_handler: Arc::new(error_handler),
            _phantom: PhantomData,
        }
    }
}
impl<O, S, F, E> Clone for WsSessionFactory<O, S, F, E>
where
    O: Into<Option<WsMessage>> + Unpin + 'static,
    S: MutService<Input = WsSessionEvent, Output = O> + Unpin + 'static,
    F: Fn(&HttpRequest) -> Result<S, Error> + 'static,
    E: Fn(&mut S, S::Error) -> Result<(), S::Error> + Unpin + 'static,
{
    fn clone(&self) -> Self {
        Self {
            factory: Arc::clone(&self.factory),
            error_handler: Arc::clone(&self.error_handler),
            _phantom: PhantomData,
        }
    }
}
impl<O, S, F, E> Handler<(HttpRequest, web::Payload)> for WsSessionFactory<O, S, F, E>
where
    O: Into<Option<WsMessage>> + Unpin + 'static,
    S: MutService<Input = WsSessionEvent, Output = O> + Unpin + 'static,
    F: Fn(&HttpRequest) -> Result<S, Error> + 'static,
    E: Fn(&mut S, S::Error) -> Result<(), S::Error> + Unpin + 'static,
{
    type Output = Result<HttpResponse, Error>;
    type Future = SettableFuture<Result<HttpResponse, Error>>;
    fn call(&self, (req, stream): (HttpRequest, web::Payload)) -> Self::Future {
        let result = match (self.factory)(&req) {
            Ok(service) => ws::start(
                WsActor::new(service, Arc::clone(&self.error_handler)),
                &req,
                stream,
            ),
            Err(err) => Err(err),
        };
        SettableFuture::new().set(result)
    }
}

/// Provided by [`WsSessionEvent::Started`], which may be used to asychronously send [`WsMessage`]
/// to the underlying session outside of the session handler service.
///
/// This impls [`Service<WsMessage>`], which can easily be chained to other [`Service`] impls to
/// dispatch events to the session from other pipelines.
#[derive(Debug)]
pub struct WsSendService {
    recipient: Recipient<WsMessage>,
}
impl WsSendService {
    fn new(recipient: Recipient<WsMessage>) -> Self {
        Self { recipient }
    }
}
impl Service for WsSendService {
    type Input = WsMessage;
    type Output = ();
    type Error = SendError<WsMessage>;
    fn process(&self, msg: WsMessage) -> Result<Self::Output, Self::Error> {
        self.recipient.try_send(msg)
    }
}

/// The input to a session [`MutService`] or [`Service`] impl.
///
/// - [`WsSessionEvent::Started`] - always called first, providing a [`WsSendService`].
/// - [`WsSessionEvent::Message`] - called when a message is received from the client.
/// - [`WsSessionEvent::Error`] - called when the [`actix_web`] [`StreamHandler`] reports a session error.
/// - [`WsSessionEvent::Stopped`] - called as the final action before the session is dropped.
#[derive(Debug)]
pub enum WsSessionEvent {
    Started(WsSendService),
    Message(WsMessage),
    Error(ws::ProtocolError),
    Stopped,
}
impl WsSessionEvent {
    fn from_actix_result(result: Result<ws::Message, ws::ProtocolError>) -> Option<Self> {
        match result {
            Ok(message) => match WsMessage::from_actix_ws_message(message) {
                None => None,
                Some(message) => Some(WsSessionEvent::Message(message)),
            },
            Err(err) => Some(Self::Error(err)),
        }
    }
}

/// A WebSocket message, which is used to receive or send messages from a session.
#[derive(Debug)]
pub enum WsMessage {
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Binary(Vec<u8>),
    Text(String),
    Close(Option<CloseReason>),
}
impl WsMessage {
    fn from_actix_ws_message(src: ws::Message) -> Option<Self> {
        match src {
            ws::Message::Binary(data) => Some(Self::Binary(data.into())),
            ws::Message::Ping(data) => Some(Self::Ping(data.into())),
            ws::Message::Pong(data) => Some(Self::Pong(data.into())),
            ws::Message::Close(reason) => Some(Self::Close(reason)),
            ws::Message::Text(text) => Some(Self::Text(text.into())),
            ws::Message::Continuation(_) => None,
            ws::Message::Nop => None,
        }
    }
}
impl From<WsMessage> for ws::Message {
    fn from(value: WsMessage) -> Self {
        match value {
            WsMessage::Ping(data) => ws::Message::Ping(data.into()),
            WsMessage::Pong(data) => ws::Message::Pong(data.into()),
            WsMessage::Binary(data) => ws::Message::Binary(data.into()),
            WsMessage::Text(text) => ws::Message::Text(text.into()),
            WsMessage::Close(reason) => ws::Message::Close(reason),
        }
    }
}
impl actix::Message for WsMessage {
    type Result = ();
}

/// Internal [`Actor`] that dispatches to the underlying [`MutService`].
struct WsActor<O, S, E>
where
    O: Unpin + 'static,
    S: MutService<Input = WsSessionEvent, Output = O> + Unpin + 'static,
    E: Fn(&mut S, S::Error) -> Result<(), S::Error> + Unpin + 'static,
{
    service: S,
    error_handler: Arc<E>,
    _phantom: PhantomData<fn(O)>,
}
impl<O, S, E> WsActor<O, S, E>
where
    O: Into<Option<WsMessage>> + Unpin + 'static,
    S: MutService<Input = WsSessionEvent, Output = O> + Unpin + 'static,
    E: Fn(&mut S, S::Error) -> Result<(), S::Error> + Unpin + 'static,
{
    fn new(service: S, error_handler: Arc<E>) -> Self {
        Self {
            service,
            error_handler,
            _phantom: PhantomData,
        }
    }
}
impl<O, S, E> Actor for WsActor<O, S, E>
where
    O: Into<Option<WsMessage>> + Unpin + 'static,
    S: MutService<Input = WsSessionEvent, Output = O> + Unpin + 'static,
    E: Fn(&mut S, S::Error) -> Result<(), S::Error> + Unpin + 'static,
{
    type Context = ws::WebsocketContext<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        match self
            .service
            .process(WsSessionEvent::Started(WsSendService::new(
                ctx.address().recipient(),
            ))) {
            Ok(send) => {
                if let Some(send) = send.into() {
                    ctx.write_raw(send.into());
                }
            }
            Err(err) => {
                if let Err(_) = (self.error_handler)(&mut self.service, err) {
                    ctx.close(Some(CloseReason::from(CloseCode::Error)));
                }
            }
        }
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        if let Err(err) = self.service.process(WsSessionEvent::Stopped) {
            (self.error_handler)(&mut self.service, err).ok();
        }
    }
}
impl<O, S, E> actix::Handler<WsMessage> for WsActor<O, S, E>
where
    O: Into<Option<WsMessage>> + Unpin + 'static,
    S: MutService<Input = WsSessionEvent, Output = O> + Unpin + 'static,
    E: Fn(&mut S, S::Error) -> Result<(), S::Error> + Unpin + 'static,
{
    type Result = ();
    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            WsMessage::Ping(data) => ctx.ping(&data),
            WsMessage::Pong(data) => ctx.pong(&data),
            WsMessage::Binary(data) => ctx.binary(data),
            WsMessage::Text(text) => ctx.text(text),
            WsMessage::Close(reason) => ctx.close(reason),
        }
    }
}
impl<O, S, E> StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsActor<O, S, E>
where
    O: Into<Option<WsMessage>> + Unpin + 'static,
    S: MutService<Input = WsSessionEvent, Output = O> + Unpin + 'static,
    E: Fn(&mut S, S::Error) -> Result<(), S::Error> + Unpin + 'static,
{
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        if let Some(msg) = WsSessionEvent::from_actix_result(msg) {
            if let WsSessionEvent::Message(WsMessage::Ping(data)) = &msg {
                ctx.pong(data);
            }
            match self.service.process(msg) {
                Ok(send) => {
                    if let Some(send) = send.into() {
                        ctx.write_raw(send.into());
                    }
                }
                Err(err) => {
                    if let Err(_) = (self.error_handler)(&mut self.service, err) {
                        ctx.close(Some(CloseCode::Error.into()));
                    }
                }
            }
        }
    }
}
