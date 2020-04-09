use crate::endpoints::CheckHealth;
use crate::error::FallError;
use crate::PoolConfig;
use fall_log::info;
use r2d2::PooledConnection;
use r2d2_redis::redis::cmd;
use r2d2_redis::{r2d2::Pool, RedisConnectionManager};
use serde::Deserialize;
use std::ops::DerefMut;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RedisConfig {
    url: String,
    pool: Option<PoolConfig>,
}

#[derive(Clone)]
pub struct RedisConn(pub Pool<RedisConnectionManager>);

impl RedisConn {
    fn get_conn(&self) -> Result<PooledConnection<RedisConnectionManager>, FallError> {
        Ok(self.0.get()?)
    }
}

impl CheckHealth for RedisConn {
    fn check(&self) -> Result<(), FallError> {
        Ok(cmd("PING").query(self.get_conn()?.deref_mut())?)
    }
}

impl RedisConfig {
    pub fn init(&self) -> Result<RedisConn, FallError> {
        info!("Init Redis...");
        Ok(self
            .pool
            .as_ref()
            .map(|p| {
                Pool::builder()
                    .max_size(p.max_size.unwrap_or(10))
                    .min_idle(p.min_idle)
                    .max_lifetime(p.max_lifetime)
                    .idle_timeout(p.idle_timeout)
                    .connection_timeout(p.connection_timeout.unwrap_or(Duration::from_secs(30)))
            })
            .unwrap_or_else(Pool::builder)
            .build(RedisConnectionManager::new(self.url.as_str())?)
            .map(RedisConn)?)
    }
}
