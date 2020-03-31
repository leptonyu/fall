use actix_web::web::resource;
use actix_web::web::ServiceConfig;
use actix_web::App;
use actix_web::HttpServer;
use actix_web::Responder;
use fall_log::*;
use fall_web::FallTransform;

async fn hello() -> impl Responder {
    info!("Hello, world");
    ""
}

pub async fn start<F>(name: String, config: F) -> std::io::Result<()>
where
    F: FnMut(&mut ServiceConfig) + Send + Clone + 'static,
{
    let _ = FallLog::new(name, std::io::stdout()).init();
    info!("Start Web");
    HttpServer::new(move || {
        App::new()
            .wrap(FallTransform::default())
            .configure(config.clone())
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

fn config(cfg: &mut ServiceConfig) {
    cfg.service(resource("/hello").to(hello));
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    start("fall-web".to_string(), config).await
}
