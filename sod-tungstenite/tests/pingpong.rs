use sod::{idle::backoff, MaybeProcessService, MutService, RetryService, Service, ServiceChain};
use sod_tungstenite::{UninitializedWsSession, WsServer, WsSession, WsSessionEvent};
use std::{sync::atomic::Ordering, thread::spawn};
use tungstenite::{http::StatusCode, Message};
use url::Url;

#[test]
fn ping_pong() {
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
}
