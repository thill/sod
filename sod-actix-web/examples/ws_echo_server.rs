use std::convert::Infallible;

use actix_web::{web, App, HttpServer};
use sod::MutService;
use sod_actix_web::ws::{WsMessage, WsSessionEvent, WsSessionFactory};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    struct EchoService;
    impl MutService for EchoService {
        type Input = WsSessionEvent;
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
