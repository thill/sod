//! [`sod::Service`] implementations to interact with [`tungstenite`] websockets.
//!
//! ## Service Impls
//!
//! All Services are [`Retryable`] and are able to be blocking or non-blocking.
//!
//! - [`WsSession`] is a [`MutService`] that wraps a [`tungstenite::WebSocket`], accepting [`WsSessionEvent`] to send or receive messages. `WsSession::into_split` can split a `WsSession` into a `WsReader`, `WsWriter`, and `WsFlusher`.
//! - [`WsReader`] is a [`Service`] that wraps a [`Mutex<tungstenite::WebSocket>`], accepting a `()` as input and producing [`tungstenite::Message`] as output.
//! - [`WsWriter`] is a [`Service`] that wraps a [`Mutex<tungstenite::WebSocket>`], accepting a `tungstenite::Message` as input.
//! - [`WsFlusher`] is a [`Service`] that wraps a [`Mutex<tungstenite::WebSocket>`], accepting a `()` as input.
//! - [`WsServer`] is a [`Service`] that that listens on a TCP port, accepting a `()` as input and producing a `WsSession` as output.
//!
//! ## Features
//!
//! - `native-tls` to enable Native TLS
//! - `__rustls-tls` to enable Rustls TLS`
//!
//! ## Blocking Example
//!
//! ```no_run
//! use sod::{idle::backoff, MaybeProcessService, MutService, RetryService, Service, ServiceChain};
//! use sod_tungstenite::{UninitializedWsSession, WsServer, WsSession, WsSessionEvent};
//! use std::{sync::atomic::Ordering, thread::spawn};
//! use tungstenite::{http::StatusCode, Message};
//! use url::Url;
//!
//! // server session logic to add `"pong: "` in front of text payload
//! struct PongService;
//! impl Service for PongService {
//!     type Input = Message;
//!     type Output = Option<Message>;
//!     type Error = ();
//!     fn process(&self, input: Message) -> Result<Self::Output, Self::Error> {
//!         match input {
//!             Message::Text(text) => Ok(Some(Message::Text(format!("pong: {text}")))),
//!             _ => Ok(None),
//!         }
//!     }
//! }
//!
//! // wires session logic and spawns in new thread
//! struct SessionSpawner;
//! impl Service for SessionSpawner {
//!     type Input = UninitializedWsSession;
//!     type Output = ();
//!     type Error = ();
//!     fn process(&self, input: UninitializedWsSession) -> Result<Self::Output, Self::Error> {
//!         spawn(|| {
//!             let (r, w, f) = input.handshake().unwrap().into_split();
//!             let chain = ServiceChain::start(r)
//!                 .next(PongService)
//!                 .next(MaybeProcessService::new(w))
//!                 .next(MaybeProcessService::new(f))
//!                 .end();
//!             sod::thread::spawn_loop(chain, |err| {
//!                 println!("Session: {err:?}");
//!                 Err(err) // stop thread on error
//!             });
//!         });
//!         Ok(())
//!     }
//! }
//!
//! // start a blocking server that creates blocking sessions
//! let server = WsServer::bind("127.0.0.1:48490").unwrap();
//!
//! // spawn a thread to start accepting new server sessions
//! let handle = sod::thread::spawn_loop(
//!     ServiceChain::start(server).next(SessionSpawner).end(),
//!     |err| {
//!         println!("Server: {err:?}");
//!         Err(err) // stop thread on error
//!     },
//! );
//!
//! // connect a client to the server
//! let (mut client, _) =
//!     WsSession::connect(Url::parse("ws://127.0.0.1:48490/socket").unwrap()).unwrap();
//!
//! // client writes `"hello world"` payload
//! client
//!     .process(WsSessionEvent::WriteMessage(Message::Text(
//!         "hello world!".to_owned(),
//!     )))
//!     .unwrap();
//!
//! // client receives `"pong: hello world"` payload
//! println!(
//!     "Received: {:?}",
//!     client.process(WsSessionEvent::ReadMessage).unwrap()
//! );
//!
//! // join until server crashes
//! handle.join().unwrap();
//! ```
//!
//! ## Non-Blocking Example
//!
//! ```
//! use sod::{idle::backoff, MaybeProcessService, MutService, RetryService, Service, ServiceChain};
//! use sod_tungstenite::{UninitializedWsSession, WsServer, WsSession, WsSessionEvent};
//! use std::{sync::atomic::Ordering, thread::spawn};
//! use tungstenite::{http::StatusCode, Message};
//! use url::Url;
//!
//! // server session logic to add `"pong: "` in front of text payload
//! struct PongService;
//! impl Service for PongService {
//!     type Input = Message;
//!     type Output = Option<Message>;
//!     type Error = ();
//!     fn process(&self, input: Message) -> Result<Self::Output, Self::Error> {
//!         match input {
//!             Message::Text(text) => Ok(Some(Message::Text(format!("pong: {text}")))),
//!             _ => Ok(None),
//!         }
//!     }
//! }
//!
//! // wires session logic and spawns in new thread
//! struct SessionSpawner;
//! impl Service for SessionSpawner {
//!     type Input = UninitializedWsSession;
//!     type Output = ();
//!     type Error = ();
//!     fn process(&self, input: UninitializedWsSession) -> Result<Self::Output, Self::Error> {
//!         spawn(|| {
//!             let (r, w, f) = input.handshake().unwrap().into_split();
//!             let chain = ServiceChain::start(RetryService::new(r, backoff))
//!                 .next(PongService)
//!                 .next(MaybeProcessService::new(RetryService::new(w, backoff)))
//!                 .next(MaybeProcessService::new(f))
//!                 .end();
//!             sod::thread::spawn_loop(chain, |err| {
//!                 println!("Session: {err:?}");
//!                 Err(err) // stop thread on error
//!             });
//!         });
//!         Ok(())
//!     }
//! }
//!
//! // start a non-blocking server that creates non-blocking sessions
//! let server = WsServer::bind("127.0.0.1:48490")
//!     .unwrap()
//!     .with_nonblocking_sessions(true)
//!     .with_nonblocking_server(true)
//!     .unwrap();
//!
//! // spawn a thread to start accepting new server sessions
//! let handle = sod::thread::spawn_loop(
//!     ServiceChain::start(RetryService::new(server, backoff))
//!         .next(SessionSpawner)
//!         .end(),
//!     |err| {
//!         println!("Server: {err:?}");
//!         Err(err) // stop thread on error
//!     },
//! );
//!
//! // connect a client to the server
//! let (mut client, response) =
//!     WsSession::connect(Url::parse("ws://127.0.0.1:48490/socket").unwrap()).unwrap();
//! assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
//!
//! // client writes `"hello world"` payload
//! client
//!     .process(WsSessionEvent::WriteMessage(Message::Text(
//!         "hello world!".to_owned(),
//!     )))
//!     .unwrap();
//!
//! // client receives `"pong: hello world"` payload
//! assert_eq!(
//!     client.process(WsSessionEvent::ReadMessage).unwrap(),
//!     Some(Message::Text("pong: hello world!".to_owned()))
//! );
//!
//! // stop the server
//! sod::idle::KEEP_RUNNING.store(false, Ordering::Release);
//! handle.join().unwrap();
//! ```

