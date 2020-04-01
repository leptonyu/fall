use crate::endpoints::endpoints;
use crate::endpoints::HealthList;
use crate::web::from_req;
use actix_web::dev::ServiceRequest;
use actix_web::dev::ServiceResponse;
use actix_web::web::ServiceConfig;
use actix_web::App;
use actix_web::HttpServer;
use fall_log::span;
use fall_log::FallLog;
use futures_util::future;
use serde::{Deserialize, Serialize};
use std::env::var;
use std::time::Duration;

pub use actix_web::http::StatusCode;
pub use error::FallError;

#[cfg(feature = "database")]
use crate::database::DatabaseConfig;
#[cfg(feature = "redis")]
use crate::redis::RedisConfig;

pub use config::Config;
pub use web::{DefaultRequestHandler, FallTransform};

#[cfg(feature = "database")]
pub mod database;
#[cfg(feature = "redis")]
pub mod redis;

pub mod endpoints;

mod error;
mod web;

#[derive(Debug, Clone, Deserialize)]
struct PoolConfig {
    max_size: Option<u32>,
    min_idle: Option<u32>,
    max_lifetime: Option<Duration>,
    idle_timeout: Option<Duration>,
    connection_timeout: Option<Duration>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Application {
    name: String,
    version: String,
    revision: Option<String>,
    commit_date: Option<String>,
    build_timestamp: Option<String>,
    build_target: Option<String>,
}

impl Default for Application {
    fn default() -> Self {
        Application {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            revision: var("VERGEN_SHA_SHORT").ok(),
            commit_date: var("VERGEN_COMMIT_DATE").ok(),
            build_timestamp: var("VERGEN_BUILD_TIMESTAMP").ok(),
            build_target: var("VERGEN_TARGET_TRIPLE").ok(),
        }
    }
}

pub trait RequestHandler {
    fn new_span(&self, req: &ServiceRequest) -> span::Span {
        from_req(req).into()
    }

    fn pre_request(&self, _req: &ServiceRequest) -> future::Ready<Result<(), actix_web::Error>> {
        future::ok(())
    }

    fn post_response<B>(&self, res: ServiceResponse<B>) -> ServiceResponse<B> {
        let status = res.status();
        if status == StatusCode::NOT_FOUND {
            return res.error_response(FallError::HTTP_ERROR(status, None));
        }
        res
    }
}

pub trait FallServer: Clone + Send + Sync {
    type H: RequestHandler;
    type W: std::io::Write + Send;

    fn get_addr(&self) -> String {
        String::from("0.0.0.0:8080")
    }

    fn new_request_handler(&self) -> Self::H;

    fn new_log(&self) -> FallLog<Self::W>;

    fn get_app(&self) -> &Application;

    fn get_config(&self) -> &Config;

    fn health_check(&self) -> HealthList {
        HealthList::new()
    }

    #[cfg(feature = "redis")]
    fn get_redis(&self) -> Result<redis::RedisConn, FallError> {
        self.get_config().get::<RedisConfig>("redis")?.init()
    }

    #[cfg(feature = "database")]
    fn get_database(&self) -> Result<database::DatabaseConn, FallError> {
        self.get_config().get::<DatabaseConfig>("database")?.init()
    }

    fn config<T, B>(&self, app: App<T, B>) -> App<T, B> {
        app
    }
}

#[derive(Clone)]
pub struct DefaultFallServer {
    app: Application,
    config: Config,
}

impl DefaultFallServer {
    pub fn new(config: Config, app: Application) -> Self {
        DefaultFallServer { app, config }
    }
}

fn set_config(config: &mut Config, app: &mut Application) -> Result<(), config::ConfigError> {
    config
        .set_default("redis.url", "redis://127.0.0.1/0")?
        .set_default("database.url", "postgres://postgres@127.0.0.1/postgres")?
        .merge(config::Environment::new())?
        .merge(config::File::with_name("app").required(false))?;
    if let Ok(name) = config.get::<String>("application.name") {
        app.name = name;
    }
    Ok(())
}

impl Default for DefaultFallServer {
    fn default() -> Self {
        let mut app = Application::default();
        let mut config = Config::new();
        set_config(&mut config, &mut app).unwrap();
        DefaultFallServer { app, config }
    }
}

impl FallServer for DefaultFallServer {
    type H = DefaultRequestHandler;
    type W = std::io::Stdout;

    fn get_addr(&self) -> String {
        format!(
            "{}:{}",
            self.config
                .get::<&str>("application.address")
                .unwrap_or("0.0.0.0"),
            self.config.get::<u16>("application.port").unwrap_or(8080)
        )
    }

    fn new_request_handler(&self) -> Self::H {
        DefaultRequestHandler
    }

    fn new_log(&self) -> FallLog<Self::W> {
        FallLog::new(self.app.name.clone(), std::io::stdout())
    }

    fn get_app(&self) -> &Application {
        &self.app
    }

    fn get_config(&self) -> &Config {
        &self.config
    }
}

pub async fn start<F, A>(config: F, app: A) -> std::io::Result<()>
where
    F: FnMut(&mut ServiceConfig) + Send + Clone + 'static,
    A: FallServer + 'static,
{
    let _ = app.new_log().init();
    let addr = app.get_addr();
    #[cfg(feature = "redis")]
    let redis = app.get_redis()?;
    #[cfg(feature = "database")]
    let db = app.get_database()?;
    HttpServer::new(move || {
        #[allow(unused_mut)]
        let mut check = app.health_check();
        let a = app.config(
            App::new()
                .wrap(FallTransform::new(app.new_request_handler()))
                .data(app.get_app().clone()),
        );
        #[cfg(feature = "redis")]
        let a = a.data(redis.clone());
        #[cfg(feature = "redis")]
        check.add_check("redis", Box::new(redis.clone()));
        #[cfg(feature = "database")]
        let a = a.data(db.clone());
        #[cfg(feature = "database")]
        check.add_check("database", Box::new(db.clone()));
        a.data(check).configure(endpoints).configure(config.clone())
    })
    .bind(addr)?
    .run()
    .await
}
