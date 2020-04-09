use crate::error::FallError;
use crate::Application;
use actix_web::web::resource;
use actix_web::web::Data;
use actix_web::web::HttpResponse;
use actix_web::web::ServiceConfig;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

pub trait CheckHealth {
    fn check(&self) -> Result<(), FallError>;
}

pub struct HealthList(BTreeMap<String, Box<dyn CheckHealth>>);

impl Default for HealthList {
    fn default() -> Self {
        HealthList(BTreeMap::new())
    }
}

impl HealthList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_check(&mut self, name: &str, check: Box<dyn CheckHealth>) {
        self.0.insert(name.to_owned(), check);
    }
}

async fn info(app: Data<Application>) -> HttpResponse {
    HttpResponse::Ok().json(app.as_ref())
}

fn modify_health(re: Result<(), FallError>, name: String, health: &mut Health) {
    let ok = re.is_ok();
    health.detail.insert(
        name,
        Health {
            status: {
                if ok {
                    HealthStatus::UP
                } else {
                    HealthStatus::DOWN
                }
            },
            err: re.err().map(|e| format!("{}", e)),
            detail: BTreeMap::new(),
        },
    );
    if !ok {
        health.status = HealthStatus::DOWN;
    }
}

async fn endpoint_health(app: Data<HealthList>) -> HttpResponse {
    #[allow(unused_mut)]
    let mut health = Health {
        status: HealthStatus::UP,
        err: None,
        detail: BTreeMap::new(),
    };

    for (k, v) in app.0.iter() {
        modify_health(v.check(), k.clone(), &mut health);
    }

    HttpResponse::Ok().json(&health)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
enum HealthStatus {
    UP,
    DOWN,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Health {
    pub status: HealthStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub detail: BTreeMap<String, Health>,
}

pub fn endpoints(cfg: &mut ServiceConfig) {
    cfg.service(resource("/endpoints/info").to(info))
        .service(resource("/endpoints/health").to(endpoint_health));
}