use sod::{MutService, RetryError, Retryable, Service};
use std::{
    borrow::BorrowMut,
    io::{self, ErrorKind, Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::{Arc, Mutex},
};
use tungstenite::{
    accept_hdr_with_config, accept_with_config,
    client::IntoClientRequest,
    handshake::{
        client::Response,
        server::{Callback, NoCallback},
    },
    protocol::WebSocketConfig,
    stream::MaybeTlsStream,
    Error, Message, WebSocket,
};

pub extern crate tungstenite;

/// An input event for [`WsSession`], which can be a read or write.
#[derive(Clone, Debug)]
pub enum WsSessionEvent {
    ReadMessage,
    WriteMessage(Message),
    Flush,
}

/// A [`MutService`] that wraps a [`tungstenite::WebSocket`], processing a [`WsSessionEvent`], producing a `Some(Message)` when a [`Message`] is read, and producing `None` otherwise.
pub struct WsSession<S> {
    ws: WebSocket<S>,
}
impl<S> WsSession<S> {
    /// Wrap the given [`WebSocket`]
    pub fn new(ws: WebSocket<S>) -> Self {
        Self { ws }
    }
    /// Split this `WsSession` into a [`WsReader`] and [`WsWriter`], utilizing a [`Mutex`] to coordinate mutability on the underlying stream.
    pub fn into_split(self) -> (WsReader<S>, WsWriter<S>, WsFlusher<S>) {
        let ws = Arc::new(Mutex::new(self.ws));
        (
            WsReader::new(Arc::clone(&ws)),
            WsWriter::new(Arc::clone(&ws)),
            WsFlusher::new(ws),
        )
    }
}
impl WsSession<MaybeTlsStream<TcpStream>> {
    /// Connect to the given URL as a WebSocket Client, producing a [`WsSession`] and HTTP [`Response`].
    pub fn connect<Req: IntoClientRequest>(
        request: Req,
    ) -> Result<(WsSession<MaybeTlsStream<TcpStream>>, Response), Error> {
        let (ws, resp) = tungstenite::connect(request)?;
        Ok((WsSession::new(ws), resp))
    }
    /// Configure the underlying [`MaybeTlsStream`] to be non-blocking.
    ///
    /// Non-blocking services should usually be encpasulated by a [`RetryService`].
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<(), io::Error> {
        set_nonblocking(self.ws.get_ref(), nonblocking)
    }
}
impl<S: Read + Write> MutService for WsSession<S> {
    type Input = WsSessionEvent;
    type Output = Option<Message>;
    type Error = Error;
    fn process(&mut self, input: WsSessionEvent) -> Result<Self::Output, Self::Error> {
        Ok(match input {
            WsSessionEvent::ReadMessage => Some(self.ws.borrow_mut().read()?),
            WsSessionEvent::WriteMessage(message) => {
                self.ws.borrow_mut().send(message)?;
                None
            }
            WsSessionEvent::Flush => {
                self.ws.borrow_mut().flush()?;
                None
            }
        })
    }
}
impl<S> Retryable<WsSessionEvent, Error> for WsSession<S> {
    fn parse_retry(&self, err: Error) -> Result<WsSessionEvent, RetryError<Error>> {
        match err {
            Error::WriteBufferFull(message) => Ok(WsSessionEvent::WriteMessage(message)),
            Error::Io(io_err) => match &io_err.kind() {
                ErrorKind::WouldBlock => Ok(WsSessionEvent::ReadMessage),
                _ => Err(RetryError::ServiceError(Error::Io(io_err))),
            },
            err => Err(RetryError::ServiceError(err)),
        }
    }
}

/// The read-side of a split [`WsSession`].
#[derive(Clone)]
pub struct WsReader<S> {
    ws: Arc<Mutex<WebSocket<S>>>,
}
impl<S> WsReader<S> {
    fn new(ws: Arc<Mutex<WebSocket<S>>>) -> Self {
        Self { ws }
    }
}
impl<S: Read + Write> Service for WsReader<S> {
    type Input = ();
    type Output = Message;
    type Error = Error;
    fn process(&self, _: ()) -> Result<Self::Output, Self::Error> {
        let mut lock = match self.ws.lock() {
            Ok(lock) => lock,
            Err(_) => {
                return Err(Error::Io(io::Error::new(
                    ErrorKind::Other,
                    "WsReader mutex poisoned",
                )))
            }
        };
        lock.read()
    }
}
impl<S> Retryable<(), Error> for WsReader<S> {
    fn parse_retry(&self, err: Error) -> Result<(), RetryError<Error>> {
        match err {
            Error::Io(io_err) => match &io_err.kind() {
                ErrorKind::WouldBlock => Ok(()),
                _ => Err(RetryError::ServiceError(Error::Io(io_err))),
            },
            err => Err(RetryError::ServiceError(err)),
        }
    }
}

/// The write-side of a split [`WsSession`].
#[derive(Clone)]
pub struct WsWriter<S> {
    ws: Arc<Mutex<WebSocket<S>>>,
}
impl<S> WsWriter<S> {
    fn new(ws: Arc<Mutex<WebSocket<S>>>) -> Self {
        Self { ws }
    }
}
impl<S: Read + Write> Service for WsWriter<S> {
    type Input = Message;
    type Output = ();
    type Error = Error;
    fn process(&self, input: Message) -> Result<Self::Output, Self::Error> {
        let mut lock = match self.ws.lock() {
            Ok(lock) => lock,
            Err(_) => {
                return Err(Error::Io(io::Error::new(
                    ErrorKind::Other,
                    "WsWriter mutex poisoned",
                )))
            }
        };
        lock.write(input)
    }
}
impl<S> Retryable<Message, Error> for WsWriter<S> {
    fn parse_retry(&self, err: Error) -> Result<Message, RetryError<Error>> {
        match err {
            Error::WriteBufferFull(message) => Ok(message),
            err => Err(RetryError::ServiceError(err)),
        }
    }
}
impl<S> Retryable<Option<Message>, Error> for WsWriter<S> {
    fn parse_retry(&self, err: Error) -> Result<Option<Message>, RetryError<Error>> {
        match err {
            Error::WriteBufferFull(message) => Ok(Some(message)),
            err => Err(RetryError::ServiceError(err)),
        }
    }
}

/// The flush-side of a split [`WsSession`].
#[derive(Clone)]
pub struct WsFlusher<S> {
    ws: Arc<Mutex<WebSocket<S>>>,
}
impl<S> WsFlusher<S> {
    fn new(ws: Arc<Mutex<WebSocket<S>>>) -> Self {
        Self { ws }
    }
}
impl<S: Read + Write> Service for WsFlusher<S> {
    type Input = ();
    type Output = ();
    type Error = Error;
    fn process(&self, (): ()) -> Result<Self::Output, Self::Error> {
        let mut lock = match self.ws.lock() {
            Ok(lock) => lock,
            Err(_) => {
                return Err(Error::Io(io::Error::new(
                    ErrorKind::Other,
                    "WsFlusher mutex poisoned",
                )))
            }
        };
        lock.flush()
    }
}

/// Used to configure if and how TLS is used for a [`WsServer`].
pub enum Tls {
    None,
    #[cfg(feature = "native-tls")]
    Native,
    #[cfg(feature = "__rustls-tls")]
    Rustls,
}

/// A [`WsSession`] that has yet to complete its handshake.
///
/// Calling `UninitializedWsSession::handshake` will block on the handshake, producing a [`WsSession`].
pub struct UninitializedWsSession {
    stream: MaybeTlsStream<TcpStream>,
    nonblocking: bool,
}
impl UninitializedWsSession {
    fn new(stream: MaybeTlsStream<TcpStream>, nonblocking: bool) -> Self {
        Self {
            stream,
            nonblocking,
        }
    }

    /// Perform a blocking handshake, producing a [`WsSession`] or [`io::Error`] from `self`.
    pub fn handshake(self) -> Result<WsSession<MaybeTlsStream<TcpStream>>, io::Error> {
        self.handshake_with_params::<NoCallback>(None, None)
    }

    /// Perform a blocking handshake, with optional config and optional callback, producing a [`WsSession`] or [`io::Error`] from `self`.
    pub fn handshake_with_params<C: Callback>(
        self,
        callback: Option<C>,
        config: Option<WebSocketConfig>,
    ) -> Result<WsSession<MaybeTlsStream<TcpStream>>, io::Error> {
        let stream = self.stream;
        set_nonblocking(&stream, false)?;
        let ws = if let Some(callback) = callback {
            match accept_hdr_with_config(stream, callback, config) {
                Ok(v) => v,
                Err(err) => {
                    return Err(io::Error::new(
                        ErrorKind::Other,
                        format!("HandshakeError: {err:?}"),
                    ))
                }
            }
        } else {
            match accept_with_config(stream, config) {
                Ok(v) => v,
                Err(err) => {
                    return Err(io::Error::new(
                        ErrorKind::Other,
                        format!("HandshakeError: {err:?}"),
                    ))
                }
            }
        };
        let session = WsSession::new(ws);
        session.set_nonblocking(self.nonblocking)?;
        return Ok(session);
    }
}

/// A TCP Server that produces [`UninitializedWsSession`] as output.
pub struct WsServer {
    server: TcpListener,
    tls: Tls,
    nonblocking_sessions: bool,
}
impl WsServer {
    /// Wrap the given TcpListener
    pub fn new(server: TcpListener) -> Self {
        Self {
            server,
            tls: Tls::None,
            nonblocking_sessions: false,
        }
    }

    /// Bind to the given socket address
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self, io::Error> {
        let server = TcpListener::bind(addr)?;
        Ok(Self {
            server,
            tls: Tls::None,
            nonblocking_sessions: false,
        })
    }
}
impl WsServer {
    /// Builder pattern, set the TLS mode to use
    pub fn with_tls(mut self, tls: Tls) -> Self {
        self.tls = tls;
        self
    }
    /// Builder pattern, configure the nonblocking status for the underlying [`TcpListener`]
    pub fn with_nonblocking_server(self, nonblocking: bool) -> Result<Self, io::Error> {
        self.server.set_nonblocking(nonblocking)?;
        Ok(self)
    }
    /// Builder pattern, configure the default nonblocking status for produced [`WsSessions`] structs.
    pub fn with_nonblocking_sessions(mut self, nonblocking_sessions: bool) -> Self {
        self.nonblocking_sessions = nonblocking_sessions;
        self
    }
}
impl Service for WsServer {
    type Input = ();
    type Output = UninitializedWsSession;
    type Error = io::Error;
    fn process(&self, _: ()) -> Result<Self::Output, Self::Error> {
        match self.server.accept() {
            Ok((stream, _)) => {
                #[cfg(not(feature = "native-tls"))]
                let stream = match self.tls {
                    Tls::None => MaybeTlsStream::Plain(stream),
                    #[cfg(feature = "native-tls")]
                    Tls::Native => MaybeTlsStream::NativeTls(stream),
                    #[cfg(feature = "__rustls-tls")]
                    Tls::Rustls => MaybeTlsStream::Rustls(stream),
                };
                Ok(UninitializedWsSession::new(
                    stream,
                    self.nonblocking_sessions,
                ))
            }
            Err(err) => Err(err),
        }
    }
}
impl Retryable<(), io::Error> for WsServer {
    fn parse_retry(&self, err: io::Error) -> Result<(), RetryError<io::Error>> {
        match &err.kind() {
            ErrorKind::WouldBlock => Ok(()),
            _ => Err(RetryError::ServiceError(err)),
        }
    }
}

fn set_nonblocking(stream: &MaybeTlsStream<TcpStream>, nonblocking: bool) -> Result<(), io::Error> {
    match stream {
        MaybeTlsStream::Plain(stream) => stream.set_nonblocking(nonblocking),
        #[cfg(feature = "native-tls")]
        MaybeTlsStream::NativeTls(stream) => stream.set_nonblocking(nonblocking),
        #[cfg(feature = "__rustls-tls")]
        MaybeTlsStream::Rustls(stream) => stream.set_nonblocking(nonblocking),
        _ => return Err(io::Error::new(ErrorKind::Other, "unrecognized stream type")),
    }
}
