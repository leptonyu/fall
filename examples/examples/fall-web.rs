use actix_web::web::resource;
use actix_web::web::ServiceConfig;
use fall_log::*;
use fall_web::*;

async fn hello() -> Result<&'static str, FallError> {
    info!("Hello, world");
    warn!("Hello, world");
    warn!("Hello, world");
    warn!("Hello, world");
    warn!("Hello, world");
    warn!("Hello, world");
    Err(FallError::bad_request("错误"))
}

fn config(cfg: &mut ServiceConfig) {
    cfg.service(resource("/hello").to(hello));
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    fall_web::start(config, DefaultFallServer::default()).await
}
