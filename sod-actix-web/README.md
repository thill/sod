# sod-actix-web

This crate provides `sod::Service` abstractions around `actix_web` via `Handler` implementations.

# Handlers

The `ServiceHandler` acts as an `actix_web` `Handler`, dispatching requests to an underlying
`sod::AsyncService` or `sod::Service` implementation.

## Service I/O

The input to the underlying `AsyncService` is directly compatible with the native `FromRequest` trait
in `actix_web`. As such, a tuple of `FromRequest` impls can be handled as input for an `AsyncService`.

The output from the underlying `AsyncService` must implement the native `Responder` trait from `actix_web`.
This means that all output type from the service should be compatible with all output types from `actix_web`.
This should include a simple `String` or full `actix_web::HttpResponse`.

## Greet Server Example

The following example mirrors the default `actix_web` greeter example, except it uses the service abstraction
provided by this library:

```rust
use actix_web::{web, App, HttpServer};
use sod::Service;
use sod_actix_web::ServiceHandler;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    struct GreetService;
    impl Service for GreetService {
        type Input = web::Path<String>;
        type Output = String;
        type Error = std::convert::Infallible;
        fn process(&self, name: web::Path<String>) -> Result<Self::Output, Self::Error> {
            Ok(format!("Hello {name}!"))
        }
    }

    HttpServer::new(|| {
        App::new().service(
            web::resource("/greet/{name}").route(web::get().to(ServiceHandler::new(GreetService.into_async()))),
        )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
```

## Math Server Example

The following example is slightly more advanced, demonstrating how `AsyncService` and a tuple of inputs may be used:

```rust
use std::{io::Error, io::ErrorKind};
use actix_web::{web, App, HttpServer};
use serde_derive::Deserialize;
use sod::{async_trait, AsyncService};
use sod_actix_web::ServiceHandler;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    #[derive(Debug, Deserialize)]
    pub struct MathParams {
        a: i64,
        b: i64,
    }

    struct MathService;
    #[async_trait]
    impl AsyncService for MathService {
        type Input = (web::Path<String>, web::Query<MathParams>);
        type Output = String;
        type Error = Error;
        async fn process(
            &self,
            (func, params): (web::Path<String>, web::Query<MathParams>),
        ) -> Result<Self::Output, Self::Error> {
            let value = match func.as_str() {
                "add" => params.a + params.b,
                "sub" => params.a - params.b,
                "mul" => params.a * params.b,
                "div" => params.a / params.b,
                _ => return Err(Error::new(ErrorKind::Other, "invalid func")),
            };
            Ok(format!("{value}"))
        }
    }

    HttpServer::new(|| {
        App::new().service(
            web::resource("/math/{func}").route(web::get().to(ServiceHandler::new(MathService))),
        )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
```

# websockets

An `actix_web` `Handler` that instantiates `MutService` impls to handle individual sessions.

## Session Factory

The `WsSessionFactory` is the entry point to this module. It is used to create session services
as connections are established by client, and acts as a `actix_web` `Handler`.

The `WsSessionFactory` encapsulates 2 functions to be defined be the user.

- `F: Fn(&HttpRequest) -> Result<S, Error> + 'static` - The factory that produces session services,
  which can produce either an Ok(`Service`), Ok(`MutService`), or Err(`Error`).
- `E: Fn(&mut S, S::Error)-> Result<(), S::Error>+ Unpin + 'static` - The error handler, which
  is used as a callback to handle errors returned by your service implementation.

Actix Wiring:

```rust
web::resource("/echo").route(web::get().to(WsSessionFactory::new(
  |_req| Ok(EchoService),
  |_service, err| println!("ERROR: {err}"),
))),
```

## Session Services

The underlying session `MutService` impls that are produced by the session service factory
must accept a `WsSessionEvent` as input and produce a `Option<WsMessage>` as output.

- `WsSessionEvent` input alerts the session of session lifecycle events and received messages.
- `Option<WsMessage>` output can optionally send response payloads to a session.

## Error Handling

The `WsSessionFactory` requires an `Fn(&mut S, S::Error) -> Result<(), S::Error>` error handler
function to be provided by the user where `S: MutService` or `S: Service`. Since actix uses an
asynchronous thread-pool behind the scenes to handle websocket requests, `Service` `Err` results
are not able to bubble up outside of the underlying `StreamHandler`.

Instead of make assumptions about how a user wants to handle errors returned by a service, that is
left entirely up to the user via the error handler. When the error handler returns `Ok(())`, no action
will be taken against the underlying session. When the `Err` is returned by the error handler, the
session will be closed.

A common error handler impl is to log the error and close the session:

```rust
|_, err| {
    log::error!("Session Error: {err}");
    Err(err)
}
```

## WsSendService

To produce messages to a session outside of input event handling, use the `WsSendService`
provided by the initial `WsSessionEvent::Started` event. You may take ownership of the
`WsSendService` to asynchronously produce messages to the service outside of the session's
input handler service, which will return a `SendError` once the session has been closed/shutdown.

## Ping/Pong

Pong replies are automatically sent by this framework, so you may ignore Ping requests for the
purpose of Ping/Pong responses.

## Echo Example

```rust
use std::convert::Infallible;
use actix_web::{web, App, HttpServer};
use sod::MutService;
use sod_actix_web::ws::{WsMessage, WsSessionEvent, WsSessionFactory};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    struct EchoService;
    impl MutService<WsSessionEvent> for EchoService {
        type Output = Option<WsMessage>;
        type Error = Infallible;
        fn process(&mut self, event: WsSessionEvent) -> Result<Self::Output, Self::Error> {
            Ok(match event {
                WsSessionEvent::Started(_) => {
                    Some(WsMessage::Text("Welcome to EchoServer!".to_owned()))
                }
                WsSessionEvent::Message(message) => match message {
                    WsMessage::Binary(data) => Some(WsMessage::Binary(data)),
                    WsMessage::Text(text) => Some(WsMessage::Text(text)),
                    _ => None, // note: pongs are sent automatically
                },
                _ => None,
            })
        }
    }

    HttpServer::new(|| {
        App::new().service(
            web::resource("/echo").route(web::get().to(WsSessionFactory::new(
                |_| Ok(EchoService),
                |_, err| {
                    println!("ERROR: {err}");
                    Err(err)
                },
            ))),
        )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
```
