use std::convert::Infallible;

use actix_web::{web, App, HttpServer};
use sod::Service;
use sod_actix_web::ServiceHandler;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    struct GreetService;
    impl Service for GreetService {
        type Input = web::Path<String>;
        type Output = String;
        type Error = Infallible;
        fn process(&self, name: web::Path<String>) -> Result<Self::Output, Self::Error> {
            Ok(format!("Hello {name}!"))
        }
    }

    HttpServer::new(|| {
        App::new().service(
            web::resource("/greet/{name}")
                .route(web::get().to(ServiceHandler::new(GreetService.into_async()))),
        )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
