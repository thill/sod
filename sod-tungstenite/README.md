# sod-tungstenite

[`sod::Service`](http://github.com/thill/sod) implementations to interact with [`tungstenite`](https://crates.io/crates/tungstenite) websockets.

## Service Impls

All Services are `Retryable` and are able to be blocking or non-blocking.

- `WsSession` is a `MutService` that wraps a `tungstenite::WebSocket`, accepting `WsSessionEvent` to send or receive messages. `WsSession::into_split` can split a `WsSession` into a `WsReader`, `WsWriter`, and `WsFlusher`.
- `WsReader` is a `Service` that wraps a `Mutex<tungstenite::WebSocket>`, accepting a `()` as input and producing `tungstenite::Message` as output.
- `WsWriter` is a `Service` that wraps a `Mutex<tungstenite::WebSocket>`, accepting a `tungstenite::Message` as input.
- `WsFlusher` is a `Service` that wraps a `Mutex<tungstenite::WebSocket>`, accepting a `()` as input.
- `WsServer` is a `Service` that that listens on a TCP port, accepting a `()` as input and producing a `WsSession` as output.

## Features

- `native-tls` to enable Native TLS
- `__rustls-tls` to enable Rustls TLS

## Blocking Example

```rust
use sod::{idle::backoff, MaybeProcessService, MutService, RetryService, Service, ServiceChain};
use sod_tungstenite::{UninitializedWsSession, WsServer, WsSession, WsSessionEvent};
use std::{sync::atomic::Ordering, thread::spawn};
use tungstenite::{http::StatusCode, Message};
use url::Url;

// server session logic to add `"pong: "` in front of text payload
struct PongService;
impl Service for PongService {
    type Input = Message;
    type Output = Option<Message>;
    type Error = ();
    fn process(&self, input: Message) -> Result<Self::Output, Self::Error> {
        match input {
            Message::Text(text) => Ok(Some(Message::Text(format!("pong: {text}")))),
            _ => Ok(None),
        }
    }
}

// wires session logic and spawns in new thread
struct SessionSpawner;
impl Service for SessionSpawner {
    type Input = UninitializedWsSession;
    type Output = ();
    type Error = ();
    fn process(&self, input: UninitializedWsSession) -> Result<Self::Output, Self::Error> {
        spawn(|| {
            let (r, w, f) = input.handshake().unwrap().into_split();
            let chain = ServiceChain::start(r)
                .next(PongService)
                .next(MaybeProcessService::new(w))
                .next(MaybeProcessService::new(f))
                .end();
            sod::thread::spawn_loop(chain, |err| {
                println!("Session: {err:?}");
                Err(err) // stop thread on error
            });
        });
        Ok(())
    }
}

// start a blocking server that creates blocking sessions
let server = WsServer::bind("127.0.0.1:48490").unwrap();

// spawn a thread to start accepting new server sessions
let handle = sod::thread::spawn_loop(
    ServiceChain::start(server).next(SessionSpawner).end(),
    |err| {
        println!("Server: {err:?}");
        Err(err) // stop thread on error
    },
);

// connect a client to the server
let (mut client, _) =
    WsSession::connect(Url::parse("ws://127.0.0.1:48490/socket").unwrap()).unwrap();

// client writes `"hello world"` payload
client
    .process(WsSessionEvent::WriteMessage(Message::Text(
        "hello world!".to_owned(),
    )))
    .unwrap();

// client receives `"pong: hello world"` payload
println!(
    "Received: {:?}",
    client.process(WsSessionEvent::ReadMessage).unwrap()
);

// join until server crashes
handle.join().unwrap();
```

## Non-Blocking Example

```rust
use sod::{idle::backoff, MaybeProcessService, MutService, RetryService, Service, ServiceChain};
use sod_tungstenite::{UninitializedWsSession, WsServer, WsSession, WsSessionEvent};
use std::{sync::atomic::Ordering, thread::spawn};
use tungstenite::{http::StatusCode, Message};
use url::Url;

// server session logic to add `"pong: "` in front of text payload
struct PongService;
impl Service for PongService {
    type Input = Message;
    type Output = Option<Message>;
    type Error = ();
    fn process(&self, input: Message) -> Result<Self::Output, Self::Error> {
        match input {
            Message::Text(text) => Ok(Some(Message::Text(format!("pong: {text}")))),
            _ => Ok(None),
        }
    }
}

// wires session logic and spawns in new thread
struct SessionSpawner;
impl Service for SessionSpawner {
    type Input = UninitializedWsSession;
    type Output = ();
    type Error = ();
    fn process(&self, input: UninitializedWsSession) -> Result<Self::Output, Self::Error> {
        spawn(|| {
            let (r, w, f) = input.handshake().unwrap().into_split();
            let chain = ServiceChain::start(RetryService::new(r, backoff))
                .next(PongService)
                .next(MaybeProcessService::new(RetryService::new(w, backoff)))
                .next(MaybeProcessService::new(f))
                .end();
            sod::thread::spawn_loop(chain, |err| {
                println!("Session: {err:?}");
                Err(err) // stop thread on error
            });
        });
        Ok(())
    }
}

// start a non-blocking server that creates non-blocking sessions
let server = WsServer::bind("127.0.0.1:48490")
    .unwrap()
    .with_nonblocking_sessions(true)
    .with_nonblocking_server(true)
    .unwrap();

// spawn a thread to start accepting new server sessions
let handle = sod::thread::spawn_loop(
    ServiceChain::start(RetryService::new(server, backoff))
        .next(SessionSpawner)
        .end(),
    |err| {
        println!("Server: {err:?}");
        Err(err) // stop thread on error
    },
);

// connect a client to the server
let (mut client, response) =
    WsSession::connect(Url::parse("ws://127.0.0.1:48490/socket").unwrap()).unwrap();
assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);

// client writes `"hello world"` payload
client
    .process(WsSessionEvent::WriteMessage(Message::Text(
        "hello world!".to_owned(),
    )))
    .unwrap();

// client receives `"pong: hello world"` payload
assert_eq!(
    client.process(WsSessionEvent::ReadMessage).unwrap(),
    Some(Message::Text("pong: hello world!".to_owned()))
);

// stop the server
sod::idle::KEEP_RUNNING.store(false, Ordering::Release);
handle.join().unwrap();
```
