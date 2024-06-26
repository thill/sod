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
